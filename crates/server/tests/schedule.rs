//! Tests d'intégration du calcul d'échéance (REQ-SUB-012).
//!
//! `POST /schedule/next-due` : calcul sans état (ancrage+clamp, ADR 0022). Auth requise.

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

async fn post(
    pool: &PgPool,
    uri: &str,
    body: serde_json::Value,
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

async fn next_due(
    pool: &PgPool,
    cookie: Option<&str>,
    anchor: &str,
    interval: u32,
    after: &str,
) -> axum::http::Response<Body> {
    post(
        pool,
        "/api/v1/schedule/next-due",
        json!({
            "first_payment": anchor,
            "cycle": { "unit": "month", "interval": interval },
            "after": after
        }),
        cookie,
    )
    .await
}

async fn next_payment(r: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["next_payment"].as_str().unwrap().to_string()
}

// --- Fonctionnel (ancrage + clamp, ADR 0022) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn end_of_month_clamps_then_returns_to_31(pool: PgPool) {
    let web = account(&pool, "sched@example.com").await;
    // 31 janv -> 28 févr (clamp).
    let r = next_due(&pool, Some(&web), "2025-01-31", 1, "2025-01-31").await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(next_payment(r).await, "2025-02-28");
    // Depuis le 28 févr -> 31 mars (revient au 31, pas ancré au 28).
    assert_eq!(
        next_payment(next_due(&pool, Some(&web), "2025-01-31", 1, "2025-02-28").await).await,
        "2025-03-31"
    );
    // Bissextile : 31 janv -> 29 févr.
    assert_eq!(
        next_payment(next_due(&pool, Some(&web), "2024-01-31", 1, "2024-01-31").await).await,
        "2024-02-29"
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn invalid_input_is_422(pool: PgPool) {
    let web = account(&pool, "sched-bad@example.com").await;
    // Date mal formée.
    assert_eq!(
        post(&pool, "/api/v1/schedule/next-due", json!({ "first_payment": "31/01/2025", "cycle": { "unit": "month", "interval": 1 }, "after": "2025-01-31" }), Some(&web)).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Intervalle nul.
    assert_eq!(
        next_due(&pool, Some(&web), "2025-01-31", 0, "2025-01-31")
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

// --- Cycles jour/semaine/année (REQ-SUB-013) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-013)]
async fn yearly_and_weekly_via_endpoint(pool: PgPool) {
    let web = account(&pool, "sched-dwy@example.com").await;
    // Année depuis le 29 févr bissextile -> 28 févr (clamp ancré, ADR 0022 ; pas le 1er mars de Wallos).
    let r = post(&pool, "/api/v1/schedule/next-due", json!({
        "first_payment": "2024-02-29", "cycle": { "unit": "year", "interval": 1 }, "after": "2024-02-29"
    }), Some(&web)).await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(next_payment(r).await, "2025-02-28");
    // Hebdomadaire : +7 jours, aucune dérive.
    let r = post(&pool, "/api/v1/schedule/next-due", json!({
        "first_payment": "2025-01-01", "cycle": { "unit": "week", "interval": 1 }, "after": "2025-01-01"
    }), Some(&web)).await;
    assert_eq!(next_payment(r).await, "2025-01-08");
}

// --- REQ-STA-005 : échéancier des prochains paiements ---

/// Crée un abonnement mensuel (interval 1) ancré à `first_payment`, éventuellement inactif ou avec une
/// date de fin. Renvoie le statut HTTP.
async fn create_sub(
    pool: &PgPool,
    cookie: &str,
    name: &str,
    first_payment: &str,
    active: bool,
    end_date: Option<&str>,
) -> StatusCode {
    let mut body = json!({
        "name": name,
        "amount": "9.99",
        "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": first_payment,
        "active": active,
    });
    if let Some(end) = end_date {
        body["end_date"] = json!(end);
    }
    post(pool, "/api/v1/subscriptions", body, Some(cookie))
        .await
        .status()
}

/// `GET /schedule/upcoming?days=..&from=..` ; renvoie la liste des occurrences (date + nom).
async fn upcoming(
    pool: &PgPool,
    cookie: Option<&str>,
    days: u32,
    from: &str,
) -> (StatusCode, Vec<(String, String)>) {
    let uri = format!("/api/v1/schedule/upcoming?days={days}&from={from}");
    let mut b = Request::builder().method("GET").uri(&uri);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    let r = app(pool.clone())
        .oneshot(b.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = r.status();
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    if status != StatusCode::OK {
        return (status, Vec::new());
    }
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let payments = v["payments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["date"].as_str().unwrap().to_string(),
                p["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    (status, payments)
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-005, case = "chaque occurrence de la fenêtre listée, y compris plusieurs d'un même abonnement")]
async fn lists_every_occurrence_including_repeats(pool: PgPool) {
    let web = account(&pool, "sta005-multi@example.com").await;
    // Abonnement mensuel ancré au 15 janv : sur une fenêtre de 90 jours depuis le 1er janv, trois
    // occurrences (15 janv/févr/mars).
    assert_eq!(
        create_sub(&pool, &web, "Netflix", "2025-01-15", true, None).await,
        StatusCode::CREATED
    );
    let (status, payments) = upcoming(&pool, Some(&web), 90, "2025-01-01").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        payments,
        vec![
            ("2025-01-15".to_string(), "Netflix".to_string()),
            ("2025-02-15".to_string(), "Netflix".to_string()),
            ("2025-03-15".to_string(), "Netflix".to_string()),
        ]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-005, case = "abonnement terminé dans la fenêtre : aucune occurrence après la date de fin")]
async fn no_occurrence_after_end_date(pool: PgPool) {
    let web = account(&pool, "sta005-end@example.com").await;
    // Mensuel ancré au 15 janv, fin au 20 févr : seules 15 janv + 15 févr, jamais 15 mars (REQ-SUB-009).
    assert_eq!(
        create_sub(
            &pool,
            &web,
            "Spotify",
            "2025-01-15",
            true,
            Some("2025-02-20")
        )
        .await,
        StatusCode::CREATED
    );
    let (_, payments) = upcoming(&pool, Some(&web), 120, "2025-01-01").await;
    assert_eq!(
        payments,
        vec![
            ("2025-01-15".to_string(), "Spotify".to_string()),
            ("2025-02-15".to_string(), "Spotify".to_string()),
        ]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-005, case = "abonnement inactif exclu de l'échéancier (REQ-SUB-008)")]
async fn inactive_subscription_is_excluded(pool: PgPool) {
    let web = account(&pool, "sta005-inactive@example.com").await;
    assert_eq!(
        create_sub(&pool, &web, "Actif", "2025-01-10", true, None).await,
        StatusCode::CREATED
    );
    assert_eq!(
        create_sub(&pool, &web, "Désactivé", "2025-01-12", false, None).await,
        StatusCode::CREATED
    );
    let (_, payments) = upcoming(&pool, Some(&web), 20, "2025-01-01").await;
    // Seul l'abonnement actif apparaît.
    assert_eq!(
        payments,
        vec![("2025-01-10".to_string(), "Actif".to_string())]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-005, case = "fenêtre invalide (jours nuls / hors bornes) -> 422")]
async fn invalid_window_is_422(pool: PgPool) {
    let web = account(&pool, "sta005-bad@example.com").await;
    assert_eq!(
        upcoming(&pool, Some(&web), 0, "2025-01-01").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        upcoming(&pool, Some(&web), 100_000, "2025-01-01").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Date `from` mal formée.
    let (status, _) = upcoming(&pool, Some(&web), 30, "01-2025-01").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// --- Autorisation §9 : getUpcomingPayments (portée foyer) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-005)]
async fn authz_owner_get_upcoming_payments(pool: PgPool) {
    let web = account(&pool, "own-up@example.com").await;
    assert_eq!(
        create_sub(&pool, &web, "Mine", "2025-01-10", true, None).await,
        StatusCode::CREATED
    );
    let (status, payments) = upcoming(&pool, Some(&web), 30, "2025-01-01").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        payments,
        vec![("2025-01-10".to_string(), "Mine".to_string())]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-005)]
async fn authz_other_get_upcoming_payments(pool: PgPool) {
    // Isolation §9 : un autre foyer ne voit jamais les échéances d'autrui — seulement les siennes.
    let owner = account(&pool, "owner-up@example.com").await;
    assert_eq!(
        create_sub(&pool, &owner, "Owner Sub", "2025-01-10", true, None).await,
        StatusCode::CREATED
    );
    let other = account(&pool, "other-up@example.com").await;
    let (status, payments) = upcoming(&pool, Some(&other), 30, "2025-01-01").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        payments.is_empty(),
        "l'autre foyer n'a aucun abonnement, échéancier vide, jamais celui d'owner"
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-005)]
async fn authz_anon_get_upcoming_payments(pool: PgPool) {
    assert_eq!(
        upcoming(&pool, None, 30, "2025-01-01").await.0,
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : computeNextDue (calcul sans état, pas de portée foyer) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn authz_owner_compute_next_due(pool: PgPool) {
    let web = account(&pool, "own-nd@example.com").await;
    assert_eq!(
        next_due(&pool, Some(&web), "2025-01-31", 1, "2025-01-31")
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn authz_other_compute_next_due(pool: PgPool) {
    // Calcul sans état : accessible à tout compte authentifié.
    let web = account(&pool, "other-nd@example.com").await;
    assert_eq!(
        next_due(&pool, Some(&web), "2025-01-31", 1, "2025-01-31")
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-012)]
async fn authz_anon_compute_next_due(pool: PgPool) {
    assert_eq!(
        next_due(&pool, None, "2025-01-31", 1, "2025-01-31")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-014, case = "le calcul d'échéance rattrape : première occurrence strictement future")]
async fn compute_next_due_catches_up_past_occurrences(pool: PgPool) {
    let cookie = account(&pool, "sub014-nextdue@example.com").await;
    // Ancrage 18 mois avant `after` : 18 occurrences dépassées, la réponse est la première future.
    let r = post(
        &pool,
        "/api/v1/schedule/next-due",
        json!({
            "first_payment": "2025-01-15",
            "cycle": { "unit": "month", "interval": 1 },
            "after": "2026-08-06"
        }),
        Some(&cookie),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(next_payment(r).await, "2026-08-15");
}
