//! Serveur wallos-rs.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use wallos_core::requirement;
use wallos_proto::{
    AggregateRequest, CategoryDto, ChangePasswordRequest, ConvertedTotalResponse,
    CreateAccountRequest, CreateCategoryRequest, CreateDeviceSessionRequest,
    CreatePaymentMethodRequest, CreateSessionRequest, CreateSubscriptionRequest, CurrencyDto,
    CurrentUser, DeviceSummary, DeviceToken, HealthResponse, MoneyInput, NextDueRequest,
    NextDueResponse, PaymentMethodDto, Problem, RenameCategoryRequest, RenamePaymentMethodRequest,
    SubscriptionDto, SubscriptionListResponse, problem,
};
use wallos_storage::Db;

pub mod accounts;
pub mod auth;
pub mod categories;
pub mod currencies;
pub mod exchange;
pub mod payment_methods;
pub mod schedule;
pub mod subscriptions;

/// API wallos-rs v1.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "wallos-rs API",
        version = "0.1.0",
        description = "Code-first OpenAPI contract for wallos-rs."
    ),
    servers((url = "/api/v1")),
    paths(
        api_v1_health,
        accounts::create_account,
        auth::create_session,
        auth::create_device_session,
        auth::get_current_user,
        auth::list_devices,
        auth::revoke_device,
        auth::change_password,
        auth::delete_session,
        exchange::aggregate_converted_handler,
        currencies::list_currencies,
        categories::create_category,
        categories::list_categories,
        categories::rename_category,
        categories::delete_category,
        schedule::compute_next_due,
        subscriptions::create_subscription,
        subscriptions::list_subscriptions,
        subscriptions::update_subscription,
        payment_methods::create_payment_method,
        payment_methods::list_payment_methods,
        payment_methods::rename_payment_method,
        payment_methods::delete_payment_method
    ),
    components(schemas(
        HealthResponse,
        Problem,
        CreateAccountRequest,
        CreateSessionRequest,
        CreateDeviceSessionRequest,
        DeviceToken,
        DeviceSummary,
        ChangePasswordRequest,
        CurrentUser,
        MoneyInput,
        AggregateRequest,
        ConvertedTotalResponse,
        CurrencyDto,
        CategoryDto,
        CreateCategoryRequest,
        RenameCategoryRequest,
        NextDueRequest,
        NextDueResponse,
        CreateSubscriptionRequest,
        SubscriptionDto,
        SubscriptionListResponse,
        PaymentMethodDto,
        CreatePaymentMethodRequest,
        RenamePaymentMethodRequest
    ))
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

/// Sérialise un [`Problem`] en réponse `application/problem+json`.
#[requirement(REQ-SEC-002)]
fn problem_response(status: StatusCode, body: Problem) -> Response {
    let payload = serde_json::to_vec(&body).unwrap_or_default();
    (
        status,
        [(header::CONTENT_TYPE, "application/problem+json")],
        payload,
    )
        .into_response()
}

/// Réponse par défaut : toute route inconnue renvoie une erreur RFC 9457.
#[requirement(REQ-SEC-002)]
async fn not_found(uri: Uri) -> Response {
    let body = problem(StatusCode::NOT_FOUND.as_u16(), "about:blank", "Not Found")
        .with_instance(uri.path().to_string());
    problem_response(StatusCode::NOT_FOUND, body)
}

/// Construit le routeur sans état : uniquement les routes publiques indépendantes de la base
/// (santé, repli). Utilisé par les tests qui n'ont pas besoin de PostgreSQL.
#[requirement(REQ-OPS-001)]
pub fn app() -> Router {
    let (router, _api) = OpenApiRouter::new()
        .routes(routes!(api_v1_health))
        .split_for_parts();
    Router::new().nest("/api/v1", router).fallback(not_found)
}

/// Construit le routeur complet, avec état (base de données) : santé + création de compte.
#[requirement(REQ-AUT-001)]
pub fn app_with_db(db: Db) -> Router {
    let (router, _api) = OpenApiRouter::new()
        .routes(routes!(api_v1_health))
        .routes(routes!(accounts::create_account))
        .routes(routes!(auth::create_session))
        .routes(routes!(auth::create_device_session))
        .routes(routes!(auth::get_current_user))
        .routes(routes!(auth::list_devices))
        .routes(routes!(auth::revoke_device))
        .routes(routes!(auth::change_password))
        .routes(routes!(auth::delete_session))
        .routes(routes!(exchange::aggregate_converted_handler))
        .routes(routes!(currencies::list_currencies))
        .routes(routes!(
            categories::create_category,
            categories::list_categories
        ))
        .routes(routes!(
            categories::rename_category,
            categories::delete_category
        ))
        .routes(routes!(schedule::compute_next_due))
        .routes(routes!(
            subscriptions::create_subscription,
            subscriptions::list_subscriptions
        ))
        .routes(routes!(subscriptions::update_subscription))
        .routes(routes!(
            payment_methods::create_payment_method,
            payment_methods::list_payment_methods
        ))
        .routes(routes!(
            payment_methods::rename_payment_method,
            payment_methods::delete_payment_method
        ))
        .split_for_parts();
    Router::new()
        .nest("/api/v1", router.with_state(db))
        .fallback(not_found)
}
