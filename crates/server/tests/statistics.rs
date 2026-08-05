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

// --- Répartition par catégorie et par payeur (REQ-STA-004) ---
//
// `GET /statistics/repartition` : coûts mensuels des abonnements ACTIFS répartis sur deux axes
// (catégorie, payeur), convertis dans la devise cible. Somme des parts = total (critère #1) ; un
// abonnement sans catégorie/payeur forme une entrée explicite `label = null` (critère #2). Oracle
// legacy gelé : e2e/fixtures/oracles/REQ-STA-004-repartition.json (stats_calculations.php).

async fn repartition(
    pool: &PgPool,
    cookie: Option<&str>,
    query: &str,
) -> axum::http::Response<Body> {
    let mut b = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/statistics/repartition{query}"));
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    app(pool.clone())
        .oneshot(b.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

/// Crée une catégorie et renvoie son id.
async fn create_category(pool: &PgPool, cookie: &str, name: &str) -> String {
    let r = post(
        pool,
        "/api/v1/categories",
        json!({ "name": name }),
        Some(cookie),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    body_json(r).await["id"].as_str().unwrap().to_string()
}

/// Crée un payeur et renvoie son id.
async fn create_payer(pool: &PgPool, cookie: &str, name: &str) -> String {
    let r = post(
        pool,
        "/api/v1/payers",
        json!({ "name": name }),
        Some(cookie),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    body_json(r).await["id"].as_str().unwrap().to_string()
}

/// Crée un abonnement mensuel EUR actif, éventuellement rattaché à une catégorie et/ou un payeur.
async fn create_axis_sub(
    pool: &PgPool,
    cookie: &str,
    name: &str,
    amount: &str,
    category: Option<&str>,
    payer: Option<&str>,
) -> StatusCode {
    let mut body = json!({
        "name": name,
        "amount": amount,
        "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": "2020-01-01",
        "active": true
    });
    if let Some(c) = category {
        body["category"] = json!(c);
    }
    if let Some(p) = payer {
        body["payer"] = json!(p);
    }
    post(pool, "/api/v1/subscriptions", body, Some(cookie))
        .await
        .status()
}

/// Extrait les couples (label, total) d'un axe ; `label` absent (entrée « sans axe ») → `None`.
fn axis(body: &Value, key: &str) -> Vec<(Option<String>, String)> {
    body[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e.get("label").and_then(Value::as_str).map(str::to_string),
                e["total"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004, case = "somme des parts par axe = total général (exemple travaillé gelé)")]
async fn repartition_sums_to_the_grand_total(pool: PgPool) {
    let web = account(&pool, "sta004-sum@example.com").await;
    let streaming = create_category(&pool, &web, "Streaming").await;
    let alex = create_payer(&pool, &web, "Alex").await;
    // Exemple gelé : Netflix 10 + Spotify 10 (Streaming/Alex), Presse 5 (sans catégorie ni payeur).
    assert_eq!(
        create_axis_sub(
            &pool,
            &web,
            "Netflix",
            "10.00",
            Some(&streaming),
            Some(&alex)
        )
        .await,
        StatusCode::CREATED
    );
    assert_eq!(
        create_axis_sub(
            &pool,
            &web,
            "Spotify",
            "10.00",
            Some(&streaming),
            Some(&alex)
        )
        .await,
        StatusCode::CREATED
    );
    assert_eq!(
        create_axis_sub(&pool, &web, "Presse", "5.00", None, None).await,
        StatusCode::CREATED
    );
    let body = body_json(repartition(&pool, Some(&web), "").await).await;
    assert_eq!(body["currency"], "EUR");
    assert_eq!(body["complete"], true);
    assert_eq!(body["total"], "25.00");
    // Axe catégorie : Streaming = 20 (le plus lourd), puis « (aucun) » = 5.
    assert_eq!(
        axis(&body, "by_category"),
        vec![
            (Some("Streaming".to_string()), "20.00".to_string()),
            (None, "5.00".to_string()),
        ]
    );
    // Axe payeur : Alex = 20, puis « (aucun) » = 5.
    assert_eq!(
        axis(&body, "by_payer"),
        vec![
            (Some("Alex".to_string()), "20.00".to_string()),
            (None, "5.00".to_string()),
        ]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004, case = "abonnement sans axe -> entrée explicite label=null, jamais omise")]
async fn missing_axis_yields_an_explicit_null_entry(pool: PgPool) {
    let web = account(&pool, "sta004-null@example.com").await;
    // Un seul abonnement, sans catégorie ni payeur : chaque axe a une unique entrée « sans » (null).
    assert_eq!(
        create_axis_sub(&pool, &web, "Orphan", "12.00", None, None).await,
        StatusCode::CREATED
    );
    let body = body_json(repartition(&pool, Some(&web), "").await).await;
    assert_eq!(body["total"], "12.00");
    for key in ["by_category", "by_payer"] {
        let entries = body[key].as_array().unwrap();
        assert_eq!(entries.len(), 1, "{key}");
        // « label » absent (sérialisé `skip_if None`) => l'interface rend « (aucun) », jamais omis.
        assert!(entries[0].get("label").is_none(), "{key} label present");
        assert_eq!(entries[0]["total"], "12.00");
        assert_eq!(entries[0]["count"], 1);
    }
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004, case = "abonnement inactif exclu de la répartition et du total")]
async fn inactive_subscription_is_excluded(pool: PgPool) {
    let web = account(&pool, "sta004-inactive@example.com").await;
    let cat = create_category(&pool, &web, "Cat").await;
    assert_eq!(
        create_axis_sub(&pool, &web, "Active", "10.00", Some(&cat), None).await,
        StatusCode::CREATED
    );
    // Inactif : ne pèse ni sur les buckets ni sur le total.
    assert_eq!(
        create_sub(&pool, &web, "Disabled", "99.00", "EUR", "2020-01-01", false).await,
        StatusCode::CREATED
    );
    let body = body_json(repartition(&pool, Some(&web), "").await).await;
    assert_eq!(body["total"], "10.00");
    assert_eq!(
        axis(&body, "by_category"),
        vec![(Some("Cat".to_string()), "10.00".to_string())]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004, case = "coûts convertis dans la devise de référence (REQ-CUR-003)")]
async fn repartition_amounts_are_converted(pool: PgPool) {
    seed_rate(&pool, "USD", "EUR", "0.50").await;
    let web = account(&pool, "sta004-convert@example.com").await;
    let cat = create_category(&pool, &web, "US").await;
    // 100 USD/mois -> 50 EUR.
    let body = json!({
        "name": "Dollar", "amount": "100.00", "currency": "USD",
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": "2020-01-01", "active": true, "category": cat
    });
    assert_eq!(
        post(&pool, "/api/v1/subscriptions", body, Some(&web))
            .await
            .status(),
        StatusCode::CREATED
    );
    let body = body_json(repartition(&pool, Some(&web), "").await).await;
    assert_eq!(body["currency"], "EUR");
    assert_eq!(body["complete"], true);
    assert_eq!(body["total"], "50.00");
    assert_eq!(
        axis(&body, "by_category"),
        vec![(Some("US".to_string()), "50.00".to_string())]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004, case = "abonnement non convertible exclu, répartition marquée partielle")]
async fn repartition_missing_rate_marks_incomplete(pool: PgPool) {
    // Aucun taux JPY->EUR : l'abonnement JPY est exclu des deux axes et `complete = false` (jamais nul silencieux).
    let web = account(&pool, "sta004-partial@example.com").await;
    let cat = create_category(&pool, &web, "Local").await;
    assert_eq!(
        create_axis_sub(&pool, &web, "Euro", "10.00", Some(&cat), None).await,
        StatusCode::CREATED
    );
    let body = json!({
        "name": "Yen", "amount": "4200", "currency": "JPY",
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": "2020-01-01", "active": true, "category": cat
    });
    assert_eq!(
        post(&pool, "/api/v1/subscriptions", body, Some(&web))
            .await
            .status(),
        StatusCode::CREATED
    );
    let body = body_json(repartition(&pool, Some(&web), "").await).await;
    assert_eq!(body["complete"], false);
    // Seul l'abonnement convertible pèse ; le total reflète l'exclusion (10, pas un mélange faussé).
    assert_eq!(body["total"], "10.00");
    assert_eq!(
        axis(&body, "by_category"),
        vec![(Some("Local".to_string()), "10.00".to_string())]
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004, case = "parts triées par coût décroissant")]
async fn axes_are_sorted_by_cost_descending(pool: PgPool) {
    let web = account(&pool, "sta004-sort@example.com").await;
    let small = create_category(&pool, &web, "Small").await;
    let big = create_category(&pool, &web, "Big").await;
    assert_eq!(
        create_axis_sub(&pool, &web, "s", "3.00", Some(&small), None).await,
        StatusCode::CREATED
    );
    assert_eq!(
        create_axis_sub(&pool, &web, "b", "8.00", Some(&big), None).await,
        StatusCode::CREATED
    );
    let body = body_json(repartition(&pool, Some(&web), "").await).await;
    // Big (8) avant Small (3) : ordre décroissant.
    assert_eq!(
        axis(&body, "by_category"),
        vec![
            (Some("Big".to_string()), "8.00".to_string()),
            (Some("Small".to_string()), "3.00".to_string()),
        ]
    );
}

// --- Autorisation (§9) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004)]
async fn authz_owner_get_repartition(pool: PgPool) {
    let web = account(&pool, "own-sta004@example.com").await;
    let cat = create_category(&pool, &web, "Mine").await;
    assert_eq!(
        create_axis_sub(&pool, &web, "Mine", "10.00", Some(&cat), None).await,
        StatusCode::CREATED
    );
    let r = repartition(&pool, Some(&web), "").await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(body_json(r).await["total"], "10.00");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004)]
async fn authz_other_get_repartition(pool: PgPool) {
    // Un autre foyer ne voit jamais les abonnements d'autrui : répartition vide, total nul (§9).
    let a = account(&pool, "a-sta004@example.com").await;
    let cat = create_category(&pool, &a, "A").await;
    assert_eq!(
        create_axis_sub(&pool, &a, "A-only", "10.00", Some(&cat), None).await,
        StatusCode::CREATED
    );
    let b = account(&pool, "b-sta004@example.com").await;
    let body = body_json(repartition(&pool, Some(&b), "").await).await;
    assert_eq!(body["total"], "0.00");
    assert!(body["by_category"].as_array().unwrap().is_empty());
    assert!(body["by_payer"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-STA-004)]
async fn authz_anon_get_repartition(pool: PgPool) {
    assert_eq!(
        repartition(&pool, None, "").await.status(),
        StatusCode::UNAUTHORIZED
    );
}
