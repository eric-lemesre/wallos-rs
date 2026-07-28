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
use axum::extract::{ConnectInfo, FromRef, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::password_policy::validate_password;
use wallos_core::requirement;
use wallos_proto::{
    ChangePasswordRequest, CreateDeviceSessionRequest, CreateSessionRequest, CurrentUser,
    DeviceSummary, DeviceToken, problem,
};
use wallos_storage::{
    Db, DeviceTokenRepository, LoginAttemptRepository, SessionRepository, UserRepository,
};

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

/// Retry-After (secondes) si le couple (compte, IP) dépasse le seuil de tentatives, sinon `None`.
///
/// Cœur de la limitation de taux (REQ-AUT-008), partagé par les endpoints de login web et appareil.
/// Best-effort : une défaillance de lecture ne bloque pas l'authentification légitime.
#[requirement(REQ-AUT-008)]
async fn rate_limit_retry_after(
    attempts: &LoginAttemptRepository<'_>,
    email: &str,
    ip: Option<&str>,
    now: DateTime<Utc>,
) -> Option<i64> {
    let since = now - ratelimit_window();
    let max = *RATELIMIT_MAX_ATTEMPTS;
    let window = ratelimit_window();
    let (email_count, email_earliest) = attempts
        .count_and_earliest_email(email, since)
        .await
        .unwrap_or((0, None));
    let (ip_count, ip_earliest) = match ip {
        Some(ip) => attempts
            .count_and_earliest_ip(ip, since)
            .await
            .unwrap_or((0, None)),
        None => (0, None),
    };
    [
        blocked_retry_after(email_count, email_earliest, now, window, max),
        blocked_retry_after(ip_count, ip_earliest, now, window, max),
    ]
    .into_iter()
    .flatten()
    .max()
}

/// Authentifie un couple e-mail/mot de passe pour un endpoint de login (web ou appareil).
///
/// Applique la limitation de taux (REQ-AUT-008) **avant** la vérification, puis un argon2 verify
/// timing-safe (REQ-AUT-002 : hash factice si le compte est absent). Journalise les échecs, remet à
/// zéro le compteur du compte sur succès. Renvoie l'`Actor`, ou une `Response` de rejet prête à
/// retourner (`429` limité, `401` identifiants invalides — corps générique identique).
#[requirement(REQ-AUT-002)]
async fn authenticate(
    db: &Db,
    email: &str,
    password: &str,
    ip: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Actor, Response> {
    let attempts = LoginAttemptRepository::new(db.pool());
    if let Some(retry_after) = rate_limit_retry_after(&attempts, email, ip, now).await {
        return Err(RateLimited { retry_after }.into_response());
    }

    let credentials = UserRepository::new(db.pool())
        .find_credentials_by_email(email)
        .await
        .ok()
        .flatten();

    match credentials {
        Some(creds) if verify_password(password, &creds.password_hash) => {
            let _ = attempts.clear_email(email).await;
            Ok(creds.actor)
        }
        Some(_) => {
            let _ = attempts.record_failure(email, ip, now).await;
            Err(Unauthorized.into_response())
        }
        None => {
            // Timing-safe : dépenser un argon2 verify même sans compte.
            let _ = verify_password(password, &DUMMY_HASH);
            let _ = attempts.record_failure(email, ip, now).await;
            Err(Unauthorized.into_response())
        }
    }
}

/// Authentifie un utilisateur et ouvre une session web (cookie).
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
    let now = Utc::now();
    let actor = match authenticate(
        &db,
        &req.email,
        req.password.expose_secret(),
        ip.as_deref(),
        now,
    )
    .await
    {
        Ok(actor) => actor,
        Err(rejection) => return rejection,
    };

    let token = generate_token();
    let token_hash = Sha256::digest(token.as_bytes());
    let expires_at = now + session_idle_ttl();

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

/// Appaire un appareil natif et émet un jeton propre à l'appareil (REQ-AUT-005).
#[utoipa::path(
    post,
    path = "/device-sessions",
    operation_id = "createDeviceSession",
    request_body = CreateDeviceSessionRequest,
    extensions(("x-requirements" = json!(["REQ-AUT-005", "REQ-AUT-008"]))),
    responses(
        (status = 200, description = "Appareil appairé ; jeton d'appareil émis (corps)", body = DeviceToken, content_type = "application/json"),
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
#[requirement(REQ-AUT-005)]
pub async fn create_device_session(
    State(db): State<Db>,
    ClientIp(ip): ClientIp,
    Json(req): Json<CreateDeviceSessionRequest>,
) -> Response {
    let now = Utc::now();
    let actor = match authenticate(
        &db,
        &req.email,
        req.password.expose_secret(),
        ip.as_deref(),
        now,
    )
    .await
    {
        Ok(actor) => actor,
        Err(rejection) => return rejection,
    };

    // Jeton d'appareil opaque, long, révocable individuellement. Renvoyé une seule fois (corps) ;
    // seule son empreinte SHA-256 est stockée (ADR 0018/0019).
    let token = generate_token();
    let token_hash = Sha256::digest(token.as_bytes());
    let device_id = Uuid::new_v4();

    if DeviceTokenRepository::new(db.pool())
        .create(
            &actor,
            device_id,
            token_hash.as_slice(),
            &req.label,
            &req.platform,
            now,
        )
        .await
        .is_err()
    {
        return problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        );
    }

    (
        StatusCode::OK,
        Json(DeviceToken {
            token: token.into(),
        }),
    )
        .into_response()
}

/// Identifiant de l'appareil ayant présenté un jeton Bearer valide sur la requête courante.
///
/// Posé dans les extensions par [`AuthActor`] quand l'authentification provient d'un jeton
/// d'appareil (pas d'un cookie). Lu par l'extracteur [`CurrentDevice`] pour distinguer l'appareil
/// courant dans la liste (REQ-AUT-006).
#[derive(Clone, Copy)]
struct CurrentDeviceId(Uuid);

/// Appareil à l'origine de la requête courante, s'il s'agit d'un jeton d'appareil (REQ-AUT-006).
///
/// Extracteur **infaillible** : `None` pour une authentification par cookie (aucun appareil courant).
pub struct CurrentDevice(Option<Uuid>);

impl<S> FromRequestParts<S> for CurrentDevice
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    #[requirement(REQ-AUT-006)]
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(parts.extensions.get::<CurrentDeviceId>().map(|c| c.0)))
    }
}

