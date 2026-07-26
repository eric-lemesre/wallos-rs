//! Authentification et session (REQ-AUT-002).
//!
//! - `POST /sessions` : vérifie e-mail + mot de passe (argon2id), ouvre une session (jeton opaque
//!   haché SHA-256, ADR 0018) posée en cookie `HttpOnly ; Secure ; SameSite=Lax`. **Timing-safe** :
//!   un argon2 verify est toujours exécuté (hash factice si le compte est absent) pour ne pas
//!   distinguer compte existant/absent. Échec → `401` générique identique.
//! - Extracteur `AuthActor` : reconstruit le contexte d'appelant depuis le cookie de session.
//! - `GET /me` : renvoie le compte courant (démontre l'accès authentifié + l'isolation par foyer).

use std::sync::LazyLock;

use std::net::SocketAddr;

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::Json;
use axum::extract::{ConnectInfo, FromRef, FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use wallos_core::actor::Actor;
use wallos_core::requirement;
use wallos_proto::{CreateSessionRequest, CurrentUser, problem};
use wallos_storage::{Db, LoginAttemptRepository, SessionRepository, UserRepository};

use crate::accounts::hash_password;
use crate::problem_response;

/// Nom du cookie de session.
const SESSION_COOKIE: &str = "session";

/// Durée d'inactivité (minutes) au-delà de laquelle une session est rejetée (REQ-AUT-004).
/// Configurable côté serveur via `SESSION_IDLE_TTL_MINUTES` (défaut 30), jamais côté client.
static SESSION_IDLE_TTL_MINUTES: LazyLock<i64> = LazyLock::new(|| {
    std::env::var("SESSION_IDLE_TTL_MINUTES")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|m| *m > 0)
        .unwrap_or(30)
});

/// Fenêtre d'inactivité d'une session (REQ-AUT-004).
#[requirement(REQ-AUT-004)]
fn session_idle_ttl() -> Duration {
    Duration::minutes(*SESSION_IDLE_TTL_MINUTES)
}

/// Hash argon2id factice, calculé une fois, pour rendre la vérification timing-safe quand le compte
/// n'existe pas (on dépense un temps comparable à une vraie vérification).
static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| hash_password("timing-safe-placeholder").unwrap_or_default());

/// Attribut `Secure` du cookie de session. **Actif par défaut** (production HTTPS) ; désactivable
/// via `SESSION_COOKIE_SECURE=false` uniquement pour les tests e2e locaux servis en HTTP.
static COOKIE_SECURE: LazyLock<bool> =
    LazyLock::new(|| std::env::var("SESSION_COOKIE_SECURE").as_deref() != Ok("false"));

/// Nombre de tentatives échouées, par compte ou par IP, au-delà duquel l'authentification est
/// limitée (REQ-AUT-008). Configurable côté serveur via `AUTH_RATELIMIT_MAX_ATTEMPTS` (défaut 5).
static RATELIMIT_MAX_ATTEMPTS: LazyLock<i64> = LazyLock::new(|| {
    std::env::var("AUTH_RATELIMIT_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|m| *m > 0)
        .unwrap_or(5)
});

/// Largeur (secondes) de la fenêtre glissante de comptage des tentatives (REQ-AUT-008).
/// Configurable côté serveur via `AUTH_RATELIMIT_WINDOW_SECONDS` (défaut 900 = 15 min).
static RATELIMIT_WINDOW_SECONDS: LazyLock<i64> = LazyLock::new(|| {
    std::env::var("AUTH_RATELIMIT_WINDOW_SECONDS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(900)
});

/// Fenêtre glissante de limitation du taux d'authentification (REQ-AUT-008).
#[requirement(REQ-AUT-008)]
fn ratelimit_window() -> Duration {
    Duration::seconds(*RATELIMIT_WINDOW_SECONDS)
}

/// Décide, à partir des statistiques d'une clé (compte ou IP), si l'accès est limité.
///
/// Renvoie `Some(retry_after_secs)` (≥ 1) si le nombre d'échecs atteint le seuil dans la fenêtre —
/// l'instant de retry étant la fin de fenêtre du plus ancien échec observé —, sinon `None`.
/// Fonction **pure** : l'instant est injecté, aucun accès à l'horloge.
#[requirement(REQ-AUT-008)]
fn blocked_retry_after(
    count: i64,
    earliest: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    window: Duration,
    max: i64,
) -> Option<i64> {
    if count < max {
        return None;
    }
    let earliest = earliest?;
    Some(((earliest + window) - now).num_seconds().max(1))
}

/// IP source de la requête (REQ-AUT-008), extraite de `ConnectInfo` posé par le service.
///
/// Extracteur **infaillible** : `None` quand aucune `ConnectInfo` n'est disponible (p. ex. tests
/// `oneshot` sans `into_make_service_with_connect_info`) — la dimension IP est alors ignorée.
pub struct ClientIp(Option<String>);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    #[requirement(REQ-AUT-008)]
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let ip = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip().to_string());
        Ok(Self(ip))
    }
}

