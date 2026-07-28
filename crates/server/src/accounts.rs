//! Création de compte (REQ-AUT-001).
//!
//! Endpoint public : accessible sans session (comme `getHealth`). Le hachage argon2id est réalisé
//! ici, côté serveur ; la couche `storage` ne reçoit qu'un hash. Anti-énumération : la réponse est
//! identique que l'e-mail existe déjà ou non.

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHasher};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use wallos_core::password_policy::{PasswordPolicyError, validate_password};
use wallos_core::requirement;
use wallos_proto::{CreateAccountRequest, problem};
use wallos_storage::{Db, UserRepository};

use crate::problem_response;

/// Applique la politique de mot de passe (REQ-AUT-003). Retourne l'erreur (légère) ; l'appelant
/// construit la `422 Problem` dont le `detail` porte la clé i18n stable du motif.
#[requirement(REQ-AUT-003)]
fn enforce_password_policy(password: &str) -> Result<(), PasswordPolicyError> {
    validate_password(password)
}

/// Crée un compte utilisateur.
#[utoipa::path(
    post,
    path = "/accounts",
    operation_id = "createAccount",
    request_body = CreateAccountRequest,
    extensions(("x-requirements" = json!(["REQ-AUT-001"]))),
    responses(
        (status = 201, description = "Compte créé — réponse identique que l'e-mail existe ou non"),
        (
            status = 422,
            description = "Requête invalide",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        )
    )
)]
#[requirement(REQ-AUT-001)]
pub async fn create_account(
    State(db): State<Db>,
    Json(req): Json<CreateAccountRequest>,
) -> Response {
    if let Err(error) = enforce_password_policy(req.password.expose_secret()) {
        let body =
            problem(422, "about:blank", "Unprocessable Entity").with_detail(error.message_key());
        return problem_response(StatusCode::UNPROCESSABLE_ENTITY, body);
    }

    let Ok(hash) = hash_password(req.password.expose_secret()) else {
        let body = problem(500, "about:blank", "Internal Server Error");
        return problem_response(StatusCode::INTERNAL_SERVER_ERROR, body);
    };

    match UserRepository::new(db.pool())
        .create_account(&req.email, &hash)
        .await
    {
        // Anti-énumération : succès comme e-mail déjà pris renvoient le même 201, sans corps.
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(_) => {
            let body = problem(500, "about:blank", "Internal Server Error");
            problem_response(StatusCode::INTERNAL_SERVER_ERROR, body)
        }
    }
}

/// Hache un mot de passe en argon2id (paramètres par défaut, alignés OWASP).
#[requirement(REQ-AUT-001)]
pub(crate) fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}