/// Contexte d'appelant. Accepte **deux sources** : le cookie de session web (REQ-AUT-002/004) ou un
/// jeton d'appareil `Authorization: Bearer <token>` (REQ-AUT-005). Rejette en `401` sinon.
pub struct AuthActor(pub Actor);

impl<S> FromRequestParts<S> for AuthActor
where
    Db: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Unauthorized;

    #[requirement(REQ-AUT-002)]
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let db = Db::from_ref(state);
        let now = Utc::now();

        // 1) Cookie de session web (modalité navigateur).
        if let Some(token) = session_token(&parts.headers) {
            let token_hash = Sha256::digest(token.as_bytes());
            let repo = SessionRepository::new(db.pool());
            if let Ok(Some(actor)) = repo.find_valid(token_hash.as_slice(), now).await {
                // Inactivité glissante : repousser l'expiration (best-effort, REQ-AUT-004).
                let _ = repo
                    .touch(token_hash.as_slice(), now + session_idle_ttl())
                    .await;
                return Ok(Self(actor));
            }
        }

        // 2) Jeton d'appareil `Authorization: Bearer` (modalité native, REQ-AUT-005).
        if let Some(token) = bearer_token(&parts.headers) {
            let token_hash = Sha256::digest(token.as_bytes());
            let repo = DeviceTokenRepository::new(db.pool());
            if let Ok(Some((actor, device_id))) = repo.find_valid(token_hash.as_slice(), now).await
            {
                parts.extensions.insert(CurrentDeviceId(device_id));
                return Ok(Self(actor));
            }
        }

        Err(Unauthorized)
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

/// Extrait le jeton d'un en-tête `Authorization: Bearer <token>` (REQ-AUT-005).
#[requirement(REQ-AUT-005)]
fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
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

/// Réponse `404` générique pour un appareil inconnu (ou hors du foyer) — ne divulgue rien (§9).
#[requirement(REQ-AUT-006)]
fn device_not_found() -> Response {
    problem_response(
        StatusCode::NOT_FOUND,
        problem(404, "about:blank", "Not Found"),
    )
}