/// Rejet `429` : trop de tentatives d'authentification (REQ-AUT-008). Porte l'en-tête `Retry-After`
/// (secondes). Type unitaire léger, calqué sur [`Unauthorized`].
struct RateLimited {
    retry_after: i64,
}

impl IntoResponse for RateLimited {
    #[requirement(REQ-AUT-008)]
    fn into_response(self) -> Response {
        let mut response = problem_response(
            StatusCode::TOO_MANY_REQUESTS,
            problem(429, "about:blank", "Too Many Requests"),
        );
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from(self.retry_after));
        response
    }
}

/// Vérifie un mot de passe contre un hash argon2id stocké.
#[requirement(REQ-AUT-002)]
fn verify_password(password: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Génère un jeton de session opaque (128 bits d'entropie via CSPRNG).
#[requirement(REQ-AUT-002)]
fn generate_token() -> String {
    SaltString::generate(&mut OsRng).to_string()
}

/// Rejet `401` générique — identique quel que soit le motif (anti-énumération / timing).
///
/// Type de rejet **léger** (unitaire) pour l'extracteur : la construction de la `Response`
/// n'a lieu qu'à `into_response`, évitant un `Err` volumineux (`clippy::result_large_err`).
pub struct Unauthorized;

impl IntoResponse for Unauthorized {
    #[requirement(REQ-AUT-002)]
    fn into_response(self) -> Response {
        problem_response(
            StatusCode::UNAUTHORIZED,
            problem(401, "about:blank", "Unauthorized"),
        )
    }
}

/// Authentifie un utilisateur et ouvre une session.
#[utoipa::path(
    post,
    path = "/sessions",
    operation_id = "createSession",
    request_body = CreateSessionRequest,
    extensions(("x-requirements" = json!(["REQ-AUT-002", "REQ-AUT-008"]))),
    responses(
        (status = 200, description = "Session ouverte ; cookie de session posé"),
        (
            status = 401,
            description = "Identifiants invalides",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        ),
        (
            status = 429,
            description = "Trop de tentatives ; réessayer après l'en-tête Retry-After",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        )
    )
)]
#[requirement(REQ-AUT-002)]
pub async fn create_session(
    State(db): State<Db>,
    ClientIp(ip): ClientIp,
    Json(req): Json<CreateSessionRequest>,
) -> Response {
    // Limitation du taux (REQ-AUT-008) : évaluée AVANT toute vérification d'identifiants, afin de
    // rejeter en 429 même si le mot de passe est correct. Compteurs par compte ET par IP source.
    let now = Utc::now();
    let since = now - ratelimit_window();
    let max = *RATELIMIT_MAX_ATTEMPTS;
    let window = ratelimit_window();
    let attempts = LoginAttemptRepository::new(db.pool());

    // Best-effort : une défaillance de lecture ne doit pas bloquer l'authentification légitime.
    let (email_count, email_earliest) = attempts
        .count_and_earliest_email(&req.email, since)
        .await
        .unwrap_or((0, None));
    let (ip_count, ip_earliest) = match ip.as_deref() {
        Some(ip) => attempts
            .count_and_earliest_ip(ip, since)
            .await
            .unwrap_or((0, None)),
        None => (0, None),
    };
    let retry_after = [
        blocked_retry_after(email_count, email_earliest, now, window, max),
        blocked_retry_after(ip_count, ip_earliest, now, window, max),
    ]
    .into_iter()
    .flatten()
    .max();
    if let Some(retry_after) = retry_after {
        return RateLimited { retry_after }.into_response();
    }

    let credentials = UserRepository::new(db.pool())
        .find_credentials_by_email(&req.email)
        .await
        .ok()
        .flatten();

    // Timing-safe : toujours exécuter un argon2 verify (réel ou factice).
    let actor = match credentials {
        Some(creds) if verify_password(&req.password, &creds.password_hash) => creds.actor,
        Some(_) => {
            let _ = attempts
                .record_failure(&req.email, ip.as_deref(), now)
                .await;
            return Unauthorized.into_response();
        }
        None => {
            let _ = verify_password(&req.password, &DUMMY_HASH);
            let _ = attempts
                .record_failure(&req.email, ip.as_deref(), now)
                .await;
            return Unauthorized.into_response();
        }
    };

    // Authentification réussie : réinitialiser le compteur du compte (best-effort).
    let _ = attempts.clear_email(&req.email).await;

    let token = generate_token();
    let token_hash = Sha256::digest(token.as_bytes());
    let expires_at = Utc::now() + session_idle_ttl();

    if SessionRepository::new(db.pool())
        .create(&actor, token_hash.as_slice(), expires_at)
        .await
        .is_err()
    {
        return problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        );
    }

    // Cookie de session opaque : aucune donnée métier, HttpOnly + SameSite=Lax (+ Secure par
    // défaut). Pas de Max-Age : le serveur fait autorité sur l'expiration (inactivité glissante).
    let secure = if *COOKIE_SECURE { "; Secure" } else { "" };
    let cookie = format!("{SESSION_COOKIE}={token}; HttpOnly{secure}; SameSite=Lax; Path=/");
    (StatusCode::OK, [(header::SET_COOKIE, cookie)]).into_response()
}

