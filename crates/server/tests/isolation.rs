//! Tests d'isolation stricte des données entre comptes (REQ-SEC-001).
//!
//! Vérifie au **niveau API** les critères d'acceptation de l'isolation :
//! 1. un compte authentifié qui accède à l'entité d'un autre foyer **par son identifiant** obtient
//!    `404` (jamais `403` ni `200`) et ne la voit pas dans ses listes ;
//! 2. toute opération protégée appelée **sans authentification** obtient `401`.
//!
//! Le 3ᵉ critère (une méthode de repository exige un contexte d'appelant, rendant l'omission non
//! compilable) est garanti par le type `wallos_core::actor::Actor` et couvert dans `crates/core`.
//! La porte `cargo xtask authz-coverage` (3 tests owner/other/anon par `operation_id`) est le
//! garde-fou transversal permanent de cette exigence (AGENTS.md §9).

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use wallos_req_macros::verifies;
use wallos_server::app_with_db;
use wallos_storage::Db;

const PASSWORD: &str = "correct horse battery staple";

fn app(pool: PgPool) -> Router {
    app_with_db(Db::from_pool(pool))
}

async fn post(pool: &PgPool, uri: &str, body: serde_json::Value) -> axum::http::Response<Body> {
    app(pool.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn signup(pool: &PgPool, email: &str) {
    assert_eq!(
        post(
            pool,
            "/api/v1/accounts",
            json!({ "email": email, "password": PASSWORD })
        )
        .await
        .status(),
        StatusCode::CREATED
    );
}

/// Appaire un appareil pour `email` et renvoie son identifiant (lu depuis la liste du propriétaire).
async fn pair_device_id(pool: &PgPool, email: &str) -> String {
    assert_eq!(
        post(
            pool,
            "/api/v1/device-sessions",
            json!({ "email": email, "password": PASSWORD, "label": "A-Laptop", "platform": "desktop" }),
        )
        .await
        .status(),
        StatusCode::OK
    );
    let cookie = login_cookie(pool, email).await;
    let devices = list_devices(pool, Some(&cookie)).await;
    devices[0]["id"].as_str().unwrap().to_string()
}

async fn login_cookie(pool: &PgPool, email: &str) -> String {
    let response = post(
        pool,
        "/api/v1/sessions",
        json!({ "email": email, "password": PASSWORD }),
    )
    .await;
    response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("login sets a session cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// `GET /devices` avec un cookie optionnel ; renvoie le corps JSON (tableau).
async fn list_devices(pool: &PgPool, cookie: Option<&str>) -> Vec<serde_json::Value> {
    let response = send(pool, "GET", "/api/v1/devices", cookie).await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn send(
    pool: &PgPool,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    app(pool.clone())
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SEC-001)]
async fn cross_household_access_by_id_is_not_found(pool: PgPool) {
    signup(&pool, "alice-iso@example.com").await;
    signup(&pool, "bob-iso@example.com").await;

    // Une entité (appareil) appartenant au foyer d'Alice.
    let alice_device = pair_device_id(&pool, "alice-iso@example.com").await;

    // Bob, authentifié, tente d'y accéder PAR SON IDENTIFIANT : 404, jamais 403 ni 204.
    let bob = login_cookie(&pool, "bob-iso@example.com").await;
    let status = send(
        &pool,
        "DELETE",
        &format!("/api/v1/devices/{alice_device}"),
        Some(&bob),
    )
    .await
    .status();
    assert_eq!(status, StatusCode::NOT_FOUND);

    // L'appareil d'Alice n'apparaît pas non plus dans la liste de Bob.
    assert!(list_devices(&pool, Some(&bob)).await.is_empty());

    // Contrôle : l'appareil existe toujours pour Alice (Bob n'a rien pu supprimer).
    let alice = login_cookie(&pool, "alice-iso@example.com").await;
    assert_eq!(list_devices(&pool, Some(&alice)).await.len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SEC-001)]
async fn unauthenticated_access_is_unauthorized(pool: PgPool) {
    signup(&pool, "carol-iso@example.com").await;
    let device = pair_device_id(&pool, "carol-iso@example.com").await;

    // Sans authentification : aucune donnée, 401 (jamais 200/404).
    assert_eq!(
        send(&pool, "GET", "/api/v1/devices", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&pool, "DELETE", &format!("/api/v1/devices/{device}"), None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
