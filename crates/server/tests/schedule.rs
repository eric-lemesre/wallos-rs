//! Tests d'intégration du calcul d'échéance (REQ-SUB-012).
//!
//! `POST /schedule/next-due` : calcul sans état (ancrage+clamp, ADR 0022). Auth requise.

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

async fn post(
    pool: &PgPool,
    uri: &str,
    body: serde_json::Value,
    cookie: Option<&str>,
) -> axum::http::Response<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    app(pool.clone())
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn account(pool: &PgPool, email: &str) -> String {
    assert_eq!(
        post(
            pool,
            "/api/v1/accounts",
            json!({ "email": email, "password": PASSWORD }),
            None
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let r = post(
        pool,
        "/api/v1/sessions",
        json!({ "email": email, "password": PASSWORD }),
        None,
    )
    .await;
    r.headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn next_due(
    pool: &PgPool,
    cookie: Option<&str>,
    anchor: &str,
    interval: u32,
    after: &str,
) -> axum::http::Response<Body> {
    post(
        pool,
        "/api/v1/schedule/next-due",
        json!({
            "first_payment": anchor,
            "cycle": { "unit": "month", "interval": interval },
            "after": after
        }),
        cookie,
    )
    .await
}

async fn next_payment(r: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["next_payment"].as_str().unwrap().to_string()
}

// --- Fonctionnel (ancrage + clamp, ADR 0022) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn end_of_month_clamps_then_returns_to_31(pool: PgPool) {
    let web = account(&pool, "sched@example.com").await;
    // 31 janv -> 28 févr (clamp).
    let r = next_due(&pool, Some(&web), "2025-01-31", 1, "2025-01-31").await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(next_payment(r).await, "2025-02-28");
    // Depuis le 28 févr -> 31 mars (revient au 31, pas ancré au 28).
    assert_eq!(
        next_payment(next_due(&pool, Some(&web), "2025-01-31", 1, "2025-02-28").await).await,
        "2025-03-31"
    );
    // Bissextile : 31 janv -> 29 févr.
    assert_eq!(
        next_payment(next_due(&pool, Some(&web), "2024-01-31", 1, "2024-01-31").await).await,
        "2024-02-29"
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn invalid_input_is_422(pool: PgPool) {
    let web = account(&pool, "sched-bad@example.com").await;
    // Date mal formée.
    assert_eq!(
        post(&pool, "/api/v1/schedule/next-due", json!({ "first_payment": "31/01/2025", "cycle": { "unit": "month", "interval": 1 }, "after": "2025-01-31" }), Some(&web)).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Intervalle nul.
    assert_eq!(
        next_due(&pool, Some(&web), "2025-01-31", 0, "2025-01-31")
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

// --- Cycles jour/semaine/année (REQ-SUB-013) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-013)]
async fn yearly_and_weekly_via_endpoint(pool: PgPool) {
    let web = account(&pool, "sched-dwy@example.com").await;
    // Année depuis le 29 févr bissextile -> 28 févr (clamp ancré, ADR 0022 ; pas le 1er mars de Wallos).
    let r = post(&pool, "/api/v1/schedule/next-due", json!({
        "first_payment": "2024-02-29", "cycle": { "unit": "year", "interval": 1 }, "after": "2024-02-29"
    }), Some(&web)).await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(next_payment(r).await, "2025-02-28");
    // Hebdomadaire : +7 jours, aucune dérive.
    let r = post(&pool, "/api/v1/schedule/next-due", json!({
        "first_payment": "2025-01-01", "cycle": { "unit": "week", "interval": 1 }, "after": "2025-01-01"
    }), Some(&web)).await;
    assert_eq!(next_payment(r).await, "2025-01-08");
}

// --- Autorisation §9 : computeNextDue (calcul sans état, pas de portée foyer) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn authz_owner_compute_next_due(pool: PgPool) {
    let web = account(&pool, "own-nd@example.com").await;
    assert_eq!(
        next_due(&pool, Some(&web), "2025-01-31", 1, "2025-01-31")
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn authz_other_compute_next_due(pool: PgPool) {
    // Calcul sans état : accessible à tout compte authentifié.
    let web = account(&pool, "other-nd@example.com").await;
    assert_eq!(
        next_due(&pool, Some(&web), "2025-01-31", 1, "2025-01-31")
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn authz_anon_compute_next_due(pool: PgPool) {
    assert_eq!(
        next_due(&pool, None, "2025-01-31", 1, "2025-01-31")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
