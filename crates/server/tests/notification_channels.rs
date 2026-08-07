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
async fn create_webhook(pool: &PgPool, cookie: &str, url: &str) -> axum::http::Response<Body> {
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

/// Récepteur qui **redirige** (307) `/redirect` vers `/internal`, et note tout accès à `/internal`.
/// Sert à prouver que l'envoi ne suit PAS les redirections (anti-SSRF).
async fn spawn_redirecting_receiver() -> (String, Arc<Mutex<bool>>) {
    let internal_hit: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let state = internal_hit.clone();
    // On construit l'URL `/internal` absolue une fois le port connu ; d'abord on réserve le port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let internal_url = format!("http://{addr}/internal");
    let router = Router::new()
        .route(
            "/redirect",
            post({
                let loc = internal_url.clone();
                move || {
                    let loc = loc.clone();
                    async move {
                        (
                            StatusCode::TEMPORARY_REDIRECT,
                            [(axum::http::header::LOCATION, loc)],
                        )
                    }
                }
            }),
        )
        .route(
            "/internal",
            post(|State(s): State<Arc<Mutex<bool>>>| async move {
                *s.lock().unwrap() = true;
                StatusCode::OK
            }),
        )
        .with_state(state);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (format!("http://{addr}/redirect"), internal_hit)
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-005, case = "l'envoi ne suit pas les redirections (anti-SSRF au niveau du chemin d'appel)")]
async fn webhook_send_does_not_follow_redirects(pool: PgPool) {
    let web = account(&pool, "not005-redirect@example.com").await;
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
    let (redirect_url, internal_hit) = spawn_redirecting_receiver().await;
    let created = body_json(create_webhook(&pool, &web, "https://hooks.example.com/x").await).await;
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(id)
        .bind(json!({ "url": redirect_url }))
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
    // Le cron réussit (envoi best-effort) mais la redirection n'est PAS suivie : `/internal` jamais touché.
    assert_eq!(body_json(resp).await["emitted"], 1);
    assert!(
        !*internal_hit.lock().unwrap(),
        "la redirection vers une cible interne ne doit pas être suivie"
    );
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

// --- Canal e-mail (REQ-NOT-003) ---

/// Crée un canal e-mail avec une configuration SMTP donnée.
async fn create_email(pool: &PgPool, cookie: &str, config: Value) -> axum::http::Response<Body> {
    send(
        pool,
        "POST",
        "/api/v1/notifications/channels",
        Some(cookie),
        Some(json!({ "kind": "email", "config": config })),
    )
    .await
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-003, case = "création d'un canal e-mail valide ; le mot de passe SMTP est redacté en réponse")]
async fn email_channel_create_redacts_password(pool: PgPool) {
    let web = account(&pool, "not003-create@example.com").await;
    let created = create_email(
        &pool,
        &web,
        json!({
            "host": "smtp.example.com", "port": 587,
            "username": "alice", "password": "s3cr3t",
            "from": "wallos@example.com"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let dto = body_json(created).await;
    assert_eq!(dto["kind"], "email");
    assert_eq!(dto["config"]["host"], "smtp.example.com");
    assert_eq!(dto["config"]["from"], "wallos@example.com");
    // Le secret n'est JAMAIS renvoyé (REQ-NOT-003 « sans exposer les identifiants »).
    assert_eq!(dto["config"]["password"], "<redacted>");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-003, case = "configuration SMTP invalide (champ manquant / expéditeur illisible) -> 422")]
async fn email_channel_invalid_config_rejected(pool: PgPool) {
    let web = account(&pool, "not003-bad@example.com").await;
    // host manquant.
    assert_eq!(
        create_email(
            &pool,
            &web,
            json!({ "port": 587, "username": "a", "password": "b", "from": "w@example.com" })
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // port hors bornes.
    assert_eq!(
        create_email(&pool, &web, json!({ "host": "smtp.example.com", "port": 0, "username": "a", "password": "b", "from": "w@example.com" }))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // from illisible.
    assert_eq!(
        create_email(&pool, &web, json!({ "host": "smtp.example.com", "port": 587, "username": "a", "password": "b", "from": "pas-une-adresse" }))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-003, case = "un canal e-mail défaillant n'interrompt pas les autres canaux ni le cron")]
async fn failing_email_channel_does_not_interrupt_other_channels(pool: PgPool) {
    let web = account(&pool, "not003-resilient@example.com").await;
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

    // Un webhook vers un récepteur local + un canal e-mail vers un SMTP injoignable (port 1, refusé).
    let (receiver_url, captured) = spawn_receiver().await;
    let wh = body_json(create_webhook(&pool, &web, "https://hooks.example.com/x").await).await;
    let wh_id = Uuid::parse_str(wh["id"].as_str().unwrap()).unwrap();
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(wh_id)
        .bind(json!({ "url": receiver_url }))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        create_email(
            &pool,
            &web,
            json!({
                "host": "127.0.0.1", "port": 1, "username": "a", "password": "b",
                "from": "wallos@example.com", "starttls": false
            }),
        )
        .await
        .status(),
        StatusCode::CREATED
    );

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
    // Le cron réussit malgré l'échec du canal e-mail (best-effort)...
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["emitted"], 1);
    // ...et le webhook a bien reçu sa charge utile (l'échec e-mail ne l'a pas interrompu).
    assert_eq!(captured.lock().unwrap().len(), 1);
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
    assert_eq!(body_json(r).await["channels"].as_array().unwrap().len(), 1);
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
    let r = send(&pool, "GET", "/api/v1/notifications/channels", None, None).await;
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

// --- Canaux de messagerie tiers (REQ-NOT-004) ---

/// Crée un canal d'un type donné avec sa configuration.
async fn create_channel(
    pool: &PgPool,
    cookie: &str,
    kind: &str,
    config: Value,
) -> axum::http::Response<Body> {
    send(
        pool,
        "POST",
        "/api/v1/notifications/channels",
        Some(cookie),
        Some(json!({ "kind": kind, "config": config })),
    )
    .await
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-004, case = "création des canaux telegram/discord/gotify/pushover ; secrets redactés en réponse et en liste")]
async fn messaging_channels_crud_redacts_secrets(pool: PgPool) {
    let web = account(&pool, "not004-crud@example.com").await;

    let telegram = body_json(
        create_channel(
            &pool,
            &web,
            "telegram",
            json!({ "bot_token": "123:s3cr3t", "chat_id": "42" }),
        )
        .await,
    )
    .await;
    assert_eq!(telegram["kind"], "telegram");
    assert_eq!(telegram["config"]["bot_token"], "<redacted>");
    assert_eq!(telegram["config"]["chat_id"], "42");

    let discord = body_json(
        create_channel(
            &pool,
            &web,
            "discord",
            json!({ "url": "https://discord.com/api/webhooks/1/x", "username": "Wallos" }),
        )
        .await,
    )
    .await;
    assert_eq!(discord["kind"], "discord");
    assert_eq!(
        discord["config"]["url"],
        "https://discord.com/api/webhooks/1/x"
    );
    assert_eq!(discord["config"]["username"], "Wallos");

    let gotify = body_json(
        create_channel(
            &pool,
            &web,
            "gotify",
            json!({ "url": "https://gotify.example.com", "token": "app-s3cr3t" }),
        )
        .await,
    )
    .await;
    assert_eq!(gotify["kind"], "gotify");
    assert_eq!(gotify["config"]["token"], "<redacted>");

    let pushover = body_json(
        create_channel(
            &pool,
            &web,
            "pushover",
            json!({ "user_key": "uk-s3cr3t", "token": "tok-s3cr3t" }),
        )
        .await,
    )
    .await;
    assert_eq!(pushover["kind"], "pushover");
    assert_eq!(pushover["config"]["user_key"], "<redacted>");
    assert_eq!(pushover["config"]["token"], "<redacted>");

    // La liste redacte de la même façon (aucun secret ne sort jamais).
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
    let channels = list["channels"].as_array().unwrap();
    assert_eq!(channels.len(), 4);
    let listed = serde_json::to_string(&list).unwrap();
    for secret in ["s3cr3t", "uk-s3cr3t", "tok-s3cr3t", "app-s3cr3t"] {
        assert!(
            !listed.contains(secret),
            "le secret {secret} ne doit jamais être renvoyé"
        );
    }
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-004, case = "configuration incomplète ou URL interne (SSRF) -> 422 pour chaque messagerie")]
async fn messaging_invalid_configs_are_rejected(pool: PgPool) {
    let web = account(&pool, "not004-bad@example.com").await;
    let cases: Vec<(&str, Value)> = vec![
        // Champs requis manquants ou vides (oracle legacy : « fill mandatory fields »).
        ("telegram", json!({ "chat_id": "42" })),
        ("telegram", json!({ "bot_token": "", "chat_id": "42" })),
        ("telegram", json!({ "bot_token": "123:abc" })),
        ("discord", json!({})),
        ("gotify", json!({ "url": "https://gotify.example.com" })),
        ("gotify", json!({ "token": "abc" })),
        ("pushover", json!({ "user_key": "uk" })),
        ("pushover", json!({ "token": "tok" })),
        // Jeton Telegram mal formé (interpolé dans le chemin de l'URL de l'API Bot, revue F2).
        (
            "telegram",
            json!({ "bot_token": "123:abc/def", "chat_id": "42" }),
        ),
        (
            "telegram",
            json!({ "bot_token": "pas-un-jeton", "chat_id": "42" }),
        ),
        // Champs blancs refusés (revue F7).
        ("pushover", json!({ "user_key": "   ", "token": "tok" })),
        // Avatar Discord : URL http(s) analysable exigée (revue F8).
        (
            "discord",
            json!({ "url": "https://discord.com/api/webhooks/1/x", "avatar_url": "javascript:alert(1)" }),
        ),
        // URL utilisateur interne/bouclage refusée (même garde SSRF que le webhook).
        ("discord", json!({ "url": "http://127.0.0.1/hook" })),
        ("discord", json!({ "url": "http://169.254.169.254/latest" })),
        (
            "gotify",
            json!({ "url": "http://localhost:8080", "token": "abc" }),
        ),
        (
            "gotify",
            json!({ "url": "http://10.0.0.5", "token": "abc" }),
        ),
    ];
    for (kind, config) in cases {
        let r = create_channel(&pool, &web, kind, config.clone()).await;
        assert_eq!(
            r.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "devrait refuser kind={kind} config={config}"
        );
    }
}

/// Requête sortante capturée par le récepteur multi-canaux.
#[derive(Debug, Clone)]
struct CapturedRequest {
    path: String,
    content_type: String,
    gotify_key: Option<String>,
    body: String,
}

/// Récepteur HTTP local qui capture **toute** requête POST (chemin, type de contenu, en-tête
/// `X-Gotify-Key`, corps brut) — sert à vérifier les formats propres à chaque messagerie.
async fn spawn_capture_receiver() -> (String, Arc<Mutex<Vec<CapturedRequest>>>) {
    async fn capture(
        State(s): State<Arc<Mutex<Vec<CapturedRequest>>>>,
        req: axum::extract::Request,
    ) -> StatusCode {
        let path = req.uri().path().to_string();
        let content_type = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let gotify_key = req
            .headers()
            .get("x-gotify-key")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let bytes = to_bytes(req.into_body(), usize::MAX).await.unwrap();
        s.lock().unwrap().push(CapturedRequest {
            path,
            content_type,
            gotify_key,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        });
        StatusCode::OK
    }
    let captured: Arc<Mutex<Vec<CapturedRequest>>> = Arc::new(Mutex::new(Vec::new()));
    let state = captured.clone();
    let router = Router::new().fallback(capture).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (format!("http://{addr}"), captured)
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-004, case = "les quatre messageries reçoivent le même message via leur adaptateur propre")]
async fn cron_sends_to_all_messaging_channels(pool: PgPool) {
    let web = account(&pool, "not004-send@example.com").await;
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

    // Un récepteur unique ; chaque canal est repointé vers lui par SQL direct (la garde SSRF interdit
    // 127.0.0.1 à l'enregistrement ; `api_base` n'est posable QUE par SQL, jamais via l'API).
    let (base, captured) = spawn_capture_receiver().await;
    let reroutes: Vec<(&str, Value, Value)> = vec![
        (
            "telegram",
            json!({ "bot_token": "123:abc", "chat_id": "42" }),
            json!({ "bot_token": "123:abc", "chat_id": "42", "api_base": base }),
        ),
        (
            "discord",
            json!({ "url": "https://discord.com/api/webhooks/1/x", "username": "Wallos" }),
            json!({ "url": format!("{base}/discord-hook"), "username": "Wallos" }),
        ),
        (
            "gotify",
            json!({ "url": "https://gotify.example.com", "token": "app-token" }),
            json!({ "url": base, "token": "app-token" }),
        ),
        (
            "pushover",
            json!({ "user_key": "uk", "token": "tok" }),
            json!({ "user_key": "uk", "token": "tok", "api_base": base }),
        ),
    ];
    for (kind, config, rerouted) in reroutes {
        let created = body_json(create_channel(&pool, &web, kind, config).await);
        let id = Uuid::parse_str(created.await["id"].as_str().unwrap()).unwrap();
        sqlx::query("update notification_channels set config = $2 where id = $1")
            .bind(id)
            .bind(rerouted)
            .execute(&pool)
            .await
            .unwrap();
    }

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

    let requests = captured.lock().unwrap().clone();
    assert_eq!(
        requests.len(),
        4,
        "une requête par messagerie: {requests:?}"
    );

    // Telegram : POST /bot{token}/sendMessage, JSON {chat_id, text} (oracle legacy).
    let telegram = requests
        .iter()
        .find(|r| r.path == "/bot123:abc/sendMessage")
        .expect("requête Telegram");
    let body: Value = serde_json::from_str(&telegram.body).unwrap();
    assert_eq!(body["chat_id"], "42");
    let text = body["text"].as_str().unwrap();
    assert!(text.contains("Netflix"), "message localisé attendu: {text}");

    // Discord : POST vers le webhook, JSON {content, username} (oracle legacy).
    let discord = requests
        .iter()
        .find(|r| r.path == "/discord-hook")
        .expect("requête Discord");
    let body: Value = serde_json::from_str(&discord.body).unwrap();
    assert!(body["content"].as_str().unwrap().contains("Netflix"));
    assert_eq!(body["username"], "Wallos");

    // Gotify : POST /message, jeton dans l'en-tête X-Gotify-Key (jamais dans l'URL), priorité 5.
    let gotify = requests
        .iter()
        .find(|r| r.path == "/message")
        .expect("requête Gotify");
    assert_eq!(gotify.gotify_key.as_deref(), Some("app-token"));
    let body: Value = serde_json::from_str(&gotify.body).unwrap();
    assert!(body["message"].as_str().unwrap().contains("Netflix"));
    assert_eq!(body["priority"], 5);

    // Pushover : POST /1/messages.json en formulaire URL-encodé token/user/message (oracle legacy).
    let pushover = requests
        .iter()
        .find(|r| r.path == "/1/messages.json")
        .expect("requête Pushover");
    assert!(
        pushover
            .content_type
            .starts_with("application/x-www-form-urlencoded")
    );
    assert!(pushover.body.contains("token=tok"));
    assert!(pushover.body.contains("user=uk"));
    assert!(pushover.body.contains("message="));

    // Le même contenu textuel sur tous les canaux (critère « seul l'adaptateur diffère »).
    let discord_body: Value = serde_json::from_str(&discord.body).unwrap();
    assert_eq!(body["message"], discord_body["content"]);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-004, case = "un canal de messagerie désactivé n'émet aucune requête sortante")]
