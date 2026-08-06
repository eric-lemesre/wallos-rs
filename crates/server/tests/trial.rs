//! Tests d'intégration de la période d'essai gratuit (REQ-SUB-010).
//!
//! Un abonnement en essai n'est pas compté dans les statistiques tant que l'essai n'est pas terminé
//! (critère #1) ; le cron émet un rappel de fin d'essai **distinct** du rappel de paiement (critère #2).
//! Concept absent de Wallos -> design (ADR 0041). Isolation §9.

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

async fn send(
    pool: &PgPool,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> axum::http::Response<Body> {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    let body = match body {
        Some(v) => {
            b = b.header(header::CONTENT_TYPE, "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    app(pool.clone())
        .oneshot(b.body(body).unwrap())
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
            Some(json!({ "email": email, "password": PASSWORD }))
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

/// Crée un abonnement mensuel EUR actif, avec une fin d'essai optionnelle.
async fn create_sub(
    pool: &PgPool,
    cookie: &str,
    name: &str,
    first_payment: &str,
    trial_end: Option<&str>,
) -> Value {
    let mut body = json!({
        "name": name, "amount": "10.00", "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 }, "first_payment": first_payment, "active": true
    });
    if let Some(t) = trial_end {
        body["trial_end"] = json!(t);
    }
    let r = send(
        pool,
        "POST",
        "/api/v1/subscriptions",
        Some(cookie),
        Some(body),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED, "création {name}");
    body_json(r).await
}

/// Total (chaîne) de la liste des abonnements.
async fn list_total(pool: &PgPool, cookie: &str) -> String {
    let body =
        body_json(send(pool, "GET", "/api/v1/subscriptions", Some(cookie), None).await).await;
    body["total"]["total"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-010, case = "un abonnement en essai n'est pas compté dans le total tant que l'essai dure")]
async fn in_trial_subscription_is_excluded_from_total(pool: PgPool) {
    let web = account(&pool, "sub010-total@example.com").await;
    // Essai jusqu'à dans 30 jours (en cours) : exclu. Un abonnement normal (10) est compté.
    let far_trial = (Utc::now().date_naive() + Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();
    create_sub(&pool, &web, "EnEssai", "2020-01-01", Some(&far_trial)).await;
    create_sub(&pool, &web, "Normal", "2020-01-01", None).await;

    // Le total ne reflète que l'abonnement normal (10.00), pas l'essai.
    assert_eq!(list_total(&pool, &web).await, "10.00");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-010, case = "une fois l'essai terminé, l'abonnement compte de nouveau")]
async fn subscription_counts_after_trial_ends(pool: PgPool) {
    let web = account(&pool, "sub010-ended@example.com").await;
    // Essai déjà terminé (hier) : l'abonnement compte.
    let past_trial = (Utc::now().date_naive() - Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    create_sub(&pool, &web, "EssaiFini", "2020-01-01", Some(&past_trial)).await;
    assert_eq!(list_total(&pool, &web).await, "10.00");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-010, case = "le DTO expose l'essai en cours (in_trial)")]
async fn dto_exposes_in_trial_flag(pool: PgPool) {
    let web = account(&pool, "sub010-flag@example.com").await;
    let far_trial = (Utc::now().date_naive() + Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();
    let created = create_sub(&pool, &web, "EnEssai", "2020-01-01", Some(&far_trial)).await;
    assert_eq!(created["in_trial"], true);
    assert_eq!(created["trial_end"], far_trial);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-010, case = "un essai précédant le premier paiement est accepté (cas normal)")]
async fn trial_before_first_payment_is_accepted(pool: PgPool) {
    let web = account(&pool, "sub010-normal@example.com").await;
    // Essai gratuit qui se termine AVANT la première facturation : cas d'usage standard.
    let created = create_sub(&pool, &web, "Essai", "2030-06-01", Some("2030-05-01")).await;
    assert_eq!(created["trial_end"], "2030-05-01");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-010, case = "date de fin d'essai illisible -> 422")]
async fn invalid_trial_date_is_rejected(pool: PgPool) {
    let web = account(&pool, "sub010-baddate@example.com").await;
    let body = json!({
        "name": "X", "amount": "10.00", "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2026-06-01",
        "trial_end": "pas-une-date"
    });
    let r = send(
        &pool,
        "POST",
        "/api/v1/subscriptions",
        Some(&web),
        Some(body),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-010, case = "le cron émet un rappel de fin d'essai DISTINCT du rappel de paiement")]
async fn cron_emits_distinct_trial_reminder(pool: PgPool) {
    let web = account(&pool, "sub010-cron@example.com").await;
    // Aujourd'hui de référence 2026-08-06, délai 1 : fin d'essai le 2026-08-07 (à 1 jour) -> rappel trial.
    // first_payment lointain pour que l'échéance de paiement ne coïncide pas.
    create_sub(&pool, &web, "Essai", "2026-08-20", Some("2026-08-07")).await;

    // Vue du jour : un rappel de type trial_ending.
    // (get /reminders utilise l'horloge serveur ; on vérifie plutôt via le cron déterministe as_of.)
    let cron = app_with_db_and_cron(
        Db::from_pool(pool.clone()),
        CronToken(Some(CRON_SECRET.to_string())),
    );
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/internal/run-reminders?as_of=2026-08-06")
        .header("x-cron-token", CRON_SECRET)
        .body(Body::empty())
        .unwrap();
    let resp = cron.oneshot(req).await.unwrap();
    let body = body_json(resp).await;
    // Un rappel émis (la fin d'essai), pour un compte.
    assert_eq!(body["emitted"], 1);
    assert_eq!(body["accounts_notified"], 1);
}
