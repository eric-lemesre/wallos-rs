//! Gestion des moyens de paiement (REQ-SUB-011).
//!
//! CRUD **isolé par foyer** : chaque handler passe l'`Actor` au `PaymentMethodRepository`, qui filtre
//! par `household_id`. Un moyen de paiement d'un autre foyer est traité comme inexistant (`404`, jamais
//! `403`, §9). Un moyen créé est immédiatement disponible pour le formulaire d'abonnement. Calque des
//! catégories (REQ-CAT-001).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;
use wallos_core::PaymentMethod;
use wallos_core::actor::Actor;
use wallos_core::requirement;
use wallos_proto::{
    CreatePaymentMethodRequest, PaymentMethodDto, RenamePaymentMethodRequest, problem,
};
use wallos_storage::{Db, PaymentMethodRepository};

use crate::auth::AuthActor;
use crate::idempotency::{self, IdempotencyKey, Outcome};
use crate::problem_response;

/// `500` générique (défaut interne non divulgué).
#[requirement(REQ-SUB-011)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// `404` générique pour un moyen de paiement inconnu ou hors du foyer — ne divulgue rien (§9).
#[requirement(REQ-SUB-011)]
fn not_found() -> Response {
    problem_response(
        StatusCode::NOT_FOUND,
        problem(404, "about:blank", "Not Found"),
    )
}

/// `422` pour un nom de moyen de paiement invalide (vide ou trop long).
#[requirement(REQ-SUB-011)]
fn invalid() -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity"),
    )
}

/// Crée un moyen de paiement dans le foyer de l'appelant. **Idempotent** via `Idempotency-Key`
/// (REQ-SYN-006).
#[utoipa::path(
    post,
    path = "/payment-methods",
    operation_id = "createPaymentMethod",
    extensions(("x-requirements" = json!(["REQ-SUB-011", "REQ-SYN-006"]))),
    params(("Idempotency-Key" = Option<String>, Header, description = "Clé d'idempotence (REQ-SYN-006)")),
    request_body = CreatePaymentMethodRequest,
    responses(
        (status = 201, description = "Moyen de paiement créé", body = PaymentMethodDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 409, description = "Clé d'idempotence réutilisée avec un corps différent", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Nom invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-011)]
#[requirement(REQ-SYN-006)]
pub async fn create_payment_method(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    IdempotencyKey(idem): IdempotencyKey,
    Json(req): Json<CreatePaymentMethodRequest>,
) -> Response {
    if let Outcome::Short(response) = idempotency::begin(&db, &actor, idem.as_deref(), &req).await {
        return response;
    }
    match build_payment_method(&db, &actor, req).await {
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

/// Exécute la création (validation + persistance) et renvoie `(statut, corps JSON)` mémorisable, ou
/// une réponse d'erreur (non mémorisée).
#[requirement(REQ-SUB-011)]
async fn build_payment_method(
    db: &Db,
    actor: &Actor,
    req: CreatePaymentMethodRequest,
) -> Result<(StatusCode, String), Response> {
    // Identifiant **fourni par le client** (offline-first, REQ-SYN-001) conservé s'il est présent et
    // valide, sinon généré par le serveur.
    let Ok(id) = req.resolve_id() else {
        return Err(invalid());
    };
    let Ok(pm) = PaymentMethod::new(id, req.name) else {
        return Err(invalid());
    };
    match PaymentMethodRepository::new(db.pool())
        .create(actor, pm.id(), pm.name())
        .await
    {
        Ok(()) => {
            let dto = PaymentMethodDto {
                id: pm.id().to_string(),
                name: pm.name().to_string(),
            };
            Ok((
                StatusCode::CREATED,
                serde_json::to_string(&dto).unwrap_or_default(),
            ))
        }
        _ => Err(internal_error()),
    }
}

/// Liste les moyens de paiement du foyer de l'appelant.
#[utoipa::path(
    get,
    path = "/payment-methods",
    operation_id = "listPaymentMethods",
    extensions(("x-requirements" = json!(["REQ-SUB-011"]))),
    responses(
        (status = 200, description = "Moyens de paiement du foyer", body = Vec<PaymentMethodDto>, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-011)]
pub async fn list_payment_methods(AuthActor(actor): AuthActor, State(db): State<Db>) -> Response {
    match PaymentMethodRepository::new(db.pool()).list(&actor).await {
        Ok(rows) => {
            let items: Vec<PaymentMethodDto> = rows
                .into_iter()
                .map(|row| PaymentMethodDto {
                    id: row.id.to_string(),
                    name: row.name,
                })
                .collect();
            Json(items).into_response()
        }
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Renomme un moyen de paiement du foyer de l'appelant.
#[utoipa::path(
    put,
    path = "/payment-methods/{id}",
    operation_id = "renamePaymentMethod",
    params(("id" = String, Path, description = "Identifiant (UUID) du moyen de paiement")),
    extensions(("x-requirements" = json!(["REQ-SUB-011"]))),
    request_body = RenamePaymentMethodRequest,
    responses(
        (status = 200, description = "Moyen de paiement renommé", body = PaymentMethodDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Inconnu ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Nom invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-011)]
pub async fn rename_payment_method(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
    Json(req): Json<RenamePaymentMethodRequest>,
) -> Response {
    let Ok(pm_id) = Uuid::parse_str(&id) else {
        return not_found();
    };
    let Ok(pm) = PaymentMethod::new(pm_id, req.name) else {
        return invalid();
    };
    match PaymentMethodRepository::new(db.pool())
        .rename(&actor, pm_id, pm.name())
        .await
    {
        Ok(true) => Json(PaymentMethodDto {
            id: pm_id.to_string(),
            name: pm.name().to_string(),
        })
        .into_response(),
        Ok(false) => not_found(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Supprime un moyen de paiement du foyer de l'appelant.
#[utoipa::path(
    delete,
    path = "/payment-methods/{id}",
    operation_id = "deletePaymentMethod",
    params(("id" = String, Path, description = "Identifiant (UUID) du moyen de paiement")),
    extensions(("x-requirements" = json!(["REQ-SUB-011"]))),
    responses(
        (status = 204, description = "Moyen de paiement supprimé"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Inconnu ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-011)]
pub async fn delete_payment_method(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Response {
    let Ok(pm_id) = Uuid::parse_str(&id) else {
        return not_found();
    };
    match PaymentMethodRepository::new(db.pool())
        .delete(&actor, pm_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => not_found(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}
