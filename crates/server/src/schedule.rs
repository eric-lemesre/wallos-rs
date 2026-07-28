//! Calcul de la prochaine échéance (REQ-SUB-012).
//!
//! Face publique du calcul d'échéance ancré+clampé (ADR 0022). Donnée sans état (pas d'accès base) :
//! le handler valide les entrées et délègue à `core::next_due`. Auth requise (anon → 401).

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::NaiveDate;
use wallos_core::billing::{BillingCycle, BillingUnit};
use wallos_core::next_due;
use wallos_core::requirement;
use wallos_proto::{NextDueRequest, NextDueResponse, problem};

use crate::auth::AuthActor;
use crate::problem_response;

#[requirement(REQ-SUB-012)]
fn unprocessable() -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity"),
    )
}

/// Calcule la prochaine échéance strictement postérieure à `after`, pour l'ancre et le cycle donnés.
#[utoipa::path(
    post,
    path = "/schedule/next-due",
    operation_id = "computeNextDue",
    extensions(("x-requirements" = json!(["REQ-SUB-012"]))),
    request_body = NextDueRequest,
    responses(
        (status = 200, description = "Prochaine échéance", body = NextDueResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Entrée invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-012)]
pub async fn compute_next_due(
    // Auth requise ; le calcul est sans état (pas de portée foyer).
    AuthActor(_actor): AuthActor,
    Json(req): Json<NextDueRequest>,
) -> Response {
    let (Ok(anchor), Ok(after)) = (
        NaiveDate::parse_from_str(&req.first_payment, "%Y-%m-%d"),
        NaiveDate::parse_from_str(&req.after, "%Y-%m-%d"),
    ) else {
        return unprocessable();
    };
    let Ok(unit) = BillingUnit::parse(&req.cycle.unit) else {
        return unprocessable();
    };
    let Ok(cycle) = BillingCycle::from_parts(unit, req.cycle.interval) else {
        return unprocessable();
    };
    match next_due(anchor, cycle, after) {
        Some(date) => Json(NextDueResponse {
            next_payment: date.to_string(),
        })
        .into_response(),
        // Débordement de plage (dates astronomiques) : entrée hors domaine raisonnable.
        None => unprocessable(),
    }
}
