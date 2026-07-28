//! Tests d'intégration des réglages du foyer — devise de référence (REQ-CUR-001).
//!
//! Réglage par foyer (§9) : chaque foyer a sa devise de référence (singleton, jamais 404). Modifier
//! la devise de référence change la devise cible des agrégats **sans altérer les montants saisis**.

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

async fn body_json(r: axum::http::Response<Body>) -> Value {
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

const URI: &str = "/api/v1/settings/reference-currency";

async fn get_currency(pool: &PgPool, cookie: &str) -> String {
    let r = send(pool, "GET", URI, Some(cookie), None).await;
    assert_eq!(r.status(), StatusCode::OK);
    body_json(r).await["currency"].as_str().unwrap().to_string()
}

// --- Fonctionnel ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn defaults_to_eur_then_persists_a_new_choice(pool: PgPool) {
    let web = account(&pool, "cur1@example.com").await;
    // Défaut : EUR (aligné sur ReferenceCurrency::DEFAULT_CODE).
    assert_eq!(get_currency(&pool, &web).await, "EUR");

    // Modification vers USD : persistée, relue à l'identique.
    let r = send(
        &pool,
        "PUT",
        URI,
        Some(&web),
        Some(json!({ "currency": "USD" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(body_json(r).await["currency"], "USD");
    assert_eq!(get_currency(&pool, &web).await, "USD");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn unsupported_currency_is_rejected(pool: PgPool) {
    let web = account(&pool, "cur-bad@example.com").await;
    let r = send(
        &pool,
        "PUT",
        URI,
        Some(&web),
        Some(json!({ "currency": "ZZZ" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // Le réglage reste au défaut (inchangé).
    assert_eq!(get_currency(&pool, &web).await, "EUR");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn subscription_total_is_expressed_in_the_reference_currency(pool: PgPool) {
    // Acceptance #1 : le total des agrégats s'exprime dans la devise de référence choisie.
    let web = account(&pool, "cur-total@example.com").await;
    let created = send(
        &pool,
        "POST",
        "/api/v1/subscriptions",
        Some(&web),
        Some(json!({
            "name": "Netflix", "amount": "9.99", "currency": "EUR",
            "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2030-01-15"
        })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    // Par défaut (EUR) : le total est en EUR.
    let list = send(&pool, "GET", "/api/v1/subscriptions", Some(&web), None).await;
    assert_eq!(body_json(list).await["total"]["currency"], "EUR");

    // Après passage en USD, le total est exprimé en USD (montant saisi inchangé : toujours EUR 9.99).
    assert_eq!(
        send(
            &pool,
            "PUT",
            URI,
            Some(&web),
            Some(json!({ "currency": "USD" }))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let list = body_json(send(&pool, "GET", "/api/v1/subscriptions", Some(&web), None).await).await;
    assert_eq!(list["total"]["currency"], "USD");
    // Revue CUR-001 : le total est **recalculé**, pas seulement réétiqueté. Sans taux EUR->USD connu,
    // le montant EUR est exclu de l'agrégat (jamais réétiqueté "9.99 USD") -> total nul + incomplet.
    assert_eq!(list["total"]["total"], "0");
    assert_eq!(list["total"]["complete"], false);
    // Acceptance #2 : le montant/devise d'origine de l'abonnement sont conservés à l'identique.
    assert_eq!(list["subscriptions"][0]["amount"], "9.99");
    assert_eq!(list["subscriptions"][0]["currency"], "EUR");
}

// --- Autorisation §9 : getReferenceCurrency (singleton par foyer : autre foyer = 200 sur le sien) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn authz_owner_get_reference_currency(pool: PgPool) {
    let web = account(&pool, "own-grc@example.com").await;
    assert_eq!(
        send(&pool, "GET", URI, Some(&web), None).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn authz_other_get_reference_currency(pool: PgPool) {
    // Un autre foyer lit **sa propre** devise de référence (jamais celle d'autrui) : son défaut EUR.
    let owner = account(&pool, "owner-grc@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            URI,
            Some(&owner),
            Some(json!({ "currency": "USD" }))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let other = account(&pool, "other-grc@example.com").await;
    assert_eq!(get_currency(&pool, &other).await, "EUR"); // isolé : voit son défaut, pas l'USD du owner
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn authz_anon_get_reference_currency(pool: PgPool) {
    assert_eq!(
        send(&pool, "GET", URI, None, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : setReferenceCurrency ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn authz_owner_set_reference_currency(pool: PgPool) {
    let web = account(&pool, "own-src@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            URI,
            Some(&web),
            Some(json!({ "currency": "GBP" }))
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn authz_other_set_reference_currency(pool: PgPool) {
    // Chaque foyer ne modifie que le sien ; l'écriture d'un autre n'affecte pas le foyer d'origine.
    let owner = account(&pool, "owner-src@example.com").await;
    let other = account(&pool, "other-src@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            URI,
            Some(&other),
            Some(json!({ "currency": "GBP" }))
        )
        .await
        .status(),
        StatusCode::OK
    );
    // Le foyer du owner reste à son défaut (isolation).
    assert_eq!(get_currency(&pool, &owner).await, "EUR");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-001)]
async fn authz_anon_set_reference_currency(pool: PgPool) {
    assert_eq!(
        send(&pool, "PUT", URI, None, Some(json!({ "currency": "USD" })))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- REQ-I18N-001 : langue de l'utilisateur ---

const LANG_URI: &str = "/api/v1/settings/language";

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-I18N-001)]
async fn language_defaults_absent_then_persists_a_choice(pool: PgPool) {
    let web = account(&pool, "lang1@example.com").await;
    // Non renseignée au départ : la réponse n'a pas de champ `language` (l'UI applique le système).
    let r = send(&pool, "GET", LANG_URI, Some(&web), None).await;
    assert_eq!(r.status(), StatusCode::OK);
    assert!(body_json(r).await.get("language").is_none());

    // Choix `fr` : persisté, relu à l'identique.
    let r = send(
        &pool,
        "PUT",
        LANG_URI,
        Some(&web),
        Some(json!({ "language": "fr" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(body_json(r).await["language"], "fr");
    let r = send(&pool, "GET", LANG_URI, Some(&web), None).await;
    assert_eq!(body_json(r).await["language"], "fr");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-I18N-001)]
async fn unsupported_language_is_rejected(pool: PgPool) {
    let web = account(&pool, "lang-bad@example.com").await;
    // Revue I18N-001 #5 : divers codes non supportés rejetés (casse, vide, code long, locale).
    for bad in ["de", "FR", "", "english", "fr-FR"] {
        assert_eq!(
            send(
                &pool,
                "PUT",
                LANG_URI,
                Some(&web),
                Some(json!({ "language": bad })),
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "code non supporté accepté à tort: {bad:?}"
        );
    }
    // Reste non renseignée (inchangée).
    let r = send(&pool, "GET", LANG_URI, Some(&web), None).await;
    assert!(body_json(r).await.get("language").is_none());
}

// --- Autorisation §9 : getLanguage ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-I18N-001)]
async fn authz_owner_get_language(pool: PgPool) {
    let web = account(&pool, "own-gl@example.com").await;
    assert_eq!(
        send(&pool, "GET", LANG_URI, Some(&web), None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-I18N-001)]
async fn authz_other_get_language(pool: PgPool) {
    // Chaque utilisateur lit **sa propre** langue : un autre ne voit jamais le choix d'autrui.
    let owner = account(&pool, "owner-gl@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            LANG_URI,
            Some(&owner),
            Some(json!({ "language": "fr" }))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let other = account(&pool, "other-gl@example.com").await;
    let r = send(&pool, "GET", LANG_URI, Some(&other), None).await;
    assert_eq!(r.status(), StatusCode::OK);
    assert!(body_json(r).await.get("language").is_none()); // son propre défaut, pas le `fr` du owner
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-I18N-001)]
async fn authz_anon_get_language(pool: PgPool) {
    assert_eq!(
        send(&pool, "GET", LANG_URI, None, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : setLanguage ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-I18N-001)]
async fn authz_owner_set_language(pool: PgPool) {
    let web = account(&pool, "own-sl@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            LANG_URI,
            Some(&web),
            Some(json!({ "language": "en" }))
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-I18N-001)]
async fn authz_other_set_language(pool: PgPool) {
    // Chaque utilisateur ne modifie que sa propre langue ; celle du owner reste inchangée.
    let owner = account(&pool, "owner-sl@example.com").await;
    let other = account(&pool, "other-sl@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            LANG_URI,
            Some(&other),
            Some(json!({ "language": "fr" }))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let r = send(&pool, "GET", LANG_URI, Some(&owner), None).await;
    assert!(body_json(r).await.get("language").is_none());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-I18N-001)]
async fn authz_anon_set_language(pool: PgPool) {
    assert_eq!(
        send(
            &pool,
            "PUT",
            LANG_URI,
            None,
            Some(json!({ "language": "fr" }))
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