/// Liste les appareils appairés du foyer courant, en distinguant l'appareil courant (REQ-AUT-006).
#[utoipa::path(
    get,
    path = "/devices",
    operation_id = "listDevices",
    extensions(("x-requirements" = json!(["REQ-AUT-006"]))),
    responses(
        (status = 200, description = "Appareils appairés du foyer", body = Vec<DeviceSummary>, content_type = "application/json"),
        (
            status = 401,
            description = "Non authentifié",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        )
    )
)]
#[requirement(REQ-AUT-006)]
pub async fn list_devices(
    AuthActor(actor): AuthActor,
    CurrentDevice(current): CurrentDevice,
    State(db): State<Db>,
) -> Response {
    match DeviceTokenRepository::new(db.pool()).list(&actor).await {
        Ok(rows) => {
            let devices: Vec<DeviceSummary> = rows
                .into_iter()
                .map(|row| DeviceSummary {
                    current: current == Some(row.id),
                    id: row.id.to_string(),
                    label: row.label,
                    platform: row.platform,
                    last_seen_at: row.last_seen_at.to_rfc3339(),
                })
                .collect();
            Json(devices).into_response()
        }
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Révoque un appareil appairé — révocation **immédiate** : son jeton ne vaut plus rien (REQ-AUT-006).
#[utoipa::path(
    delete,
    path = "/devices/{id}",
    operation_id = "revokeDevice",
    params(("id" = String, Path, description = "Identifiant (UUID) de l'appareil à révoquer")),
    extensions(("x-requirements" = json!(["REQ-AUT-006"]))),
    responses(
        (status = 204, description = "Appareil révoqué"),
        (
            status = 401,
            description = "Non authentifié",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        ),
        (
            status = 404,
            description = "Appareil inconnu ou hors du foyer",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        )
    )
)]
#[requirement(REQ-AUT-006)]
pub async fn revoke_device(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Response {
    // Un identifiant mal formé est traité comme inexistant (404) — ne divulgue rien.
    let Ok(device_id) = Uuid::parse_str(&id) else {
        return device_not_found();
    };
    match DeviceTokenRepository::new(db.pool())
        .revoke(&actor, device_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => device_not_found(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Change le mot de passe du compte courant et coupe les accès existants (REQ-AUT-007).
///
/// Exige le mot de passe actuel (sinon `403`, aucun état modifié). Le nouveau doit respecter la
/// politique (REQ-AUT-003, sinon `422`). En cas de succès, **toutes les sessions et jetons
/// d'appareil sont invalidés sauf la crédential courante** — un changement de mot de passe qui ne
/// couperait pas les accès existants ne remédierait à rien.
#[utoipa::path(
    put,
    path = "/password",
    operation_id = "changePassword",
    request_body = ChangePasswordRequest,
    extensions(("x-requirements" = json!(["REQ-AUT-007", "REQ-AUT-008"]))),
    responses(
        (status = 204, description = "Mot de passe changé ; autres sessions et jetons d'appareil invalidés"),
        (
            status = 401,
            description = "Non authentifié",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        ),
        (
            status = 403,
            description = "Mot de passe actuel incorrect ; aucun changement",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        ),
        (
            status = 422,
            description = "Nouveau mot de passe non conforme",
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
#[requirement(REQ-AUT-007)]
pub async fn change_password(
    AuthActor(actor): AuthActor,
    CurrentDevice(current_device): CurrentDevice,
    ClientIp(ip): ClientIp,
    State(db): State<Db>,
    headers: HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> Response {
    let now = Utc::now();
    let users = UserRepository::new(db.pool());

    // Compte de l'appelant : e-mail (clé de limitation de taux) + hash actuel. Incohérence -> 403.
    let Some((email, stored_hash)) = users
        .find_email_and_password_hash(&actor)
        .await
        .ok()
        .flatten()
    else {
        return problem_response(
            StatusCode::FORBIDDEN,
            problem(403, "about:blank", "Forbidden"),
        );
    };

    // Limitation de taux (REQ-AUT-008) : le mot de passe actuel est vérifié en argon2 ; sans cette
    // garde, une session détournée pourrait forcer `current_password` pour prendre le compte.
    let attempts = LoginAttemptRepository::new(db.pool());
    if let Some(retry_after) = rate_limit_retry_after(&attempts, &email, ip.as_deref(), now).await {
        return RateLimited { retry_after }.into_response();
    }

    // Mot de passe actuel revérifié : échec -> 403, sans aucune modification d'état.
    if !verify_password(req.current_password.expose_secret(), &stored_hash) {
        let _ = attempts.record_failure(&email, ip.as_deref(), now).await;
        return problem_response(
            StatusCode::FORBIDDEN,
            problem(403, "about:blank", "Forbidden"),
        );
    }
    let _ = attempts.clear_email(&email).await;

    // Nouveau mot de passe : politique REQ-AUT-003 (longueur + non compromis).
    if let Err(error) = validate_password(req.new_password.expose_secret()) {
        let body =
            problem(422, "about:blank", "Unprocessable Entity").with_detail(error.message_key());
        return problem_response(StatusCode::UNPROCESSABLE_ENTITY, body);
    }

    let Ok(new_hash) = hash_password(req.new_password.expose_secret()) else {
        return problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        );
    };
    if users.update_password(&actor, &new_hash).await.is_err() {
        return problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        );
    }

    // Coupe tous les autres accès. On préserve la crédential courante : la session web si la requête
    // vient d'un cookie, sinon l'appareil courant si elle vient d'un jeton Bearer.
    let current_session_hash =
        session_token(&headers).map(|token| Sha256::digest(token.as_bytes()));
    let _ = SessionRepository::new(db.pool())
        .revoke_all_for_user_except(&actor, current_session_hash.as_ref().map(|h| h.as_slice()))
        .await;
    let _ = DeviceTokenRepository::new(db.pool())
        .revoke_all_for_user_except(&actor, current_device)
        .await;

    StatusCode::NO_CONTENT.into_response()
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
