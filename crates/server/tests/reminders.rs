//! Tests d'intégration des rappels avant échéance (REQ-NOT-001).
//!
//! Réglage du délai, vue des rappels du jour, et cron `POST /internal/run-reminders` (déclenchement
//! exact, regroupement par compte, idempotence le même jour, oracle Wallos gelé). Isolation §9 ;
//! le cron est authentifié par un secret d'opérateur.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use wallos_req_macros::verifies;
use wallos_server::{CronToken, app_with_db, app_with_db_and_cron};
use wallos_storage::Db;

const PASSWORD: &str = "correct horse battery staple";
const CRON_SECRET: &str = "test-cron-secret";

fn app(pool: PgPool) -> Router {
    app_with_db(Db::from_pool(pool))
}

/// Application avec le secret de cron injecté (tests du cron, sans variable d'environnement).
fn app_cron(pool: PgPool) -> Router {
    app_with_db_and_cron(
        Db::from_pool(pool),
        CronToken(Some(CRON_SECRET.to_string())),
    )
}

async fn oneshot(router: Router, req: Request<Body>) -> axum::http::Response<Body> {
    router.oneshot(req).await.unwrap()
}

fn builder(method: &str, uri: &str) -> axum::http::request::Builder {
    Request::builder().method(method).uri(uri)
}

async fn body_json(resp: axum::http::Response<Body>) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn account(pool: &PgPool, email: &str) -> String {
    let create = builder("POST", "/api/v1/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "email": email, "password": PASSWORD }).to_string(),
        ))
        .unwrap();
    assert_eq!(
        oneshot(app(pool.clone()), create).await.status(),
        StatusCode::CREATED
    );
    let login = builder("POST", "/api/v1/sessions")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "email": email, "password": PASSWORD }).to_string(),
        ))
        .unwrap();
    let r = oneshot(app(pool.clone()), login).await;
    r.headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// GET/PUT/POST avec cookie sur l'app par défaut.
