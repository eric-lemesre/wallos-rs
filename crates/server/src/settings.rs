//! Réglages du foyer (REQ-CUR-001) : devise de référence.
//!
//! La devise de référence est la devise cible de tous les agrégats (REQ-CUR-004) ; la modifier
//! recalcule les totaux dans la nouvelle devise **sans altérer les montants saisis**. Réglage
//! **par foyer** (§9) : chaque foyer a le sien (singleton, jamais 404 — toujours au moins le défaut).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use wallos_core::ReferenceCurrency;
use wallos_core::requirement;
use wallos_proto::{ReferenceCurrencyDto, problem};
use wallos_storage::{Db, SettingsRepository};

use crate::auth::AuthActor;
use crate::problem_response;

/// Lit la devise de référence du foyer de l'appelant.
#[utoipa::path(
    get,
    path = "/settings/reference-currency",
    operation_id = "getReferenceCurrency",
    extensions(("x-requirements" = json!(["REQ-CUR-001"]))),
    responses(
        (status = 200, description = "Devise de référence du foyer", body = ReferenceCurrencyDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-CUR-001)]
pub async fn get_reference_currency(AuthActor(actor): AuthActor, State(db): State<Db>) -> Response {
    match SettingsRepository::new(db.pool())
        .reference_currency(&actor)
        .await
    {
        Ok(currency) => Json(ReferenceCurrencyDto { currency }).into_response(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Fixe la devise de référence du foyer de l'appelant.
#[utoipa::path(
    put,
    path = "/settings/reference-currency",
    operation_id = "setReferenceCurrency",
    extensions(("x-requirements" = json!(["REQ-CUR-001"]))),
    request_body = ReferenceCurrencyDto,
    responses(
        (status = 200, description = "Devise de référence mise à jour", body = ReferenceCurrencyDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Devise hors référentiel", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-CUR-001)]
pub async fn set_reference_currency(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Json(req): Json<ReferenceCurrencyDto>,
) -> Response {
    // Valide contre le référentiel supporté ; jamais de devise inconnue en base.
    let Ok(reference) = ReferenceCurrency::parse(&req.currency) else {
        return problem_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            problem(422, "about:blank", "Unprocessable Entity")
                .with_detail("currency: devise hors référentiel"),
        );
    };
    let code = reference.code().as_str().to_string();
    match SettingsRepository::new(db.pool())
        .set_reference_currency(&actor, &code)
        .await
    {
        Ok(()) => Json(ReferenceCurrencyDto { currency: code }).into_response(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}