async fn disabled_messaging_channel_sends_nothing(pool: PgPool) {
    let web = account(&pool, "not004-disabled@example.com").await;
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
    let (base, captured) = spawn_capture_receiver().await;
    // Canal Telegram repointé puis DÉSACTIVÉ : aucune requête ne doit le concerner (critère #2).
    let created = body_json(
        create_channel(
            &pool,
            &web,
            "telegram",
            json!({ "bot_token": "123:abc", "chat_id": "42" }),
        )
        .await,
    )
    .await;
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    sqlx::query("update notification_channels set config = $2, enabled = false where id = $1")
        .bind(id)
        .bind(json!({ "bot_token": "123:abc", "chat_id": "42", "api_base": base }))
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
    // Le rappel est bien émis (journalisé) mais aucun envoi sortant n'a lieu.
    assert_eq!(body_json(resp).await["emitted"], 1);
    assert!(captured.lock().unwrap().is_empty());
}

// --- Envoi de test d'un canal (REQ-NOT-006) ---

/// Récepteur HTTP local qui répond un statut fixe à tout POST (cas d'échec `http-status`).
async fn spawn_failing_receiver(status: StatusCode) -> String {
    let router = Router::new().fallback(move || async move { status });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://{addr}/hook")
}

/// Déclenche l'envoi de test d'un canal.
async fn test_channel(pool: &PgPool, cookie: Option<&str>, id: &str) -> axum::http::Response<Body> {
    send(
        pool,
        "POST",
        &format!("/api/v1/notifications/channels/{id}/test"),
        cookie,
        None,
    )
    .await
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-006, case = "le test d'un canal de messagerie envoie un message factice et rapporte `sent`")]
async fn test_channel_sends_and_reports_sent(pool: PgPool) {
    let web = account(&pool, "not006-sent@example.com").await;
    // Canal Telegram repointé vers un récepteur local : le test passe par le même adaptateur que le cron.
    let (base, captured) = spawn_capture_receiver().await;
    let created = body_json(
        create_channel(
            &pool,
            &web,
            "telegram",
            json!({ "bot_token": "123:abc", "chat_id": "42" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(Uuid::parse_str(&id).unwrap())
        .bind(json!({ "bot_token": "123:abc", "chat_id": "42", "api_base": base }))
        .execute(&pool)
        .await
        .unwrap();

    let resp = test_channel(&pool, Some(&web), &id).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["code"], "sent");

    // Le message factice est bien parti, au format Telegram, avec l'abonnement de test.
    let requests = captured.lock().unwrap().clone();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/bot123:abc/sendMessage");
    let payload: Value = serde_json::from_str(&requests[0].body).unwrap();
    assert!(
        payload["text"]
            .as_str()
            .unwrap()
            .contains("Test subscription")
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-006, case = "échec du test : le diagnostic rapporte le statut HTTP de la cible, jamais l'erreur brute")]
async fn test_channel_reports_http_status(pool: PgPool) {
    let web = account(&pool, "not006-status@example.com").await;
    let failing_url = spawn_failing_receiver(StatusCode::INTERNAL_SERVER_ERROR).await;
    let created = body_json(create_webhook(&pool, &web, "https://hooks.example.com/x").await).await;
    let id = created["id"].as_str().unwrap().to_string();
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(Uuid::parse_str(&id).unwrap())
        .bind(json!({ "url": failing_url }))
        .execute(&pool)
        .await
        .unwrap();

    let resp = test_channel(&pool, Some(&web), &id).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["code"], "http-status");
    assert_eq!(body["http_status"], 500);
    // Aucune fuite : ni URL ni texte d'erreur brut dans la réponse.
    assert!(!body.to_string().contains("127.0.0.1"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-006, case = "cible injoignable : le diagnostic rapporte `connection-failed`")]
async fn test_channel_reports_connection_failure(pool: PgPool) {
    let web = account(&pool, "not006-conn@example.com").await;
    let created = body_json(create_webhook(&pool, &web, "https://hooks.example.com/x").await).await;
    let id = created["id"].as_str().unwrap().to_string();
    // Port 1 : connexion refusée immédiatement (aucune résolution DNS, déterministe).
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(Uuid::parse_str(&id).unwrap())
        .bind(json!({ "url": "http://127.0.0.1:1/hook" }))
        .execute(&pool)
        .await
        .unwrap();

    let resp = test_channel(&pool, Some(&web), &id).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["code"], "connection-failed");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-006, case = "un canal désactivé reste testable (valider la configuration avant activation)")]
async fn test_channel_works_on_disabled_channel(pool: PgPool) {
    let web = account(&pool, "not006-disabled@example.com").await;
    let (base, captured) = spawn_capture_receiver().await;
    let created = body_json(
        create_channel(
            &pool,
            &web,
            "gotify",
            json!({ "url": "https://gotify.example.com", "token": "app-token" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    sqlx::query("update notification_channels set config = $2, enabled = false where id = $1")
        .bind(Uuid::parse_str(&id).unwrap())
        .bind(json!({ "url": base, "token": "app-token" }))
        .execute(&pool)
        .await
        .unwrap();

    let resp = test_channel(&pool, Some(&web), &id).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["ok"], true);
    assert_eq!(captured.lock().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-006)]
async fn authz_owner_test_notification_channel(pool: PgPool) {
    let web = account(&pool, "not006-authz-owner@example.com").await;
    let created = body_json(create_webhook(&pool, &web, "https://hooks.example.com/x").await).await;
    let id = created["id"].as_str().unwrap().to_string();
    // Cible locale fermée : l'envoi échoue proprement, mais l'ACCÈS du propriétaire est 2xx.
    sqlx::query("update notification_channels set config = $2 where id = $1")
        .bind(Uuid::parse_str(&id).unwrap())
        .bind(json!({ "url": "http://127.0.0.1:1/hook" }))
        .execute(&pool)
        .await
        .unwrap();
    let resp = test_channel(&pool, Some(&web), &id).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-006)]
async fn authz_other_test_notification_channel(pool: PgPool) {
    let web = account(&pool, "not006-authz-web@example.com").await;
    let other = account(&pool, "not006-authz-other@example.com").await;
    let created = body_json(create_webhook(&pool, &web, "https://hooks.example.com/x").await).await;
    let id = created["id"].as_str().unwrap().to_string();
    // Un tiers authentifié voit 404 (jamais 403) — et surtout AUCUN envoi n'est déclenché.
    let resp = test_channel(&pool, Some(&other), &id).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-006)]
async fn authz_anon_test_notification_channel(pool: PgPool) {
    let resp = test_channel(&pool, None, &Uuid::new_v4().to_string()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
