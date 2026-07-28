//! Tests d'intégration du référentiel des devises (REQ-CUR-007).
//!
//! `GET /currencies` expose le référentiel supporté (donnée globale). Auth requise.

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

async fn post_json(
    pool: &PgPool,
    uri: &str,
    body: serde_json::Value,
) -> axum::http::Response<Body> {
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
    let response = post_json(
        pool,
        "/api/v1/accounts",
        json!({ "email": email, "password": PASSWORD }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn login_cookie(pool: &PgPool, email: &str) -> String {
    let response = post_json(
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

async fn get_currencies(pool: &PgPool, cookie: Option<&str>) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method("GET").uri("/api/v1/currencies");
    if let Some(c) = cookie {
        builder = builder.header(header::COOKIE, c);
    }
    app(pool.clone())
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

// --- Fonctionnel ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-007)]
async fn lists_supported_currencies_with_symbol_and_decimals(pool: PgPool) {
    signup(&pool, "cur@example.com").await;
    let web = login_cookie(&pool, "cur@example.com").await;

    let res = get_currencies(&pool, Some(&web)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    // Référentiel capturé : 34 devises.
    assert_eq!(list.len(), 34);
    let eur = list.iter().find(|c| c["code"] == "EUR").unwrap();
    assert_eq!(eur["symbol"], "€");
    assert_eq!(eur["decimals"], 2);
    let jpy = list.iter().find(|c| c["code"] == "JPY").unwrap();
    assert_eq!(jpy["decimals"], 0);
}

// --- Autorisation §9 : listCurrencies (protégé ; donnée globale, pas de portée foyer) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-007)]
async fn authz_owner_list_currencies(pool: PgPool) {
    signup(&pool, "owner-cur@example.com").await;
    let web = login_cookie(&pool, "owner-cur@example.com").await;
    assert_eq!(
        get_currencies(&pool, Some(&web)).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-007)]
async fn authz_other_list_currencies(pool: PgPool) {
    // Donnée de référence globale : un autre compte authentifié y accède aussi.
    signup(&pool, "other-cur@example.com").await;
    let web = login_cookie(&pool, "other-cur@example.com").await;
    assert_eq!(
        get_currencies(&pool, Some(&web)).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-007)]
async fn authz_anon_list_currencies(pool: PgPool) {
    assert_eq!(
        get_currencies(&pool, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
