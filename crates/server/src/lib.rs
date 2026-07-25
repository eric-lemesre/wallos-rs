//! Serveur wallos-rs.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::{Json, Router};
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use wallos_core::requirement;
use wallos_proto::HealthResponse;

/// API wallos-rs v1.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "wallos-rs API",
        version = "0.1.0",
        description = "Code-first OpenAPI contract for wallos-rs."
    ),
    servers((url = "/api/v1")),
    paths(api_v1_health),
    components(schemas(HealthResponse))
)]
pub struct ApiDoc;

/// État de santé du serveur.
#[utoipa::path(
    get,
    path = "/health",
    operation_id = "getHealth",
    extensions(("x-requirements" = json!(["REQ-OPS-001"]))),
    responses(
        (status = 200, description = "Serveur opérationnel", body = HealthResponse, content_type = "application/json")
    )
)]
#[requirement(REQ-OPS-001)]
pub async fn api_v1_health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "wallos-rs".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        status: "ok".to_string(),
    })
}

/// Construit le routeur de l'application.
#[requirement(REQ-OPS-001)]
pub fn app() -> Router {
    let (router, _api) = OpenApiRouter::new()
        .routes(routes!(api_v1_health))
        .split_for_parts();
    Router::new().nest("/api/v1", router)
}
