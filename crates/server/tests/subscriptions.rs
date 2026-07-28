//! Tests d'intégration de la création d'abonnements (REQ-SUB-002).

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
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
    body: Value,
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

fn valid_body() -> Value {
    json!({
        "name": "Netflix",
        "amount": "9.99",
        "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": "2030-01-31"
    })
}

async fn create(pool: &PgPool, cookie: &str, body: Value) -> axum::http::Response<Body> {
    post(pool, "/api/v1/subscriptions", body, Some(cookie)).await
}

async fn body_json(r: axum::http::Response<Body>) -> Value {
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// --- Fonctionnel ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-002)]
async fn creates_subscription_with_next_payment(pool: PgPool) {
    let web = account(&pool, "sub@example.com").await;
    let r = create(&pool, &web, valid_body()).await;
    assert_eq!(r.status(), StatusCode::CREATED);
    let body = body_json(r).await;
    assert_eq!(body["name"], "Netflix");
    assert_eq!(body["amount"], "9.99"); // montant en chaîne (R4)
    assert_eq!(body["currency"], "EUR");
    assert_eq!(body["active"], true);
    assert!(body["id"].as_str().is_some());
    // Prochaine échéance calculée immédiatement : first_payment futur -> lui-même.
    assert_eq!(body["next_payment"], "2030-01-31");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-002)]
async fn create_persists_the_row_with_a_household(pool: PgPool) {
    // HIGH-2 : la persistance réelle est vérifiée (pas seulement la réponse HTTP).
    let web = account(&pool, "persist@example.com").await;
    let body = body_json(create(&pool, &web, valid_body()).await).await;
    let id: uuid::Uuid = body["id"].as_str().unwrap().parse().unwrap();

    let row = sqlx::query(
        "select name, amount, currency, cycle_unit, cycle_interval, household_id \
         from subscriptions where id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("name"), "Netflix");
    assert_eq!(
        row.get::<rust_decimal::Decimal, _>("amount"),
        "9.99".parse().unwrap()
    );
    assert_eq!(row.get::<String, _>("currency"), "EUR");
    assert_eq!(row.get::<String, _>("cycle_unit"), "month");
    assert_eq!(row.get::<i32, _>("cycle_interval"), 1);
    // Rattaché à un foyer (isolation §9 : la ligne porte un `household_id` non nul).
    let _household: uuid::Uuid = row.get("household_id");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-002)]
async fn unknown_cycle_unit_and_empty_name_rejected_per_field(pool: PgPool) {
    let web = account(&pool, "sub-fields@example.com").await;
    let mut unit = valid_body();
    unit["cycle"]["unit"] = json!("fortnight");
    let r = create(&pool, &web, unit).await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body_json(r).await["detail"]
            .as_str()
            .unwrap()
            .contains("cycle.unit")
    );

    let mut empty = valid_body();
    empty["name"] = json!("   ");
    let r = create(&pool, &web, empty).await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body_json(r).await["detail"]
            .as_str()
            .unwrap()
            .contains("name")
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-002)]
async fn negative_amount_is_rejected_per_field(pool: PgPool) {
    let web = account(&pool, "sub-neg@example.com").await;
    let mut b = valid_body();
    b["amount"] = json!("-5.00");
    let r = create(&pool, &web, b).await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // Erreur par champ : le détail identifie `amount`.
    assert!(
        body_json(r).await["detail"]
            .as_str()
            .unwrap()
            .contains("amount")
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-002)]
async fn unknown_currency_is_rejected_per_field(pool: PgPool) {
    let web = account(&pool, "sub-cur@example.com").await;
    let mut b = valid_body();
    b["currency"] = json!("ZZZ");
    let r = create(&pool, &web, b).await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body_json(r).await["detail"]
            .as_str()
            .unwrap()
            .contains("currency")
    );
}

// --- Autorisation §9 : createSubscription ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-002)]
async fn authz_owner_create_subscription(pool: PgPool) {
    let web = account(&pool, "own-sub@example.com").await;
    assert_eq!(
        create(&pool, &web, valid_body()).await.status(),
        StatusCode::CREATED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-002)]
async fn authz_other_create_subscription(pool: PgPool) {
    // Chaque compte crée ses propres abonnements.
    let web = account(&pool, "other-sub@example.com").await;
    assert_eq!(
        create(&pool, &web, valid_body()).await.status(),
        StatusCode::CREATED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-002)]
async fn authz_anon_create_subscription(pool: PgPool) {
    assert_eq!(
        post(&pool, "/api/v1/subscriptions", valid_body(), None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
