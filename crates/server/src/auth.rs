//! Authentification et session (REQ-AUT-002).
//!
//! - `POST /sessions` : vérifie e-mail + mot de passe (argon2id), ouvre une session (jeton opaque
//!   haché SHA-256, ADR 0018) posée en cookie `HttpOnly ; Secure ; SameSite=Lax`. **Timing-safe** :
//!   un argon2 verify est toujours exécuté (hash factice si le compte est absent) pour ne pas
//!   distinguer compte existant/absent. Échec → `401` générique identique.
//! - Extracteur `AuthActor` : reconstruit le contexte d'appelant depuis le cookie de session.
//! - `GET /me` : renvoie le compte courant (démontre l'accès authentifié + l'isolation par foyer).

use std::sync::LazyLock;

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::Json;
use axum::extract::{FromRef, FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use wallos_core::actor::Actor;
use wallos_core::requirement;
use wallos_proto::{CreateSessionRequest, CurrentUser, problem};
use wallos_storage::{Db, SessionRepository, UserRepository};

use crate::accounts::hash_password;
use crate::problem_response;

/// Nom du cookie de session.
const SESSION_COOKIE: &str = "session";
/// Durée de vie d'une session, en heures.
const SESSION_TTL_HOURS: i64 = 24;

/// Hash argon2id factice, calculé une fois, pour rendre la vérification timing-safe quand le compte
/// n'existe pas (on dépense un temps comparable à une vraie vérification).
static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| hash_password("timing-safe-placeholder").unwrap_or_default());

/// Attribut `Secure` du cookie de session. **Actif par défaut** (production HTTPS) ; désactivable
/// via `SESSION_COOKIE_SECURE=false` uniquement pour les tests e2e locaux servis en HTTP.
static COOKIE_SECURE: LazyLock<bool> =
    LazyLock::new(|| std::env::var("SESSION_COOKIE_SECURE").as_deref() != Ok("false"));

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
    extensions(("x-requirements" = json!(["REQ-AUT-002"]))),
    responses(
        (status = 200, description = "Session ouverte ; cookie de session posé"),
        (
            status = 401,
            description = "Identifiants invalides",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        )
    )
)]
#[requirement(REQ-AUT-002)]
pub async fn create_session(
    State(db): State<Db>,
    Json(req): Json<CreateSessionRequest>,
) -> Response {
    let credentials = UserRepository::new(db.pool())
        .find_credentials_by_email(&req.email)
        .await
        .ok()
        .flatten();

    // Timing-safe : toujours exécuter un argon2 verify (réel ou factice).
    let actor = match credentials {
        Some(creds) if verify_password(&req.password, &creds.password_hash) => creds.actor,
        Some(_) => return Unauthorized.into_response(),
        None => {
            let _ = verify_password(&req.password, &DUMMY_HASH);
            return Unauthorized.into_response();
        }
    };

    let token = generate_token();
    let token_hash = Sha256::digest(token.as_bytes());
    let expires_at = Utc::now() + Duration::hours(SESSION_TTL_HOURS);

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

    // Cookie opaque : aucune donnée métier, HttpOnly + SameSite=Lax (+ Secure par défaut).
    // REQ-AUT-004 affinera (rotation, expiration d'inactivité).
    let secure = if *COOKIE_SECURE { "; Secure" } else { "" };
    let cookie = format!(
        "{SESSION_COOKIE}={token}; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age={}",
        SESSION_TTL_HOURS * 3600
    );
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
        let Some(token) = session_token(parts) else {
            return Err(Unauthorized);
        };
        let token_hash = Sha256::digest(token.as_bytes());
        let db = Db::from_ref(state);
        match SessionRepository::new(db.pool())
            .find_valid(token_hash.as_slice(), Utc::now())
            .await
        {
            Ok(Some(actor)) => Ok(Self(actor)),
            _ => Err(Unauthorized),
        }
    }
}

/// Extrait la valeur du cookie de session de l'en-tête `Cookie`.
#[requirement(REQ-AUT-002)]
fn session_token(parts: &Parts) -> Option<String> {
    let header = parts.headers.get(header::COOKIE)?.to_str().ok()?;
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
