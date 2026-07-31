//! Tests d'intégration de la série d'évolution du coût mensuel (REQ-STA-006).
//!
//! `GET /statistics/cost-evolution` : série des N derniers mois, chaque point reflétant les
//! abonnements **actifs à ce mois-là** (fenêtre `first_payment`/`end_date`), convertis dans la devise
//! de référence. Auth requise ; isolation §9.

use axum::Router;
use axum::body::Body;
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

async fn post(
    pool: &PgPool,
    uri: &str,
    body: Value,
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

/// Crée un abonnement mensuel (interval 1) dans une devise donnée, éventuellement inactif.
async fn create_sub(
    pool: &PgPool,
    cookie: &str,
    name: &str,
    amount: &str,
    currency: &str,
    first_payment: &str,
    active: bool,
) -> StatusCode {
    let body = json!({
        "name": name,
        "amount": amount,
        "currency": currency,
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": first_payment,
        "active": active
    });
    post(pool, "/api/v1/subscriptions", body, Some(cookie))
        .await
        .status()
}

/// Amorce un taux de change (donnée de référence globale) directement en base.
async fn seed_rate(pool: &PgPool, base: &str, quote: &str, rate: &str) {
    sqlx::query(
        "insert into exchange_rates (base_currency, quote_currency, rate, as_of, source, fetched_at) \
         values ($1, $2, $3::numeric, '2026-01-01'::date, 'test', now())",
    )
    .bind(base)
    .bind(quote)
    .bind(rate)
    .execute(pool)
    .await
    .unwrap();
}

async fn evolution(pool: &PgPool, cookie: Option<&str>, query: &str) -> axum::http::Response<Body> {
    let mut b = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/statistics/cost-evolution{query}"));
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    app(pool.clone())
        .oneshot(b.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body_json(r: axum::http::Response<Body>) -> Value {
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn totals(body: &Value) -> Vec<String> {
    body["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["total"].as_str().unwrap().to_string())
        .collect()
}

// --- Fonctionnel ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn series_has_twelve_points_by_default(pool: PgPool) {
    let web = account(&pool, "sta006-default@example.com").await;
    assert_eq!(
        create_sub(&pool, &web, "Old", "10.00", "EUR", "2020-01-01", true).await,
        StatusCode::CREATED
    );
    let body = body_json(evolution(&pool, Some(&web), "").await).await;
    assert_eq!(body["currency"], "EUR");
    let points = body["points"].as_array().unwrap();
    assert_eq!(points.len(), 12);
    // Ordonnés du plus ancien au plus récent (mois croissants YYYY-MM).
    let months: Vec<&str> = points
        .iter()
        .map(|p| p["month"].as_str().unwrap())
        .collect();
    let mut sorted = months.clone();
    sorted.sort_unstable();
    assert_eq!(months, sorted);
    assert_eq!(body["from"], months[0]);
    assert_eq!(body["to"], months[11]);
    // Abonnement présent depuis 2020 : coût constant sur toute la fenêtre.
    assert!(totals(&body).iter().all(|t| t == "10.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn point_reflects_active_at_that_month_not_current_state(pool: PgPool) {
    let web = account(&pool, "sta006-temporal@example.com").await;
    // Ancien (toujours actif) : compte partout. Futur : ne compte nulle part dans la fenêtre passée.
    assert_eq!(
        create_sub(&pool, &web, "Old", "10.00", "EUR", "2020-01-01", true).await,
        StatusCode::CREATED
    );
    assert_eq!(
        create_sub(&pool, &web, "Future", "99.00", "EUR", "2099-01-01", true).await,
        StatusCode::CREATED
    );
    let body = body_json(evolution(&pool, Some(&web), "?months=6").await).await;
    assert_eq!(body["points"].as_array().unwrap().len(), 6);
    // Seul l'ancien pèse : « Future » n'est jamais projeté rétroactivement.
    assert!(totals(&body).iter().all(|t| t == "10.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn inactive_subscription_is_excluded_from_the_series(pool: PgPool) {
    let web = account(&pool, "sta006-inactive@example.com").await;
    assert_eq!(
        create_sub(&pool, &web, "Disabled", "10.00", "EUR", "2020-01-01", false).await,
        StatusCode::CREATED
    );
    let body = body_json(evolution(&pool, Some(&web), "?months=3").await).await;
    // Aucun abonnement actif : série de zéros.
    assert!(totals(&body).iter().all(|t| t == "0.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn amounts_are_converted_to_the_reference_currency(pool: PgPool) {
    // Devise de référence EUR ; taux USD->EUR = 0,50. Un abonnement de 100 USD/mois pèse 50 EUR.
    seed_rate(&pool, "USD", "EUR", "0.50").await;
    let web = account(&pool, "sta006-convert@example.com").await;
    assert_eq!(
        create_sub(&pool, &web, "Dollar", "100.00", "USD", "2020-01-01", true).await,
        StatusCode::CREATED
    );
    let body = body_json(evolution(&pool, Some(&web), "?months=2").await).await;
    assert_eq!(body["currency"], "EUR");
    assert!(totals(&body).iter().all(|t| t == "50.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn out_of_range_months_is_422(pool: PgPool) {
    let web = account(&pool, "sta006-bad@example.com").await;
    assert_eq!(
        evolution(&pool, Some(&web), "?months=0").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        evolution(&pool, Some(&web), "?months=1000").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

// --- Autorisation (§9) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn authz_owner_get_cost_evolution(pool: PgPool) {
    let web = account(&pool, "own-sta006@example.com").await;
    assert_eq!(
        create_sub(&pool, &web, "Mine", "10.00", "EUR", "2020-01-01", true).await,
        StatusCode::CREATED
    );
    let r = evolution(&pool, Some(&web), "?months=3").await;
    assert_eq!(r.status(), StatusCode::OK);
    assert!(totals(&body_json(r).await).iter().all(|t| t == "10.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn authz_other_get_cost_evolution(pool: PgPool) {
    // Un autre foyer ne voit jamais les abonnements d'autrui : sa série reste nulle (§9).
    let a = account(&pool, "a-sta006@example.com").await;
    assert_eq!(
        create_sub(&pool, &a, "A-only", "10.00", "EUR", "2020-01-01", true).await,
        StatusCode::CREATED
    );
    let b = account(&pool, "b-sta006@example.com").await;
    let r = evolution(&pool, Some(&b), "?months=3").await;
    assert_eq!(r.status(), StatusCode::OK);
    assert!(totals(&body_json(r).await).iter().all(|t| t == "0.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn authz_anon_get_cost_evolution(pool: PgPool) {
    assert_eq!(
        evolution(&pool, None, "?months=3").await.status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Compléments de revue kimi STA-006 (F3/F4) ---

/// Poste un abonnement au corps JSON libre (cycle/end_date personnalisés) ; renvoie le statut.
async fn create_raw(pool: &PgPool, cookie: &str, body: Value) -> StatusCode {
    post(pool, "/api/v1/subscriptions", body, Some(cookie))
        .await
        .status()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn subscription_ended_before_the_window_contributes_nothing(pool: PgPool) {
    let web = account(&pool, "sta006-ended@example.com").await;
    // Actif mais terminé en 2020 : sa fenêtre ne recoupe aucun des 12 derniers mois → série nulle,
    // mais la série reste COMPLÈTE (l'abonnement est convertible, juste hors fenêtre temporelle).
    assert_eq!(
        create_raw(
            &pool,
            &web,
            json!({
                "name": "Old-ended", "amount": "10.00", "currency": "EUR",
                "cycle": { "unit": "month", "interval": 1 },
                "first_payment": "2020-01-01", "end_date": "2020-06-30", "active": true
            })
        )
        .await,
        StatusCode::CREATED
    );
    let body = body_json(evolution(&pool, Some(&web), "?months=12").await).await;
    assert_eq!(body["complete"], true);
    assert!(totals(&body).iter().all(|t| t == "0.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn missing_rate_excludes_and_marks_incomplete(pool: PgPool) {
    // Devise de référence EUR, aucun taux JPY→EUR : l'abonnement est exclu et la série signalée partielle.
    let web = account(&pool, "sta006-partial@example.com").await;
    assert_eq!(
        create_sub(&pool, &web, "Yen", "4200", "JPY", "2020-01-01", true).await,
        StatusCode::CREATED
    );
    let body = body_json(evolution(&pool, Some(&web), "?months=3").await).await;
    assert_eq!(body["complete"], false);
    assert!(totals(&body).iter().all(|t| t == "0.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn annual_cycle_is_normalized_to_monthly(pool: PgPool) {
    let web = account(&pool, "sta006-annual@example.com").await;
    // 120 EUR/an = 10 EUR/mois (normalisation STA-001).
    assert_eq!(
        create_raw(
            &pool,
            &web,
            json!({
                "name": "Annual", "amount": "120.00", "currency": "EUR",
                "cycle": { "unit": "year", "interval": 1 },
                "first_payment": "2020-01-01", "active": true
            })
        )
        .await,
        StatusCode::CREATED
    );
    let body = body_json(evolution(&pool, Some(&web), "?months=2").await).await;
    assert_eq!(body["complete"], true);
    assert!(totals(&body).iter().all(|t| t == "10.00"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-006)]
async fn jpy_reference_formats_with_zero_decimals(pool: PgPool) {
    // Override devise cible = JPY (0 décimale) : identité JPY→JPY, formatage sans décimale.
    let web = account(&pool, "sta006-jpy@example.com").await;
    assert_eq!(
        create_sub(&pool, &web, "Yen", "4200", "JPY", "2020-01-01", true).await,
        StatusCode::CREATED
    );
    let body = body_json(evolution(&pool, Some(&web), "?months=2&currency=JPY").await).await;
    assert_eq!(body["currency"], "JPY");
    assert_eq!(body["complete"], true);
    assert!(totals(&body).iter().all(|t| t == "4200"));
}
