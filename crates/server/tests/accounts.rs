//! Tests d'intégration de la création de compte (REQ-AUT-001).
//!
//! `#[sqlx::test]` provisionne une base éphémère ; les migrations vivent dans le crate `storage`.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use wallos_req_macros::verifies;
use wallos_server::app_with_db;
use wallos_storage::Db;

const VALID_PASSWORD: &str = "correct horse battery staple";

fn app(pool: PgPool) -> Router {
    app_with_db(Db::from_pool(pool))
}

async fn post_accounts(router: Router, body: serde_json::Value) -> axum::http::Response<Body> {
    router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-001)]
async fn valid_signup_creates_account_hashed(pool: PgPool) {
    let response = post_accounts(
        app(pool.clone()),
        json!({ "email": "alice@example.com", "password": VALID_PASSWORD }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let (count, hash): (i64, String) =
        sqlx::query_as("select count(*)::int8, coalesce(max(password_hash), '') from users")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    assert!(
        hash.starts_with("$argon2id$"),
        "password must be argon2id-hashed, got {hash}"
    );
    assert!(
        !hash.contains(VALID_PASSWORD),
        "plaintext must never be stored"
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-001)]
async fn duplicate_email_is_indistinguishable(pool: PgPool) {
    let body = json!({ "email": "bob@example.com", "password": VALID_PASSWORD });
    let first = post_accounts(app(pool.clone()), body.clone()).await;
    let second = post_accounts(app(pool.clone()), body).await;
    // Anti-énumération : les deux réponses sont identiques (201), rien ne révèle l'existence.
    assert_eq!(first.status(), StatusCode::CREATED);
    assert_eq!(second.status(), StatusCode::CREATED);

    let count: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "no second account created");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-001)]
async fn short_password_is_rejected(pool: PgPool) {
    let response = post_accounts(
        app(pool),
        json!({ "email": "carol@example.com", "password": "short" }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert_eq!(content_type, "application/problem+json");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-001)]
async fn password_at_exact_minimum_length_is_accepted(pool: PgPool) {
    // Exactement 12 caractères : la borne est inclusive (>= 12), pas stricte.
    let response = post_accounts(
        app(pool),
        json!({ "email": "boundary@example.com", "password": "abcdefghijkl" }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-003)]
async fn compromised_password_is_rejected(pool: PgPool) {
    // Assez long (12) mais figurant dans la liste de compromis -> 422 (REQ-AUT-003).
    let response = post_accounts(
        app(pool),
        json!({ "email": "dave@example.com", "password": "password1234" }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// --- Autorisation §9 : l'inscription est un endpoint PUBLIC (comme getHealth) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-001)]
async fn authz_owner_create_account(pool: PgPool) {
    let response = post_accounts(
        app(pool),
        json!({ "email": "owner@example.com", "password": VALID_PASSWORD }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-001)]
async fn authz_other_create_account(pool: PgPool) {
    // Un autre utilisateur (non authentifié également) accède tout autant : endpoint public.
    let response = post_accounts(
        app(pool),
        json!({ "email": "other@example.com", "password": VALID_PASSWORD }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-001)]
async fn authz_anon_create_account(pool: PgPool) {
    // Sans aucun en-tête d'authentification : accessible (l'inscription précède la session).
    let response = post_accounts(
        app(pool),
        json!({ "email": "anon@example.com", "password": VALID_PASSWORD }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}