async fn with_cookie(
    pool: &PgPool,
    method: &str,
    uri: &str,
    cookie: &str,
    body: Option<Value>,
) -> axum::http::Response<Body> {
    let mut b = builder(method, uri).header(header::COOKIE, cookie);
    let body = match body {
        Some(v) => {
            b = b.header(header::CONTENT_TYPE, "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    oneshot(app(pool.clone()), b.body(body).unwrap()).await
}

/// Crée un abonnement mensuel actif dont le premier paiement est `first_payment` (YYYY-MM-DD).
async fn create_sub(pool: &PgPool, cookie: &str, name: &str, first_payment: &str) -> String {
    let body = json!({
        "name": name, "amount": "9.99", "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 }, "first_payment": first_payment, "active": true
    });
    let r = with_cookie(pool, "POST", "/api/v1/subscriptions", cookie, Some(body)).await;
    assert_eq!(r.status(), StatusCode::CREATED);
    body_json(r).await["id"].as_str().unwrap().to_string()
}

/// Lance le cron avec le secret et une date de référence donnée.
async fn run_cron(pool: &PgPool, token: Option<&str>, as_of: &str) -> axum::http::Response<Body> {
    let mut b = builder(
        "POST",
        &format!("/api/v1/internal/run-reminders?as_of={as_of}"),
    );
    if let Some(t) = token {
        b = b.header("x-cron-token", t);
    }
    oneshot(app_cron(pool.clone()), b.body(Body::empty()).unwrap()).await
}

// --- Réglage du délai ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001, case = "délai par défaut = 1 jour, modifiable")]
async fn lead_time_defaults_to_one_and_is_configurable(pool: PgPool) {
    let web = account(&pool, "not001-setting@example.com").await;
    let got =
        body_json(with_cookie(&pool, "GET", "/api/v1/settings/reminder", &web, None).await).await;
    assert_eq!(got["lead_days"], 1);

    let updated = with_cookie(
        &pool,
        "PUT",
        "/api/v1/settings/reminder",
        &web,
        Some(json!({ "lead_days": 3 })),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let got =
        body_json(with_cookie(&pool, "GET", "/api/v1/settings/reminder", &web, None).await).await;
    assert_eq!(got["lead_days"], 3);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001, case = "délai hors bornes -> 422")]
async fn out_of_range_lead_is_rejected(pool: PgPool) {
    let web = account(&pool, "not001-bad@example.com").await;
    let r = with_cookie(
        &pool,
        "PUT",
        "/api/v1/settings/reminder",
        &web,
        Some(json!({ "lead_days": 999 })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// --- Cron : déclenchement, regroupement, idempotence (oracle) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001, case = "déclenchement exact + regroupement par compte (exemple oracle gelé)")]
async fn cron_emits_grouped_reminders_on_exact_lead_day(pool: PgPool) {
    // Aujourd'hui de référence 2026-08-06, délai 1 : Netflix et Spotify échéant le 2026-08-07 (à 1 jour)
    // déclenchent ; Presse échéant le 2026-08-10 (à 4 jours) ne déclenche pas.
    let web = account(&pool, "not001-cron@example.com").await;
    create_sub(&pool, &web, "Netflix", "2026-08-07").await;
    create_sub(&pool, &web, "Spotify", "2026-08-07").await;
    create_sub(&pool, &web, "Presse", "2026-08-10").await;

    let body = body_json(run_cron(&pool, Some(CRON_SECRET), "2026-08-06").await).await;
    // Deux rappels émis, pour UN seul compte (regroupés).
    assert_eq!(body["emitted"], 2);
    assert_eq!(body["accounts_notified"], 1);
    assert_eq!(body["as_of"], "2026-08-06");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001, case = "ré-exécution le même jour : aucun rappel ré-émis (idempotence journal)")]
async fn cron_is_idempotent_same_day(pool: PgPool) {
    let web = account(&pool, "not001-idem@example.com").await;
    create_sub(&pool, &web, "Netflix", "2026-08-07").await;

    let first = body_json(run_cron(&pool, Some(CRON_SECRET), "2026-08-06").await).await;
    assert_eq!(first["emitted"], 1);
    // Deuxième passage le même jour : déjà journalisé, rien de neuf.
    let second = body_json(run_cron(&pool, Some(CRON_SECRET), "2026-08-06").await).await;
    assert_eq!(second["emitted"], 0);
    assert_eq!(second["accounts_notified"], 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001, case = "cron désactivé si aucun secret configuré -> 404")]
async fn cron_disabled_without_secret(pool: PgPool) {
    // App par défaut : aucun CRON_TOKEN injecté -> endpoint désactivé.
    let r = with_cookie(&pool, "POST", "/api/v1/internal/run-reminders", "x=y", None).await;
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

// --- Vue des rappels du jour ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001, case = "la vue liste les rappels dus aujourd'hui (horloge serveur)")]
async fn reminders_view_lists_todays_due(pool: PgPool) {
    let web = account(&pool, "not001-view@example.com").await;
    // Échéance demain (délai par défaut 1) -> dû aujourd'hui.
    let tomorrow = (Utc::now().date_naive() + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    create_sub(&pool, &web, "Netflix", &tomorrow).await;
    // Échéance dans 10 jours -> pas dû aujourd'hui.
    let later = (Utc::now().date_naive() + Duration::days(10))
        .format("%Y-%m-%d")
        .to_string();
    create_sub(&pool, &web, "Presse", &later).await;

    let body = body_json(with_cookie(&pool, "GET", "/api/v1/reminders", &web, None).await).await;
    let names: Vec<&str> = body["reminders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Netflix"]);
    assert_eq!(body["reminders"][0]["days_until"], 1);
}

// --- Autorisation (§9) : réglages + vue (AuthActor) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_owner_get_reminder_setting(pool: PgPool) {
    let web = account(&pool, "own-getset@example.com").await;
    assert_eq!(
        with_cookie(&pool, "GET", "/api/v1/settings/reminder", &web, None)
            .await
            .status(),
        StatusCode::OK
    );
}
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_other_get_reminder_setting(pool: PgPool) {
    // Chaque foyer a son propre réglage ; un autre foyer lit le sien (jamais celui d'autrui).
    let _a = account(&pool, "a-getset@example.com").await;
    let b = account(&pool, "b-getset@example.com").await;
    assert_eq!(
        with_cookie(&pool, "GET", "/api/v1/settings/reminder", &b, None)
            .await
            .status(),
        StatusCode::OK
    );
}
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_anon_get_reminder_setting(pool: PgPool) {
    let r = with_cookie(&pool, "GET", "/api/v1/settings/reminder", "x=y", None).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_owner_set_reminder_setting(pool: PgPool) {
    let web = account(&pool, "own-setset@example.com").await;
    assert_eq!(
        with_cookie(
            &pool,
            "PUT",
            "/api/v1/settings/reminder",
            &web,
            Some(json!({ "lead_days": 2 }))
        )
        .await
        .status(),
        StatusCode::OK
    );
}
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_other_set_reminder_setting(pool: PgPool) {
    // Modifier son propre réglage n'affecte jamais un autre foyer.
    let a = account(&pool, "a-setset@example.com").await;
    let b = account(&pool, "b-setset@example.com").await;
    with_cookie(
        &pool,
        "PUT",
        "/api/v1/settings/reminder",
        &b,
        Some(json!({ "lead_days": 5 })),
    )
    .await;
    let got_a =
        body_json(with_cookie(&pool, "GET", "/api/v1/settings/reminder", &a, None).await).await;
    assert_eq!(got_a["lead_days"], 1);
}
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_anon_set_reminder_setting(pool: PgPool) {
    let r = with_cookie(
        &pool,
        "PUT",
        "/api/v1/settings/reminder",
        "x=y",
        Some(json!({ "lead_days": 2 })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_owner_get_reminders(pool: PgPool) {
    let web = account(&pool, "own-getrem@example.com").await;
    assert_eq!(
        with_cookie(&pool, "GET", "/api/v1/reminders", &web, None)
            .await
            .status(),
        StatusCode::OK
    );
}
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_other_get_reminders(pool: PgPool) {
    // Un foyer sans abonnement dû ne voit rien de celui d'autrui.
    let a = account(&pool, "a-getrem@example.com").await;
    let tomorrow = (Utc::now().date_naive() + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    create_sub(&pool, &a, "A-Netflix", &tomorrow).await;
    let b = account(&pool, "b-getrem@example.com").await;
    let body = body_json(with_cookie(&pool, "GET", "/api/v1/reminders", &b, None).await).await;
    assert!(body["reminders"].as_array().unwrap().is_empty());
}
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_anon_get_reminders(pool: PgPool) {
    let r = with_cookie(&pool, "GET", "/api/v1/reminders", "x=y", None).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

// --- Autorisation : cron (secret d'opérateur) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_owner_run_reminders(pool: PgPool) {
    // Secret valide -> autorisé (200).
    let r = run_cron(&pool, Some(CRON_SECRET), "2026-08-06").await;
    assert_eq!(r.status(), StatusCode::OK);
}
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_other_run_reminders(pool: PgPool) {
    // Secret invalide -> refusé (401).
    let r = run_cron(&pool, Some("mauvais-secret"), "2026-08-06").await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-NOT-001)]
async fn authz_anon_run_reminders(pool: PgPool) {
    // Aucun secret présenté (cron pourtant configuré) -> refusé (401).
    let r = run_cron(&pool, None, "2026-08-06").await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
