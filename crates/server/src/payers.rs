//! Gestion des payeurs (REQ-SUB-017).
//!
//! CRUD **isolé par foyer** : chaque handler passe l'`Actor` au `PayerRepository`, qui filtre par
//! `household_id`. Un payeur d'un autre foyer est traité comme inexistant (`404`, jamais `403`, §9).
//! Un payeur est une **étiquette nominative** (pas de compte). La suppression d'un payeur référencé
//! par un abonnement est **refusée** (`409`) — comportement capturé sur Wallos 5.4.2 (`household_in_use`,
//! oracle REQ-SUB-017-payer.json).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;
use wallos_core::Payer;
use wallos_core::actor::Actor;
use wallos_core::requirement;
use wallos_proto::{CreatePayerRequest, PayerDto, RenamePayerRequest, problem};
use wallos_storage::{CreateOutcome, Db, PayerDeleteOutcome, PayerRepository};

use crate::auth::AuthActor;
use crate::idempotency::{self, IdempotencyKey, Outcome};
use crate::problem_response;

/// `500` générique (défaut interne non divulgué).
#[requirement(REQ-SUB-017)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// `409` : l'`id` fourni par le client est **déjà pris** (collision de clé primaire, REQ-SYN-001).
#[requirement(REQ-SYN-001)]
fn duplicate_id() -> Response {
    problem_response(
        StatusCode::CONFLICT,
        problem(409, "about:blank", "Conflict").with_detail("id: identifiant déjà utilisé"),
    )
}

/// `404` générique pour un payeur inconnu ou hors du foyer — ne divulgue rien (§9).
#[requirement(REQ-SUB-017)]
fn payer_not_found() -> Response {
    problem_response(
        StatusCode::NOT_FOUND,
        problem(404, "about:blank", "Not Found"),
    )
}

/// `409` : le payeur est **référencé** par au moins un abonnement et ne peut donc pas être supprimé
/// (REQ-SUB-017). Comportement de référence capturé sur Wallos 5.4.2 (`household_in_use`).
#[requirement(REQ-SUB-017)]
fn payer_in_use() -> Response {
    problem_response(
        StatusCode::CONFLICT,
        problem(409, "about:blank", "Conflict")
            .with_detail("payer: référencé par un abonnement, suppression refusée"),
    )
}

/// `422` pour un nom de payeur invalide (vide ou trop long).
#[requirement(REQ-SUB-017)]
fn invalid_payer() -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity"),
    )
}

/// Crée un payeur dans le foyer de l'appelant. **Idempotent** via l'en-tête `Idempotency-Key`
/// (REQ-SYN-006).
#[utoipa::path(
    post,
    path = "/payers",
    operation_id = "createPayer",
    extensions(("x-requirements" = json!(["REQ-SUB-017", "REQ-SYN-006"]))),
    params(("Idempotency-Key" = Option<String>, Header, description = "Clé d'idempotence (REQ-SYN-006)")),
    request_body = CreatePayerRequest,
    responses(
        (status = 201, description = "Payeur créé", body = PayerDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 409, description = "Conflit : identifiant déjà utilisé, ou clé d'idempotence réutilisée avec un corps différent", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Identifiant ou nom invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-017)]
