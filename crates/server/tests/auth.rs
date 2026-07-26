//! Tests d'intégration de l'authentification et des sessions (REQ-AUT-002).

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
    let response = post(
        pool,
        "/api/v1/accounts",
        json!({ "email": email, "password": PASSWORD }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn login(pool: &PgPool, email: &str, password: &str) -> axum::http::Response<Body> {
    post(
        pool,
        "/api/v1/sessions",
        json!({ "email": email, "password": password }),
    )
    .await
}

/// Extrait `session=<jeton>` du `Set-Cookie` d'une réponse de login.
fn session_cookie(response: &axum::http::Response<Body>) -> String {
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("login sets a session cookie");
    set_cookie.split(';').next().unwrap().to_string()
}

async fn get_me(pool: &PgPool, cookie: Option<&str>) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method("GET").uri("/api/v1/me");
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    app(pool.clone())
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

// --- Parcours fonctionnels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-002)]
async fn login_opens_session_and_grants_access_to_own_data(pool: PgPool) {
    signup(&pool, "alice@example.com").await;
    let logged_in = login(&pool, "alice@example.com", PASSWORD).await;
    assert_eq!(logged_in.status(), StatusCode::OK);

    let cookie = session_cookie(&logged_in);
    // Le cookie porte les attributs de sécurité.
    let raw = logged_in
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(raw.contains("HttpOnly") && raw.contains("Secure") && raw.contains("SameSite=Lax"));

    let me = get_me(&pool, Some(&cookie)).await;
    assert_eq!(me.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(me.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["email"], "alice@example.com");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-002)]
async fn wrong_password_and_absent_account_are_indistinguishable(pool: PgPool) {
    signup(&pool, "bob@example.com").await;

    let wrong = login(&pool, "bob@example.com", "wrong password entirely").await;
    let absent = login(&pool, "ghost@example.com", "wrong password entirely").await;

    // Même statut ET même corps : aucun signal ne distingue les deux cas.
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(absent.status(), StatusCode::UNAUTHORIZED);
    let wb = axum::body::to_bytes(wrong.into_body(), usize::MAX)
        .await
        .unwrap();
    let ab = axum::body::to_bytes(absent.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(wb, ab);
}

// --- Autorisation §9 : createSession (public) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-002)]
async fn authz_owner_create_session(pool: PgPool) {
    signup(&pool, "owner@example.com").await;
    assert_eq!(
        login(&pool, "owner@example.com", PASSWORD).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-002)]
async fn authz_other_create_session(pool: PgPool) {
    signup(&pool, "other@example.com").await;
    assert_eq!(
        login(&pool, "other@example.com", PASSWORD).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-002)]
async fn authz_anon_create_session(pool: PgPool) {
    // Endpoint public : appelable sans session préalable.
    signup(&pool, "anon@example.com").await;
    assert_eq!(
        login(&pool, "anon@example.com", PASSWORD).await.status(),
        StatusCode::OK
    );
}

// --- Autorisation §9 : getCurrentUser (protégé) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-002)]
async fn authz_owner_get_current_user(pool: PgPool) {
    signup(&pool, "self@example.com").await;
    let cookie = session_cookie(&login(&pool, "self@example.com", PASSWORD).await);
    assert_eq!(get_me(&pool, Some(&cookie)).await.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-002)]
async fn authz_other_get_current_user(pool: PgPool) {
    // Un autre compte authentifié ne voit que SES propres données (isolation par foyer).
    signup(&pool, "first@example.com").await;
    signup(&pool, "second@example.com").await;
    let cookie = session_cookie(&login(&pool, "second@example.com", PASSWORD).await);
    let me = get_me(&pool, Some(&cookie)).await;
    assert_eq!(me.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(me.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["email"], "second@example.com");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-002)]
async fn authz_anon_get_current_user(pool: PgPool) {
    // Sans cookie de session : 401.
    assert_eq!(get_me(&pool, None).await.status(), StatusCode::UNAUTHORIZED);
}