/// Contexte d'appelant extrait du cookie de session. Rejette en `401` si absent, inconnu ou expiré.
pub struct AuthActor(pub Actor);

impl<S> FromRequestParts<S> for AuthActor
where
    Db: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Unauthorized;

    #[requirement(REQ-AUT-002)]
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Some(token) = session_token(&parts.headers) else {
            return Err(Unauthorized);
        };
        let token_hash = Sha256::digest(token.as_bytes());
        let db = Db::from_ref(state);
        let repo = SessionRepository::new(db.pool());
        let now = Utc::now();
        match repo.find_valid(token_hash.as_slice(), now).await {
            Ok(Some(actor)) => {
                // Inactivité glissante : repousser l'expiration (best-effort, REQ-AUT-004).
                let _ = repo
                    .touch(token_hash.as_slice(), now + session_idle_ttl())
                    .await;
                Ok(Self(actor))
            }
            _ => Err(Unauthorized),
        }
    }
}

/// Extrait la valeur du cookie de session de l'en-tête `Cookie`.
#[requirement(REQ-AUT-002)]
fn session_token(headers: &HeaderMap) -> Option<String> {
    let header = headers.get(header::COOKIE)?.to_str().ok()?;
    header
        .split(';')
        .map(str::trim)
        .find_map(|kv| kv.strip_prefix(&format!("{SESSION_COOKIE}=")))
        .map(str::to_owned)
}

/// Renvoie le compte authentifié courant.
#[utoipa::path(
    get,
    path = "/me",
    operation_id = "getCurrentUser",
    extensions(("x-requirements" = json!(["REQ-AUT-002"]))),
    responses(
        (status = 200, description = "Compte courant", body = CurrentUser, content_type = "application/json"),
        (
            status = 401,
            description = "Non authentifié",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        )
    )
)]
#[requirement(REQ-AUT-002)]
pub async fn get_current_user(AuthActor(actor): AuthActor, State(db): State<Db>) -> Response {
    match UserRepository::new(db.pool())
        .find_in_household(&actor, actor.user_id())
        .await
    {
        Ok(Some(user)) => Json(CurrentUser { email: user.email }).into_response(),
        // Session valide mais compte introuvable : incohérence -> 401 (ne divulgue rien).
        _ => Unauthorized.into_response(),
    }
}

/// Déconnecte : invalide la session côté serveur et expire le cookie (REQ-AUT-009).
///
/// **Idempotent** : renvoie toujours `204`, même sans cookie ou session déjà invalidée.
#[utoipa::path(
    delete,
    path = "/sessions",
    operation_id = "deleteSession",
    extensions(("x-requirements" = json!(["REQ-AUT-009"]))),
    responses((status = 204, description = "Session invalidée (idempotent) ; cookie expiré"))
)]
#[requirement(REQ-AUT-009)]
pub async fn delete_session(State(db): State<Db>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        let token_hash = Sha256::digest(token.as_bytes());
        // Best-effort : l'idempotence prime, on renvoie 204 quoi qu'il arrive.
        let _ = SessionRepository::new(db.pool())
            .delete(token_hash.as_slice())
            .await;
    }
    let secure = if *COOKIE_SECURE { "; Secure" } else { "" };
    let expired = format!("{SESSION_COOKIE}=; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age=0");
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, expired)]).into_response()
}

#[cfg(test)]
mod tests {
    use super::{Duration, Utc, blocked_retry_after};
    use wallos_core::verifies;

    const MAX: i64 = 5;

    fn window() -> Duration {
        Duration::seconds(900)
    }

    /// Sous le seuil : jamais limité.
    #[test]
    #[verifies(REQ-AUT-008)]
    fn under_threshold_is_not_blocked() {
        let now = Utc::now();
        assert_eq!(
            blocked_retry_after(MAX - 1, Some(now), now, window(), MAX),
            None
        );
    }

    /// Seuil atteint mais aucun instant de référence (cas dégénéré) : non limité, pas de panique.
    #[test]
    #[verifies(REQ-AUT-008)]
    fn at_threshold_without_earliest_is_not_blocked() {
        let now = Utc::now();
        assert_eq!(blocked_retry_after(MAX, None, now, window(), MAX), None);
    }

    /// Seuil atteint, plus ancien échec à l'instant courant : Retry-After = fenêtre complète.
    #[test]
    #[verifies(REQ-AUT-008)]
    fn at_threshold_returns_full_window() {
        let now = Utc::now();
        assert_eq!(
            blocked_retry_after(MAX, Some(now), now, window(), MAX),
            Some(900)
        );
    }

    /// Plus ancien échec déjà sorti de la fenêtre : Retry-After borné à 1 seconde (jamais ≤ 0).
    #[test]
    #[verifies(REQ-AUT-008)]
    fn expired_window_is_clamped_to_one_second() {
        let now = Utc::now();
        let earliest = now - window() - Duration::seconds(30);
        assert_eq!(
            blocked_retry_after(MAX, Some(earliest), now, window(), MAX),
            Some(1)
        );
    }
}
