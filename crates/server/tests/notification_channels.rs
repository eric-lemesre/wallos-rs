//! Tests d'intégration des canaux de notification (REQ-NOT-005).
//!
//! CRUD isolé par foyer (§9) ; validation **anti-SSRF** de l'URL de webhook à l'enregistrement
//! (adresses internes/bouclage refusées, 422) ; **émission sortante** : le cron POST la charge utile
//! JSON documentée vers l'URL configurée. Autorisation §9 : propriétaire 2xx, tiers authentifié isolé,
//! anonyme 401.

use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use wallos_req_macros::verifies;
use wallos_server::{CronToken, app_with_db, app_with_db_and_cron};
use wallos_storage::Db;

const PASSWORD: &str = "correct horse battery staple";
const CRON_SECRET: &str = "test-cron-secret";

fn app(pool: PgPool) -> Router {
    app_with_db(Db::from_pool(pool))
}

async fn send(
    pool: &PgPool,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        builder = builder.header(header::COOKIE, c);
    }
    let body = match body {
        Some(v) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    app(pool.clone())
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap()
}

async fn body_json(resp: axum::http::Response<Body>) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn account(pool: &PgPool, email: &str) -> String {
    assert_eq!(
        send(
            pool,
            "POST",
            "/api/v1/accounts",
            None,
            Some(json!({ "email": email, "password": PASSWORD })),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let r = send(
        pool,
        "POST",
        "/api/v1/sessions",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
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

/// Crée un canal webhook et renvoie la réponse.
async fn create_webhook(
    pool: &PgPool,
    cookie: &str,
    url: &str,
) -> axum::http::Response<Body> {
    send(
        pool,
        "POST",
        "/api/v1/notifications/channels",
        Some(cookie),
        Some(json!({ "kind": "webhook", "config": { "url": url } })),
    )
    .await
}

// --- CRUD fonctionnel ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005, case = "création d'un webhook public, listé puis supprimé")]
async fn webhook_channel_crud(pool: PgPool) {
    let web = account(&pool, "not005-crud@example.com").await;
    let created = create_webhook(&pool, &web, "https://hooks.example.com/abc").await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let dto = body_json(created).await;
    assert_eq!(dto["kind"], "webhook");
    assert_eq!(dto["config"]["url"], "https://hooks.example.com/abc");
    assert_eq!(dto["enabled"], true);
    let id = dto["id"].as_str().unwrap().to_string();

    // Listé.
    let list = body_json(
        send(
            &pool,
            "GET",
            "/api/v1/notifications/channels",
            Some(&web),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(list["channels"].as_array().unwrap().len(), 1);

    // Supprimé (204), puis absent.
    let del = send(
        &pool,
        "DELETE",
        &format!("/api/v1/notifications/channels/{id}"),
        Some(&web),
        None,
    )
    .await;
    assert_eq!(del.status(), StatusCode::NO_CONTENT);
    let list = body_json(
        send(
            &pool,
            "GET",
            "/api/v1/notifications/channels",
            Some(&web),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(list["channels"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005, case = "URL interne/bouclage refusée (SSRF) -> 422")]
async fn loopback_and_private_urls_are_rejected(pool: PgPool) {
    let web = account(&pool, "not005-ssrf@example.com").await;
    for bad in [
        "http://localhost/hook",
        "http://127.0.0.1/hook",
        "http://169.254.169.254/latest/meta-data",
        "http://10.0.0.5/hook",
        "http://[::1]/hook",
        "not-a-url",
        "ftp://example.com/x",
    ] {
        let r = create_webhook(&pool, &web, bad).await;
        assert_eq!(
            r.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "devrait refuser {bad}"
        );
    }
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005, case = "type de canal non supporté / config incomplète -> 422")]
async fn unsupported_kind_and_missing_url_are_rejected(pool: PgPool) {
    let web = account(&pool, "not005-bad@example.com").await;
    // Type inconnu.
    let r = send(
        &pool,
        "POST",
        "/api/v1/notifications/channels",
        Some(&web),
        Some(json!({ "kind": "carrier-pigeon", "config": { "url": "https://example.com" } })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // url manquante.
    let r = send(
        &pool,
        "POST",
        "/api/v1/notifications/channels",
        Some(&web),
        Some(json!({ "kind": "webhook", "config": {} })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// --- Émission sortante bout-en-bout (critère #1) ---

/// Démarre un récepteur HTTP local qui capture les charges utiles POST sur `/hook`.
async fn spawn_receiver() -> (String, Arc<Mutex<Vec<Value>>>) {
    let captured: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let state = captured.clone();
    let router = Router::new()
        .route(
            "/hook",
            post(
                |State(s): State<Arc<Mutex<Vec<Value>>>>, Json(v): Json<Value>| async move {
                    s.lock().unwrap().push(v);
                    StatusCode::OK
                },
            ),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (format!("http://{addr}/hook"), captured)
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005, case = "le cron POST la charge utile JSON documentée vers l'URL du webhook")]
async fn cron_posts_payload_to_webhook(pool: PgPool) {
    let web = account(&pool, "not005-send@example.com").await;
    // Abonnement dont l'échéance est à 1 jour de la date de référence (délai par défaut 1) → rappel émis.
    assert_eq!(
        send(
            &pool,
            "POST",
            "/api/v1/subscriptions",
            Some(&web),
            Some(json!({
                "name": "Netflix", "amount": "9.99", "currency": "EUR",
                "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2026-08-07",
                "active": true
            })),
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    // Récepteur local, puis canal webhook créé via l'API avec une URL publique (passe la garde SSRF) et
    // ré-pointé vers le récepteur par SQL direct — l'enregistrement d'une URL de bouclage est justement
    // interdit, on contourne donc la garde uniquement pour tester le CHEMIN D'ENVOI.
    let (receiver_url, captured) = spawn_receiver().await;
    let created = body_json(create_webhook(&pool, &web, "https://hooks.example.com/x").await).await;
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(id)
        .bind(json!({ "url": receiver_url }))
        .execute(&pool)
        .await
        .unwrap();

    // Cron déterministe : as_of 2026-08-06, échéance 2026-08-07 (à 1 jour) → un rappel émis et envoyé.
    let cron = app_with_db_and_cron(
        Db::from_pool(pool.clone()),
        CronToken(Some(CRON_SECRET.to_string())),
    );
    let resp = cron
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/internal/run-reminders?as_of=2026-08-06")
                .header("x-cron-token", CRON_SECRET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["emitted"], 1);

    // Le récepteur a reçu exactement une charge utile, au format documenté.
    let payloads = captured.lock().unwrap().clone();
    assert_eq!(payloads.len(), 1, "une charge utile POST attendue");
    let p = &payloads[0];
    assert_eq!(p["reminder_count"], 1);
    assert_eq!(p["as_of"], "2026-08-06");
    assert_eq!(p["reminders"][0]["name"], "Netflix");
    assert_eq!(p["reminders"][0]["kind"], "payment");
    assert_eq!(p["reminders"][0]["days_until"], 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005, case = "un canal désactivé n'émet aucune requête sortante")]
async fn disabled_channel_sends_nothing(pool: PgPool) {
    let web = account(&pool, "not005-disabled@example.com").await;
    assert_eq!(
        send(
            &pool,
            "POST",
            "/api/v1/subscriptions",
            Some(&web),
            Some(json!({
                "name": "Netflix", "amount": "9.99", "currency": "EUR",
                "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2026-08-07",
                "active": true
            })),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let (receiver_url, captured) = spawn_receiver().await;
    // Canal créé PUIS désactivé + repointé vers le récepteur (via SQL).
    let created = body_json(create_webhook(&pool, &web, "https://hooks.example.com/x").await).await;
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    sqlx::query("update notification_channels set config = $2, enabled = false where id = $1")
        .bind(id)
        .bind(json!({ "url": receiver_url }))
        .execute(&pool)
        .await
        .unwrap();

    let cron = app_with_db_and_cron(
        Db::from_pool(pool.clone()),
        CronToken(Some(CRON_SECRET.to_string())),
    );
    let resp = cron
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/internal/run-reminders?as_of=2026-08-06")
                .header("x-cron-token", CRON_SECRET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["emitted"], 1);
    // Aucun envoi : le canal est désactivé.
    assert!(captured.lock().unwrap().is_empty());
}

// --- Autorisation §9 : createNotificationChannel ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_owner_create_notification_channel(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let r = create_webhook(&pool, &a, "https://hooks.example.com/a").await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_other_create_notification_channel(pool: PgPool) {
    account(&pool, "owner@example.com").await;
    let other = account(&pool, "other@example.com").await;
    // Le tiers crée dans SON foyer : 201 isolé.
    let r = create_webhook(&pool, &other, "https://hooks.example.com/b").await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_anon_create_notification_channel(pool: PgPool) {
    let r = send(
        &pool,
        "POST",
        "/api/v1/notifications/channels",
        None,
        Some(json!({ "kind": "webhook", "config": { "url": "https://hooks.example.com/a" } })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

// --- Autorisation §9 : listNotificationChannels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_owner_list_notification_channels(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    create_webhook(&pool, &a, "https://hooks.example.com/a").await;
    let r = send(
        &pool,
        "GET",
        "/api/v1/notifications/channels",
        Some(&a),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(
        body_json(r).await["channels"].as_array().unwrap().len(),
        1
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_other_list_notification_channels(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    create_webhook(&pool, &a, "https://hooks.example.com/a").await;
    let other = account(&pool, "other@example.com").await;
    // Le tiers ne voit que SON foyer (vide).
    let list = body_json(
        send(
            &pool,
            "GET",
            "/api/v1/notifications/channels",
            Some(&other),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(list["channels"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_anon_list_notification_channels(pool: PgPool) {
    let r = send(
        &pool,
        "GET",
        "/api/v1/notifications/channels",
        None,
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

// --- Autorisation §9 : deleteNotificationChannel ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_owner_delete_notification_channel(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let id = body_json(create_webhook(&pool, &a, "https://hooks.example.com/a").await).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = send(
        &pool,
        "DELETE",
        &format!("/api/v1/notifications/channels/{id}"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_other_delete_notification_channel(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let id = body_json(create_webhook(&pool, &a, "https://hooks.example.com/a").await).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let other = account(&pool, "other@example.com").await;
    // Le tiers ne peut pas supprimer le canal d'autrui : 404 (jamais 403, §9).
    let r = send(
        &pool,
        "DELETE",
        &format!("/api/v1/notifications/channels/{id}"),
        Some(&other),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005)]
async fn authz_anon_delete_notification_channel(pool: PgPool) {
    let r = send(
        &pool,
        "DELETE",
        &format!("/api/v1/notifications/channels/{}", Uuid::new_v4()),
        None,
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
