//! Tests d'intégration de l'authentification et des sessions (REQ-AUT-002).

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
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

/// Comme [`post`], mais en injectant une IP source (via `ConnectInfo`) pour exercer la limitation
/// de taux par IP (REQ-AUT-008) sous `oneshot`, qui ne pose autrement aucune adresse de pair.
async fn post_from_ip(
    pool: &PgPool,
    uri: &str,
    body: serde_json::Value,
    ip: &str,
) -> axum::http::Response<Body> {
    let addr: SocketAddr = format!("{ip}:44444").parse().unwrap();
    let mut request = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(addr));
    app(pool.clone()).oneshot(request).await.unwrap()
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

async fn logout(pool: &PgPool, cookie: Option<&str>) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method("DELETE").uri("/api/v1/sessions");
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

// --- Limitation du taux de tentatives (REQ-AUT-008) ---

/// Extrait et valide l'en-tête `Retry-After` (entier ≥ 1) d'une réponse 429.
fn retry_after_seconds(response: &axum::http::Response<Body>) -> i64 {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .expect("429 carries a numeric Retry-After header")
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-008)]
async fn too_many_failed_attempts_returns_429_including_with_correct_password(pool: PgPool) {
    signup(&pool, "target@example.com").await;

    // Le seuil par défaut est 5 : cinq échecs consécutifs remplissent la fenêtre.
    for _ in 0..5 {
        let attempt = login(&pool, "target@example.com", "not the password").await;
        assert_eq!(attempt.status(), StatusCode::UNAUTHORIZED);
    }

    // Tentative suivante AVEC le bon mot de passe : rejetée en 429 (la limite prime).
    let blocked = login(&pool, "target@example.com", PASSWORD).await;
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(retry_after_seconds(&blocked) >= 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-008)]
async fn rate_limit_applies_per_ip_across_different_accounts(pool: PgPool) {
    const ATTACKER: &str = "203.0.113.7";
    const BYSTANDER: &str = "198.51.100.9";

    // Cinq échecs depuis la même IP, sur cinq comptes DIFFÉRENTS : aucun compte n'atteint le seuil,
    // mais l'IP oui.
    for i in 0..5 {
        let body = json!({ "email": format!("victim{i}@example.com"), "password": "nope" });
        let attempt = post_from_ip(&pool, "/api/v1/sessions", body, ATTACKER).await;
        assert_eq!(attempt.status(), StatusCode::UNAUTHORIZED);
    }

    // Tentative suivante depuis l'IP attaquante : limitée en 429.
    let blocked = post_from_ip(
        &pool,
        "/api/v1/sessions",
        json!({ "email": "victim9@example.com", "password": "nope" }),
        ATTACKER,
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(retry_after_seconds(&blocked) >= 1);

    // Contrôle : une autre IP n'est pas affectée par la limite de l'IP attaquante.
    signup(&pool, "innocent@example.com").await;
    let allowed = post_from_ip(
        &pool,
        "/api/v1/sessions",
        json!({ "email": "innocent@example.com", "password": PASSWORD }),
        BYSTANDER,
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
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

// --- Déconnexion (REQ-AUT-009) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-009)]
async fn logout_invalidates_session_server_side(pool: PgPool) {
    signup(&pool, "logmeout@example.com").await;
    let cookie = session_cookie(&login(&pool, "logmeout@example.com", PASSWORD).await);
    assert_eq!(get_me(&pool, Some(&cookie)).await.status(), StatusCode::OK);

    assert_eq!(
        logout(&pool, Some(&cookie)).await.status(),
        StatusCode::NO_CONTENT
    );
    // Le même cookie ne donne plus accès : invalidation côté serveur.
    assert_eq!(
        get_me(&pool, Some(&cookie)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-009)]
async fn logout_is_idempotent(pool: PgPool) {
    signup(&pool, "twice@example.com").await;
    let cookie = session_cookie(&login(&pool, "twice@example.com", PASSWORD).await);
    assert_eq!(
        logout(&pool, Some(&cookie)).await.status(),
        StatusCode::NO_CONTENT
    );
    // Rejeu sur session déjà invalidée : toujours 204.
    assert_eq!(
        logout(&pool, Some(&cookie)).await.status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-009)]
async fn authz_owner_delete_session(pool: PgPool) {
    signup(&pool, "owner-lo@example.com").await;
    let cookie = session_cookie(&login(&pool, "owner-lo@example.com", PASSWORD).await);
    assert_eq!(
        logout(&pool, Some(&cookie)).await.status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-009)]
async fn authz_other_delete_session(pool: PgPool) {
    signup(&pool, "other-lo@example.com").await;
    let cookie = session_cookie(&login(&pool, "other-lo@example.com", PASSWORD).await);
    assert_eq!(
        logout(&pool, Some(&cookie)).await.status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-009)]
async fn authz_anon_delete_session(pool: PgPool) {
    // Sans cookie : la déconnexion reste 204 (idempotente, endpoint public).
    assert_eq!(logout(&pool, None).await.status(), StatusCode::NO_CONTENT);
}
