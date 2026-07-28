//! Exposition du référentiel des devises supportées (REQ-CUR-007).
//!
//! Donnée de **référence globale** (pas de portée par foyer) : le référentiel `core` est statique,
//! l'endpoint le projette simplement en DTO. Auth requise (anon → 401), comme le reste de l'API.

use axum::Json;
use axum::response::{IntoResponse, Response};
use wallos_core::requirement;
use wallos_proto::CurrencyDto;

use crate::auth::AuthActor;

/// Liste les devises supportées (code, symbole, libellé, décimales), pour l'interface.
#[utoipa::path(
    get,
    path = "/currencies",
    operation_id = "listCurrencies",
    extensions(("x-requirements" = json!(["REQ-CUR-007"]))),
    responses(
        (status = 200, description = "Référentiel des devises supportées", body = Vec<CurrencyDto>, content_type = "application/json"),
        (
            status = 401,
            description = "Non authentifié",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        )
    )
)]
#[requirement(REQ-CUR-007)]
pub async fn list_currencies(
    // Auth requise ; le référentiel est global, l'`Actor` n'est pas utilisé pour filtrer.
    AuthActor(_actor): AuthActor,
) -> Response {
    let currencies: Vec<CurrencyDto> = wallos_core::currencies::all()
        .iter()
        .map(CurrencyDto::from_core)
        .collect();
    Json(currencies).into_response()
}
