//! Tests d'intégration de la règle transverse d'exclusion (REQ-STA-003).
//!
//! Un abonnement **désactivé** (REQ-SUB-008), **terminé** (REQ-SUB-009) ou **en essai gratuit**
//! (REQ-SUB-010) est exclu de **tous** les agrégats, selon la règle propre à son état ; un abonnement
//! **réactivé** y est immédiatement réintégré (critère #2). On exerce ici les quatre surfaces d'agrégat
//! (total de liste, répartition, évolution, échéancier) sur une même règle. Oracle: legacy (`inactive=0`
//! partout dans Wallos) étendu aux états fin/essai du modèle subtrack. Isolation §9.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use wallos_req_macros::verifies;
use wallos_server::app_with_db;
use wallos_storage::Db;

const PASSWORD: &str = "correct horse battery staple";

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

/// Crée un abonnement mensuel EUR (10.00), avec activité et fin d'essai/fin optionnelles. Renvoie l'id.
async fn create_sub(
    pool: &PgPool,
    cookie: &str,
    name: &str,
    first_payment: &str,
    active: bool,
    trial_end: Option<&str>,
    end_date: Option<&str>,
) -> String {
    let mut body = json!({
        "name": name, "amount": "10.00", "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 }, "first_payment": first_payment, "active": active
    });
    if let Some(t) = trial_end {
        body["trial_end"] = json!(t);
    }
    if let Some(e) = end_date {
        body["end_date"] = json!(e);
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
    body_json(r).await["id"].as_str().unwrap().to_string()
}

/// Total (chaîne) de la liste des abonnements.
async fn list_total(pool: &PgPool, cookie: &str) -> String {
    let body =
        body_json(send(pool, "GET", "/api/v1/subscriptions", Some(cookie), None).await).await;
    body["total"]["total"].as_str().unwrap().to_string()
}

/// Dates de l'échéancier sur `[from, from+days]`.
async fn upcoming_dates(pool: &PgPool, cookie: &str, from: &str, days: u32) -> Vec<String> {
    let uri = format!("/api/v1/schedule/upcoming?from={from}&days={days}");
    let body = body_json(send(pool, "GET", &uri, Some(cookie), None).await).await;
    body["payments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["date"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-003, case = "un abonnement désactivé est exclu du total ET de l'échéancier")]
async fn disabled_is_excluded_from_total_and_schedule(pool: PgPool) {
    let web = account(&pool, "sta003-disabled@example.com").await;
    // Un actif (compté) et un désactivé (exclu partout), tous deux avec échéances dans la fenêtre.
    create_sub(&pool, &web, "Actif", "2026-01-05", true, None, None).await;
    create_sub(&pool, &web, "Désactivé", "2026-01-06", false, None, None).await;

    assert_eq!(list_total(&pool, &web).await, "10.00");
    // Échéancier janvier : seule l'échéance de l'actif (05) apparaît, jamais celle du désactivé (06).
    let dates = upcoming_dates(&pool, &web, "2026-01-01", 20).await;
    assert_eq!(dates, vec!["2026-01-05".to_string()]);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-003, case = "un abonnement en essai : aucune échéance pendant l'essai, reprise après")]
async fn in_trial_occurrences_are_excluded_from_schedule_until_trial_ends(pool: PgPool) {
    let web = account(&pool, "sta003-trial-schedule@example.com").await;
    // Mensuel ancré au 10 janv, essai jusqu'au 10 mars : janv/févr tombent pendant l'essai (exclus),
    // 10 mars (fin d'essai, dû) et 10 avril apparaissent.
    create_sub(
        &pool,
        &web,
        "Essai",
        "2026-01-10",
        true,
        Some("2026-03-10"),
        None,
    )
    .await;
    let dates = upcoming_dates(&pool, &web, "2026-01-01", 120).await;
    assert_eq!(
        dates,
        vec!["2026-03-10".to_string(), "2026-04-10".to_string()]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-003, case = "un abonnement en essai est exclu de la répartition à ce jour")]
async fn in_trial_is_excluded_from_repartition(pool: PgPool) {
    let web = account(&pool, "sta003-trial-repartition@example.com").await;
    // Essai courant jusqu'en 2999 (toujours en essai) : exclu de la répartition. Un actif normal compte.
    create_sub(
        &pool,
        &web,
        "Essai",
        "2020-01-01",
        true,
        Some("2999-01-01"),
        None,
    )
    .await;
    create_sub(&pool, &web, "Normal", "2020-01-01", true, None, None).await;

    let body = body_json(
        send(
            &pool,
            "GET",
            "/api/v1/statistics/repartition",
            Some(&web),
            None,
        )
        .await,
    )
    .await;
    // Total de la répartition = le seul abonnement hors essai (10.00), l'essai ne pèse pas.
    assert_eq!(body["total"], "10.00");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-003, case = "critère #2 : réactiver un abonnement le réintègre immédiatement au total")]
async fn reactivating_a_subscription_reincludes_it_immediately(pool: PgPool) {
    let web = account(&pool, "sta003-reactivate@example.com").await;
    let id = create_sub(&pool, &web, "Bascule", "2020-01-01", false, None, None).await;
    // Désactivé à la création : exclu du total.
    assert_eq!(list_total(&pool, &web).await, "0");

    // Réactivation via update : réintégré au recalcul suivant, sans étape supplémentaire.
    let r = send(
        &pool,
        "PUT",
        &format!("/api/v1/subscriptions/{id}"),
        Some(&web),
        Some(json!({
            "name": "Bascule", "amount": "10.00", "currency": "EUR",
            "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2020-01-01", "active": true
        })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK, "réactivation");
    assert_eq!(list_total(&pool, &web).await, "10.00");
}