#[requirement(REQ-SYN-006)]
pub async fn create_payer(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    IdempotencyKey(idem): IdempotencyKey,
    Json(req): Json<CreatePayerRequest>,
) -> Response {
    if let Outcome::Short(response) = idempotency::begin(&db, &actor, idem.as_deref(), &req).await {
        return response;
    }
    match build_payer(&db, &actor, req).await {
        Ok((status, body)) => {
            idempotency::finish(&db, &actor, idem.as_deref(), status, &body).await;
            (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
        }
        Err(response) => {
            idempotency::abort(&db, &actor, idem.as_deref()).await;
            response
        }
    }
}

/// Exécute la création (validation + persistance) et renvoie la réponse **réussie** mémorisable.
#[requirement(REQ-SUB-017)]
async fn build_payer(
    db: &Db,
    actor: &Actor,
    req: CreatePayerRequest,
) -> Result<(StatusCode, String), Response> {
    let Ok(id) = req.resolve_id() else {
        return Err(invalid_payer());
    };
    let Ok(payer) = Payer::new(id, req.name) else {
        return Err(invalid_payer());
    };
    match PayerRepository::new(db.pool())
        .create(actor, payer.id(), payer.name())
        .await
    {
        Ok(CreateOutcome::Created) => {
            let dto = PayerDto {
                id: payer.id().to_string(),
                name: payer.name().to_string(),
            };
            Ok((
                StatusCode::CREATED,
                serde_json::to_string(&dto).unwrap_or_default(),
            ))
        }
        // Aucune unicité de nom sur les payeurs : DuplicateName ne peut pas survenir.
        Ok(CreateOutcome::DuplicateId) => Err(duplicate_id()),
        Ok(CreateOutcome::DuplicateName) | Err(_) => Err(internal_error()),
    }
}

/// Liste les payeurs du foyer de l'appelant.
#[utoipa::path(
    get,
    path = "/payers",
    operation_id = "listPayers",
    extensions(("x-requirements" = json!(["REQ-SUB-017"]))),
    responses(
        (status = 200, description = "Payeurs du foyer", body = Vec<PayerDto>, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-017)]
pub async fn list_payers(AuthActor(actor): AuthActor, State(db): State<Db>) -> Response {
    match PayerRepository::new(db.pool()).list(&actor).await {
        Ok(rows) => {
            let payers: Vec<PayerDto> = rows
                .into_iter()
                .map(|row| PayerDto {
                    id: row.id.to_string(),
                    name: row.name,
                })
                .collect();
            Json(payers).into_response()
        }
        Err(_) => internal_error(),
    }
}

/// Renomme un payeur du foyer de l'appelant.
#[utoipa::path(
    put,
    path = "/payers/{id}",
    operation_id = "renamePayer",
    params(("id" = String, Path, description = "Identifiant (UUID) du payeur")),
    extensions(("x-requirements" = json!(["REQ-SUB-017"]))),
    request_body = RenamePayerRequest,
    responses(
        (status = 200, description = "Payeur renommé", body = PayerDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Payeur inconnu ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Nom invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-017)]
pub async fn rename_payer(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
    Json(req): Json<RenamePayerRequest>,
) -> Response {
    let Ok(payer_id) = Uuid::parse_str(&id) else {
        return payer_not_found();
    };
    let Ok(payer) = Payer::new(payer_id, req.name) else {
        return invalid_payer();
    };
    match PayerRepository::new(db.pool())
        .rename(&actor, payer_id, payer.name())
        .await
    {
        Ok(true) => Json(PayerDto {
            id: payer_id.to_string(),
            name: payer.name().to_string(),
        })
        .into_response(),
        Ok(false) => payer_not_found(),
        Err(_) => internal_error(),
    }
}

/// Supprime un payeur du foyer de l'appelant.
///
/// Refuse la suppression d'un payeur **référencé** par un abonnement (`409`, comportement capturé sur
/// l'application d'origine — jamais de réaffectation ni de cascade).
#[utoipa::path(
    delete,
    path = "/payers/{id}",
    operation_id = "deletePayer",
    params(("id" = String, Path, description = "Identifiant (UUID) du payeur")),
    extensions(("x-requirements" = json!(["REQ-SUB-017"]))),
    responses(
        (status = 204, description = "Payeur supprimé"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Payeur inconnu ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 409, description = "Payeur référencé par un abonnement : suppression refusée", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-017)]
pub async fn delete_payer(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Response {
    let Ok(payer_id) = Uuid::parse_str(&id) else {
        return payer_not_found();
    };
    match PayerRepository::new(db.pool())
        .delete(&actor, payer_id)
        .await
    {
        Ok(PayerDeleteOutcome::Deleted) => StatusCode::NO_CONTENT.into_response(),
        Ok(PayerDeleteOutcome::NotFound) => payer_not_found(),
        Ok(PayerDeleteOutcome::InUse) => payer_in_use(),
        Err(_) => internal_error(),
    }
}
