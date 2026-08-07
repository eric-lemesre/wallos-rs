//! Tests d'intégration du réessai et de l'abandon des livraisons (REQ-NOT-007).
//!
//! Pattern *outbox* : le suivi est ouvert AVANT l'envoi (revue NOT-002 F1/F2 — pas de perte
//! silencieuse sur crash) et refermé au succès. Un échec laisse une ligne `pending` réessayée à
//! intervalle croissant (politique pure `wallos_core::retry_delay_minutes`), abandonnée — et
//! **visible** — une fois la borne atteinte. Autorisation §9 sur la liste.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::routing::post;
use axum::{Json, routing::any};
use chrono::{Duration, Utc};
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

/// Abonnement mensuel actif échéant demain (délai par défaut 1 → rappel dû aujourd'hui).
async fn subscription_due_tomorrow(pool: &PgPool, cookie: &str) {
    let tomorrow = (Utc::now().date_naive() + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let r = send(
        pool,
        "POST",
        "/api/v1/subscriptions",
        Some(cookie),
        Some(json!({
            "name": "Netflix", "amount": "9.99", "currency": "EUR",
            "cycle": { "unit": "month", "interval": 1 }, "first_payment": tomorrow,
            "active": true
        })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

/// Canal webhook créé via l'API puis repointé vers `url` par SQL (la garde SSRF interdit le
/// bouclage à l'enregistrement). Renvoie l'UUID du canal.
async fn webhook_to(pool: &PgPool, cookie: &str, url: &str) -> Uuid {
    let r = send(
        pool,
        "POST",
        "/api/v1/notifications/channels",
        Some(cookie),
        Some(json!({ "kind": "webhook", "config": { "url": "https://hooks.example.com/x" } })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    let id = Uuid::parse_str(body_json(r).await["id"].as_str().unwrap()).unwrap();
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(id)
        .bind(json!({ "url": url }))
        .execute(pool)
        .await
        .unwrap();
    id
}

/// Récepteur local comptant les POST reçus sur `/hook`.
async fn spawn_counting_receiver() -> (String, Arc<Mutex<usize>>) {
    let count: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let state = count.clone();
    let router = Router::new()
        .route(
            "/hook",
            post(
                |State(s): State<Arc<Mutex<usize>>>, Json(_): Json<Value>| async move {
                    *s.lock().unwrap() += 1;
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
    (format!("http://{addr}/hook"), count)
}

/// Récepteur local qui répond toujours 500 (échec d'envoi contrôlé).
async fn spawn_failing_receiver() -> String {
    let router = Router::new().fallback(any(|| async { StatusCode::INTERNAL_SERVER_ERROR }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://{addr}/hook")
}

/// Lance le cron (application neuve à chaque appel) avec la date de référence du jour.
async fn run_cron(pool: &PgPool) -> Value {
    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let cron = app_with_db_and_cron(
        Db::from_pool(pool.clone()),
        CronToken(Some(CRON_SECRET.to_string())),
    );
    let resp = cron
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/internal/run-reminders?as_of={today}"))
                .header("x-cron-token", CRON_SECRET)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    body_json(resp).await
}

/// Liste les livraisons du foyer via l'API.
async fn list_deliveries(pool: &PgPool, cookie: &str) -> Value {
    body_json(
        send(
            pool,
            "GET",
            "/api/v1/notifications/deliveries",
            Some(cookie),
            None,
        )
        .await,
    )
    .await
}

// --- Cycle de vie d'une livraison ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-007, case = "un envoi réussi ne laisse aucun suivi (outbox refermée au nominal)")]
async fn successful_send_leaves_no_delivery(pool: PgPool) {
    let cookie = account(&pool, "not007-ok@example.com").await;
    subscription_due_tomorrow(&pool, &cookie).await;
    let (url, count) = spawn_counting_receiver().await;
    webhook_to(&pool, &cookie, &url).await;

    let resp = run_cron(&pool).await;
    assert_eq!(resp["emitted"], 1);
    assert_eq!(*count.lock().unwrap(), 1);
    let list = list_deliveries(&pool, &cookie).await;
    assert_eq!(list["deliveries"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-007, case = "un échec d'envoi ouvre un suivi `pending` avec échéance de réessai, visible via l'API")]
async fn failed_send_opens_pending_delivery(pool: PgPool) {
    let cookie = account(&pool, "not007-fail@example.com").await;
    subscription_due_tomorrow(&pool, &cookie).await;
    let failing = spawn_failing_receiver().await;
    webhook_to(&pool, &cookie, &failing).await;

    let resp = run_cron(&pool).await;
    assert_eq!(resp["emitted"], 1);

    let list = list_deliveries(&pool, &cookie).await;
    let deliveries = list["deliveries"].as_array().unwrap();
    assert_eq!(deliveries.len(), 1);
    let d = &deliveries[0];
    assert_eq!(d["status"], "pending");
    assert_eq!(d["attempts"], 1);
    assert_eq!(d["channel_kind"], "webhook");
    assert_eq!(d["last_code"], "http-status");
    assert!(
        d["next_attempt_at"].is_string(),
        "échéance de réessai posée"
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-007, case = "le réessai attend son échéance, puis rejoue le lot ; le succès referme le suivi")]
async fn retry_waits_deadline_then_resolves_on_success(pool: PgPool) {
    let cookie = account(&pool, "not007-retry@example.com").await;
    subscription_due_tomorrow(&pool, &cookie).await;
    let failing = spawn_failing_receiver().await;
    let channel_id = webhook_to(&pool, &cookie, &failing).await;

    assert_eq!(run_cron(&pool).await["emitted"], 1);
    assert_eq!(
        list_deliveries(&pool, &cookie).await["deliveries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Avant l'échéance : aucune tentative (l'intervalle croissant est respecté).
    let resp = run_cron(&pool).await;
    assert_eq!(resp["retried"], 0);

    // Le canal redevient joignable et l'échéance est atteinte (SQL : le temps ne se simule pas).
    let (ok_url, count) = spawn_counting_receiver().await;
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(channel_id)
        .bind(json!({ "url": ok_url }))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("update notification_deliveries set next_attempt_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();

    let resp = run_cron(&pool).await;
    assert_eq!(resp["retried"], 1);
    assert_eq!(resp["abandoned"], 0);
    assert_eq!(*count.lock().unwrap(), 1, "le lot raté a été rejoué");
    // Suivi refermé : plus rien à signaler.
    let list = list_deliveries(&pool, &cookie).await;
    assert_eq!(list["deliveries"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-007, case = "borne de tentatives atteinte -> abandon VISIBLE dans l'interface, plus aucun réessai")]
async fn exhausted_retries_abandon_visibly(pool: PgPool) {
    let cookie = account(&pool, "not007-abandon@example.com").await;
    subscription_due_tomorrow(&pool, &cookie).await;
    let failing = spawn_failing_receiver().await;
    webhook_to(&pool, &cookie, &failing).await;
    assert_eq!(run_cron(&pool).await["emitted"], 1);

    // Accélère l'histoire : 4 tentatives déjà consommées, échéance atteinte (SQL).
    sqlx::query(
        "update notification_deliveries \
         set attempts = 4, next_attempt_at = now() - interval '1 second'",
    )
    .execute(&pool)
    .await
    .unwrap();

    // 5e tentative : échec → borne atteinte → abandon.
    let resp = run_cron(&pool).await;
    assert_eq!(resp["retried"], 1);
    assert_eq!(resp["abandoned"], 1);

    // Visible par l'utilisateur (critère #2), avec le diagnostic.
    let list = list_deliveries(&pool, &cookie).await;
    let deliveries = list["deliveries"].as_array().unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0]["status"], "abandoned");
    assert_eq!(deliveries[0]["attempts"], 5);
    assert!(deliveries[0]["next_attempt_at"].is_null());

    // Plus aucun réessai : l'abandon est terminal.
    let resp = run_cron(&pool).await;
    assert_eq!(resp["retried"], 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-007, case = "un canal désactivé n'est jamais réessayé (aucune requête sortante, REQ-NOT-004)")]
async fn disabled_channel_is_not_retried(pool: PgPool) {
    let cookie = account(&pool, "not007-disabled@example.com").await;
    subscription_due_tomorrow(&pool, &cookie).await;
    let failing = spawn_failing_receiver().await;
    let channel_id = webhook_to(&pool, &cookie, &failing).await;
    assert_eq!(run_cron(&pool).await["emitted"], 1);

    // Canal désactivé, échéance atteinte : le réessai doit l'ignorer.
    sqlx::query("update notification_channels set enabled = false where id = $1")
        .bind(channel_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("update notification_deliveries set next_attempt_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();

    let resp = run_cron(&pool).await;
    assert_eq!(resp["retried"], 0);
    // Le suivi reste `pending` : un canal ré-activé reprendra le rythme.
    let list = list_deliveries(&pool, &cookie).await;
    assert_eq!(list["deliveries"][0]["status"], "pending");
}

// --- Autorisation §9 (listNotificationDeliveries) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-007)]
async fn authz_owner_list_notification_deliveries(pool: PgPool) {
    let cookie = account(&pool, "not007-authz-owner@example.com").await;
    let r = send(
        &pool,
        "GET",
        "/api/v1/notifications/deliveries",
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-007)]
async fn authz_other_list_notification_deliveries(pool: PgPool) {
    let owner = account(&pool, "not007-authz-o@example.com").await;
    subscription_due_tomorrow(&pool, &owner).await;
    let failing = spawn_failing_receiver().await;
    webhook_to(&pool, &owner, &failing).await;
    assert_eq!(run_cron(&pool).await["emitted"], 1);

    // Le tiers ne voit que SON foyer (vide) — jamais les livraisons d'autrui.
    let other = account(&pool, "not007-authz-other@example.com").await;
    let list = list_deliveries(&pool, &other).await;
    assert_eq!(list["deliveries"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-007)]
async fn authz_anon_list_notification_deliveries(pool: PgPool) {
    let r = send(&pool, "GET", "/api/v1/notifications/deliveries", None, None).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
