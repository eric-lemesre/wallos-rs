//! Création d'abonnements (REQ-SUB-002).
//!
//! Intègre le modèle (SUB-001), le cycle (SUB-003) et le calcul d'échéance (SUB-012/013) : à la
//! création, l'abonnement est rattaché au **foyer** de l'appelant (§9) et sa **prochaine échéance
//! est calculée immédiatement** (dérivée, `next_due`). Validation **par champ** (critère #2).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use uuid::Uuid;
use wallos_core::next_due;
use wallos_core::requirement;
use wallos_proto::{CreateSubscriptionRequest, FieldError, SubscriptionDto, problem};
use wallos_storage::{Db, SubscriptionRepository};

use crate::auth::AuthActor;
use crate::problem_response;

/// `422` identifiant le champ fautif (RFC 9457 `detail`), REQ-SUB-002 critère #2.
#[requirement(REQ-SUB-002)]
fn field_error(err: &FieldError) -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity")
            .with_detail(format!("{}: {}", err.field, err.message)),
    )
}

/// Crée un abonnement dans le foyer de l'appelant ; renvoie l'abonnement et sa prochaine échéance.
#[utoipa::path(
    post,
    path = "/subscriptions",
    operation_id = "createSubscription",
    extensions(("x-requirements" = json!(["REQ-SUB-002"]))),
    request_body = CreateSubscriptionRequest,
    responses(
        (status = 201, description = "Abonnement créé (avec prochaine échéance)", body = SubscriptionDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Validation par champ", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-002)]
pub async fn create_subscription(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Json(req): Json<CreateSubscriptionRequest>,
) -> Response {
    let subscription = match req.into_core(Uuid::new_v4()) {
        Ok(sub) => sub,
        Err(err) => return field_error(&err),
    };

    // Prochaine échéance : première occurrence à partir d'aujourd'hui (inclus). Horloge serveur —
    // le domaine reste pur (l'instant est injecté ici).
    let today = Utc::now().date_naive();
    let reference = today.pred_opt().unwrap_or(today);
    let Some(next) = next_due(
        subscription.first_payment(),
        subscription.cycle(),
        reference,
    ) else {
        return field_error(&FieldError::new("first_payment", "échéance hors plage"));
    };

    match SubscriptionRepository::new(db.pool())
        .create(&actor, &subscription)
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(SubscriptionDto::from_core_with_next_payment(
                &subscription,
                next,
            )),
        )
            .into_response(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}
