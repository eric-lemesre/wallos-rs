//! Tests d'intégration des catégories (REQ-CAT-001).
//!
//! CRUD isolé par foyer : create/list/rename/delete n'affectent que les catégories de l'appelant.
//! Autorisation §9 : propriétaire 2xx, tiers authentifié 404 (jamais 403), anonyme 401.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashSet;
use tower::ServiceExt;
use wallos_core::language::Language;
use wallos_core::{DEFAULT_CATEGORY_COUNT, default_category_names};
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
    body: Option<serde_json::Value>,
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

async fn signup(pool: &PgPool, email: &str) {
    let r = send(
        pool,
        "POST",
        "/api/v1/accounts",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

/// Inscription en précisant la langue (REQ-CAT-002 / I18N-001) ; renvoie le statut HTTP.
async fn signup_with_language(pool: &PgPool, email: &str, language: &str) -> StatusCode {
    send(
        pool,
        "POST",
        "/api/v1/accounts",
        None,
        Some(json!({ "email": email, "password": PASSWORD, "language": language })),
    )
    .await
    .status()
}

async fn login_cookie(pool: &PgPool, email: &str) -> String {
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

async fn account(pool: &PgPool, email: &str) -> String {
    signup(pool, email).await;
    login_cookie(pool, email).await
}

async fn create_category(pool: &PgPool, cookie: &str, name: &str) -> axum::http::Response<Body> {
    send(
        pool,
        "POST",
        "/api/v1/categories",
        Some(cookie),
        Some(json!({ "name": name })),
    )
    .await
}

async fn categories(pool: &PgPool, cookie: &str) -> Vec<serde_json::Value> {
    let r = send(pool, "GET", "/api/v1/categories", Some(cookie), None).await;
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Catégories **hors jeu par défaut** (REQ-CAT-002) : depuis le seed automatique, chaque compte naît
/// avec [`DEFAULT_CATEGORY_COUNT`] catégories. Les tests CRUD/isolation raisonnent sur les catégories
/// que le test crée lui-même ; ce filtre retire les défauts pour préserver leur intention d'origine.
/// (Les comptes des tests sont créés sans langue → jeu par défaut anglais.)
async fn custom_categories(pool: &PgPool, cookie: &str) -> Vec<serde_json::Value> {
    let defaults: HashSet<&str> = default_category_names(Language::English)
        .into_iter()
        .collect();
    categories(pool, cookie)
        .await
        .into_iter()
        .filter(|c| !defaults.contains(c["name"].as_str().unwrap_or_default()))
        .collect()
}

async fn created_id(r: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["id"].as_str().unwrap().to_string()
}

// --- REQ-CAT-002 : jeu de catégories par défaut à la création du compte ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-002, case = "compte fraîchement créé → jeu par défaut présent, dans la langue du compte")]
async fn new_account_has_default_categories_in_its_language(pool: PgPool) {
    // Compte créé en français : le formulaire d'abonnement (qui consomme GET /categories) offre
    // d'emblée le jeu par défaut traduit — jamais une liste vide (acceptance CAT-002).
    assert_eq!(
        signup_with_language(&pool, "fr-user@example.com", "fr").await,
        StatusCode::CREATED
    );
    let fr = login_cookie(&pool, "fr-user@example.com").await;
    let names_fr: Vec<String> = categories(&pool, &fr)
        .await
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names_fr.len(),
        DEFAULT_CATEGORY_COUNT,
        "liste non vide et complète"
    );
    for expected in default_category_names(Language::French) {
        assert!(
            names_fr.iter().any(|n| n == expected),
            "catégorie par défaut FR manquante: {expected:?}"
        );
    }

    // Compte créé sans langue : jeu par défaut en anglais (langue de base).
    let en = account(&pool, "en-user@example.com").await;
    let names_en: Vec<String> = categories(&pool, &en)
        .await
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names_en.len(), DEFAULT_CATEGORY_COUNT);
    assert!(names_en.iter().any(|n| n == "Music"));
    assert!(!names_en.iter().any(|n| n == "Musique"));
    // Sentinelle « No category » non semée (subtrack : « sans catégorie » = category_id NULL).
    assert!(!names_en.iter().any(|n| n == "No category"));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-002, case = "langue non supportée à l'inscription → 422")]
async fn signup_with_unsupported_language_is_rejected(pool: PgPool) {
    // Cohérent avec PUT /settings/language : un code de langue non supporté est refusé (422),
    // jamais silencieusement ignoré.
    assert_eq!(
        signup_with_language(&pool, "de-user@example.com", "de").await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

// --- Parcours fonctionnels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn created_category_is_listed_immediately(pool: PgPool) {
    let web = account(&pool, "cat@example.com").await;
    let created = create_category(&pool, &web, "Streaming").await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let list = custom_categories(&pool, &web).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "Streaming");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-005)]
async fn categories_are_listed_in_a_deterministic_order(pool: PgPool) {
    // REQ-CAT-005 : l'ordre est déterministe (par nom, départage par id) et identique d'un appel à
    // l'autre — donc identique sur les trois modalités (elles consomment la même liste API).
    let web = account(&pool, "cat-order@example.com").await;
    for name in ["Gamma", "Alpha", "Beta"] {
        assert_eq!(
            create_category(&pool, &web, name).await.status(),
            StatusCode::CREATED
        );
    }
    let first: Vec<String> = custom_categories(&pool, &web)
        .await
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    // Ordre alphabétique déterministe, indépendant de l'ordre d'insertion.
    assert_eq!(first, vec!["Alpha", "Beta", "Gamma"]);
    // Stable : un second appel renvoie exactement le même ordre.
    let second: Vec<String> = custom_categories(&pool, &web)
        .await
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(first, second);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-004)]
async fn duplicate_name_in_same_household_is_rejected(pool: PgPool) {
    let web = account(&pool, "cat-dup@example.com").await;
    assert_eq!(
        create_category(&pool, &web, "Streaming").await.status(),
        StatusCode::CREATED
    );
    // Même nom -> refusé (422).
    let dup = create_category(&pool, &web, "Streaming").await;
    assert_eq!(dup.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // Insensible à la casse : « streaming » entre aussi en collision.
    assert_eq!(
        create_category(&pool, &web, "streaming").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Une seule catégorie a été créée.
    assert_eq!(custom_categories(&pool, &web).await.len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-004)]
async fn same_name_in_other_household_is_allowed(pool: PgPool) {
    let a = account(&pool, "cat-a@example.com").await;
    assert_eq!(
        create_category(&pool, &a, "Streaming").await.status(),
        StatusCode::CREATED
    );
    // Un autre foyer peut avoir sa propre catégorie du même nom (isolation §9).
    let b = account(&pool, "cat-b@example.com").await;
    assert_eq!(
        create_category(&pool, &b, "Streaming").await.status(),
        StatusCode::CREATED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-004)]
async fn rename_to_existing_name_is_rejected(pool: PgPool) {
    let web = account(&pool, "cat-rendup@example.com").await;
    assert_eq!(
        create_category(&pool, &web, "Streaming").await.status(),
        StatusCode::CREATED
    );
    let musique_id = created_id(create_category(&pool, &web, "Musique").await).await;
    // Renommer « Musique » en « Streaming » (déjà pris) -> 422.
    let r = send(
        &pool,
        "PUT",
        &format!("/api/v1/categories/{musique_id}"),
        Some(&web),
        Some(json!({ "name": "Streaming" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn rename_and_delete_own_category(pool: PgPool) {
    let web = account(&pool, "cat2@example.com").await;
    // Nom volontairement erroné ("Musci") pour illustrer la correction par renommage.
    let id = created_id(create_category(&pool, &web, "Musci").await).await;

    // Renommer (corrige la faute).
    let renamed = send(
        &pool,
        "PUT",
        &format!("/api/v1/categories/{id}"),
        Some(&web),
        Some(json!({ "name": "Musique" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    assert_eq!(custom_categories(&pool, &web).await[0]["name"], "Musique");

    // Supprimer.
    let deleted = send(
        &pool,
        "DELETE",
        &format!("/api/v1/categories/{id}"),
        Some(&web),
        None,
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    assert!(custom_categories(&pool, &web).await.is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn empty_name_is_rejected(pool: PgPool) {
    let web = account(&pool, "cat3@example.com").await;
    assert_eq!(
        create_category(&pool, &web, "   ").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

// --- Isolation (§9) : les opérations n'affectent que ses propres catégories ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn categories_are_isolated_between_accounts(pool: PgPool) {
    let alice = account(&pool, "alice-cat@example.com").await;
    let bob = account(&pool, "bob-cat@example.com").await;
    let alice_cat = created_id(create_category(&pool, &alice, "Alice Only").await).await;

    // Bob ne voit pas la catégorie d'Alice.
    assert!(custom_categories(&pool, &bob).await.is_empty());
    // Bob ne peut ni renommer ni supprimer celle d'Alice -> 404 (jamais 403).
    assert_eq!(
        send(
            &pool,
            "PUT",
            &format!("/api/v1/categories/{alice_cat}"),
            Some(&bob),
            Some(json!({ "name": "Hacked" }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{alice_cat}"),
            Some(&bob),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    // La catégorie d'Alice est intacte.
    assert_eq!(
        custom_categories(&pool, &alice).await[0]["name"],
        "Alice Only"
    );
}

// --- Cas limites d'identifiant (dans le propre foyer de l'appelant) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn rename_or_delete_nonexistent_category_is_404(pool: PgPool) {
    let web = account(&pool, "missing-cat@example.com").await;
    // UUID valide mais inexistant DANS SON PROPRE foyer -> 404 (pas une erreur 500).
    let ghost = uuid::Uuid::new_v4();
    assert_eq!(
        send(
            &pool,
            "PUT",
            &format!("/api/v1/categories/{ghost}"),
            Some(&web),
            Some(json!({ "name": "X" }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{ghost}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn malformed_category_id_is_404(pool: PgPool) {
    // Convention codebase (cf. revokeDevice) : un identifiant mal formé est traité comme inexistant
    // (404, ne divulgue rien), jamais 400/500.
    let web = account(&pool, "malformed-cat@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            "/api/v1/categories/not-a-uuid",
            Some(&web),
            Some(json!({ "name": "X" }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &pool,
            "DELETE",
            "/api/v1/categories/not-a-uuid",
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

// --- Autorisation §9 : createCategory ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_owner_create_category(pool: PgPool) {
    let web = account(&pool, "own-cc@example.com").await;
    assert_eq!(
        create_category(&pool, &web, "X").await.status(),
        StatusCode::CREATED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_other_create_category(pool: PgPool) {
    // Chaque compte crée SES propres catégories (pas de ressource d'autrui visée à la création).
    let web = account(&pool, "other-cc@example.com").await;
    assert_eq!(
        create_category(&pool, &web, "X").await.status(),
        StatusCode::CREATED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_anon_create_category(pool: PgPool) {
    assert_eq!(
        create_category(&pool, "session=nope", "X").await.status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : listCategories ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_owner_list_categories(pool: PgPool) {
    let web = account(&pool, "own-lc@example.com").await;
    assert_eq!(
        send(&pool, "GET", "/api/v1/categories", Some(&web), None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_other_list_categories(pool: PgPool) {
    // Un autre foyer ne voit jamais les catégories d'un autre : sa liste est la sienne (vide ici).
    let owner = account(&pool, "owner-lc@example.com").await;
    let _ = create_category(&pool, &owner, "Secret").await;
    let other = account(&pool, "other-lc@example.com").await;
    assert!(custom_categories(&pool, &other).await.is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_anon_list_categories(pool: PgPool) {
    assert_eq!(
        send(&pool, "GET", "/api/v1/categories", None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : renameCategory ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_owner_rename_category(pool: PgPool) {
    let web = account(&pool, "own-rc@example.com").await;
    let id = created_id(create_category(&pool, &web, "Old").await).await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            &format!("/api/v1/categories/{id}"),
            Some(&web),
            Some(json!({ "name": "New" }))
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_other_rename_category(pool: PgPool) {
    let owner = account(&pool, "owner-rc@example.com").await;
    let id = created_id(create_category(&pool, &owner, "Old").await).await;
    let other = account(&pool, "other-rc@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            &format!("/api/v1/categories/{id}"),
            Some(&other),
            Some(json!({ "name": "Hacked" }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_anon_rename_category(pool: PgPool) {
    let id = uuid::Uuid::new_v4();
    assert_eq!(
        send(
            &pool,
            "PUT",
            &format!("/api/v1/categories/{id}"),
            None,
            Some(json!({ "name": "X" }))
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : deleteCategory ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_owner_delete_category(pool: PgPool) {
    let web = account(&pool, "own-dc@example.com").await;
    let id = created_id(create_category(&pool, &web, "Tmp").await).await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{id}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_other_delete_category(pool: PgPool) {
    let owner = account(&pool, "owner-dc@example.com").await;
    let id = created_id(create_category(&pool, &owner, "Tmp").await).await;
    let other = account(&pool, "other-dc@example.com").await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{id}"),
            Some(&other),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn authz_anon_delete_category(pool: PgPool) {
    let id = uuid::Uuid::new_v4();
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{id}"),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- REQ-CAT-003 : suppression d'une catégorie référencée (oracle Wallos, §8.1) ---

/// Crée un abonnement rattaché (ou non) à une catégorie, dans le foyer de l'appelant.
async fn create_subscription(
    pool: &PgPool,
    cookie: &str,
    name: &str,
    category: Option<&str>,
) -> axum::http::Response<Body> {
    let mut body = json!({
        "name": name,
        "amount": "9.99",
        "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": "2030-01-15",
    });
    if let Some(c) = category {
        body["category"] = json!(c);
    }
    send(
        pool,
        "POST",
        "/api/v1/subscriptions",
        Some(cookie),
        Some(body),
    )
    .await
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-003, case = "catégorie référencée -> suppression refusée (409), catégorie intacte")]
async fn referenced_category_cannot_be_deleted(pool: PgPool) {
    // Comportement de référence capturé sur Wallos 5.4.2 (§8.1, handleDeleteCategory) : une catégorie
    // référencée par au moins un abonnement du compte NE PEUT PAS être supprimée (`category_in_use`).
    let web = account(&pool, "cat-del-inuse@example.com").await;
    let cat_id = created_id(create_category(&pool, &web, "Streaming").await).await;
    assert_eq!(
        create_subscription(&pool, &web, "Netflix", Some(&cat_id))
            .await
            .status(),
        StatusCode::CREATED
    );

    // Suppression refusée : 409 (jamais 204, jamais 404), la catégorie reste présente.
    let refused = send(
        &pool,
        "DELETE",
        &format!("/api/v1/categories/{cat_id}"),
        Some(&web),
        None,
    )
    .await;
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    assert_eq!(custom_categories(&pool, &web).await[0]["name"], "Streaming");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-003, case = "aucun abonnement ne référence une catégorie inexistante")]
async fn category_becomes_deletable_once_no_longer_referenced(pool: PgPool) {
    let web = account(&pool, "cat-del-free@example.com").await;
    let cat_id = created_id(create_category(&pool, &web, "Streaming").await).await;
    let sub = create_subscription(&pool, &web, "Netflix", Some(&cat_id)).await;
    assert_eq!(sub.status(), StatusCode::CREATED);
    let sub_id = created_id(sub).await;

    // Tant que l'abonnement référence la catégorie, la suppression est refusée (409).
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{cat_id}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );

    // On détache l'abonnement de la catégorie (PUT sans `category` -> category_id NULL) : la catégorie
    // n'est plus référencée -> suppression autorisée (204).
    let detach = send(
        &pool,
        "PUT",
        &format!("/api/v1/subscriptions/{sub_id}"),
        Some(&web),
        Some(json!({
            "name": "Netflix",
            "amount": "9.99",
            "currency": "EUR",
            "cycle": { "unit": "month", "interval": 1 },
            "first_payment": "2030-01-15",
        })),
    )
    .await;
    assert_eq!(detach.status(), StatusCode::OK);
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{cat_id}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    // Critère #2 : plus aucune catégorie (donc aucun abonnement ne pointe vers une catégorie fantôme).
    assert!(custom_categories(&pool, &web).await.is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-003, case = "un abonnement d'un autre foyer ne bloque pas la suppression (isolation §9)")]
async fn reference_from_another_household_does_not_block_deletion(pool: PgPool) {
    // Isolation : seul le comptage des abonnements DU foyer de l'appelant bloque la suppression.
    let alice = account(&pool, "cat-del-iso-a@example.com").await;
    let bob = account(&pool, "cat-del-iso-b@example.com").await;
    // Alice crée une catégorie et un abonnement qui la référence.
    let alice_cat = created_id(create_category(&pool, &alice, "Streaming").await).await;
    assert_eq!(
        create_subscription(&pool, &alice, "Netflix", Some(&alice_cat))
            .await
            .status(),
        StatusCode::CREATED
    );
    // Bob crée SA propre catégorie (même nom autorisé, foyers distincts) SANS abonnement référent.
    let bob_cat = created_id(create_category(&pool, &bob, "Streaming").await).await;
    // Bob peut supprimer sa catégorie : les abonnements d'Alice ne comptent pas pour Bob.
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{bob_cat}"),
            Some(&bob),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    // La catégorie référencée d'Alice, elle, reste protégée (409).
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/categories/{alice_cat}"),
            Some(&alice),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

// --- REQ-SYN-001 : identifiant stable généré côté client + horodatage serveur ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-001, case = "l'UUID généré côté client est conservé à la création")]
async fn client_provided_uuid_is_preserved(pool: PgPool) {
    let web = account(&pool, "syn-cat@example.com").await;
    // Une catégorie « créée hors ligne » porte déjà son UUID client ; poussée au serveur, il est conservé.
    let client_id = uuid::Uuid::new_v4().to_string();
    let created = send(
        &pool,
        "POST",
        "/api/v1/categories",
        Some(&web),
        Some(json!({ "id": client_id, "name": "Streaming" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(created_id(created).await, client_id);
    // La liste renvoie bien l'entité sous l'identifiant fourni par le client.
    let list = custom_categories(&pool, &web).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], client_id);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-001, case = "un id fourni non-UUID est rejeté (422)")]
async fn invalid_client_provided_id_is_rejected(pool: PgPool) {
    let web = account(&pool, "syn-cat-bad@example.com").await;
    let bad = send(
        &pool,
        "POST",
        "/api/v1/categories",
        Some(&web),
        Some(json!({ "id": "not-a-uuid", "name": "Streaming" })),
    )
    .await;
    assert_eq!(bad.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // Rien n'a été créé.
    assert_eq!(custom_categories(&pool, &web).await.len(), 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-001, case = "l'horodatage de modification est fourni par le serveur et avancé")]
async fn modification_timestamp_is_server_provided(pool: PgPool) {
    use chrono::{DateTime, Utc};

    let web = account(&pool, "syn-cat-ts@example.com").await;
    let client_id = uuid::Uuid::new_v4();
    let created = send(
        &pool,
        "POST",
        "/api/v1/categories",
        Some(&web),
        Some(json!({ "id": client_id.to_string(), "name": "Streaming" })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    // À la création, l'horodatage est posé par le serveur (= created_at, même instant d'insertion).
    let (created_at, updated_at): (DateTime<Utc>, DateTime<Utc>) =
        sqlx::query_as("select created_at, updated_at from categories where id = $1")
            .bind(client_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(created_at, updated_at);

    // Une modification ultérieure **avance** l'horodatage (fourni par l'horloge serveur, jamais le client).
    tokio::time::sleep(std::time::Duration::from_millis(50)).await; // marge robuste (revue SYN-001 F3)
    let renamed = send(
        &pool,
        "PUT",
        &format!("/api/v1/categories/{client_id}"),
        Some(&web),
        Some(json!({ "name": "Musique" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);

    let after: DateTime<Utc> =
        sqlx::query_scalar("select updated_at from categories where id = $1")
            .bind(client_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        after > created_at,
        "updated_at doit avancer après modification"
    );
}

// --- REQ-SYN-006 : idempotence des opérations d'écriture ---

/// POST /categories en portant un en-tête `Idempotency-Key`.
async fn create_with_key(
    pool: &PgPool,
    cookie: &str,
    name: &str,
    key: &str,
) -> axum::http::Response<Body> {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/categories")
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .header("idempotency-key", key)
        .body(Body::from(json!({ "name": name }).to_string()))
        .unwrap();
    app(pool.clone()).oneshot(req).await.unwrap()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-006, case = "rejeu clé+corps identiques : même réponse, aucun doublon")]
async fn idempotent_replay_returns_same_response_without_side_effect(pool: PgPool) {
    let web = account(&pool, "idem@example.com").await;
    let first = create_with_key(&pool, &web, "Streaming", "key-abc").await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let id1 = created_id(first).await;

    // Rejeu avec la même clé et le même corps : réponse identique (même id), sans nouvelle création.
    let replay = create_with_key(&pool, &web, "Streaming", "key-abc").await;
    assert_eq!(replay.status(), StatusCode::CREATED);
    assert_eq!(created_id(replay).await, id1);

    // Aucun effet de bord supplémentaire : une seule catégorie existe.
    assert_eq!(custom_categories(&pool, &web).await.len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-006, case = "clé réutilisée avec un corps différent : 409")]
async fn idempotency_key_reused_with_different_body_is_conflict(pool: PgPool) {
    let web = account(&pool, "idem-conflict@example.com").await;
    assert_eq!(
        create_with_key(&pool, &web, "Streaming", "key-x")
            .await
            .status(),
        StatusCode::CREATED
    );
    // Même clé, corps différent -> conflit (409).
    let conflict = create_with_key(&pool, &web, "Musique", "key-x").await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    // La seconde n'a rien créé : une seule catégorie (l'originale).
    let names: Vec<String> = custom_categories(&pool, &web)
        .await
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["Streaming"]);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-006, case = "portée par utilisateur : même clé, comptes distincts")]
async fn idempotency_keys_are_scoped_per_user(pool: PgPool) {
    let a = account(&pool, "idem-a@example.com").await;
    let b = account(&pool, "idem-b@example.com").await;
    // Les deux comptes emploient la même clé : aucune interférence (isolation §9).
    assert_eq!(
        create_with_key(&pool, &a, "Streaming", "shared")
            .await
            .status(),
        StatusCode::CREATED
    );
    let b_resp = create_with_key(&pool, &b, "Musique", "shared").await;
    assert_eq!(b_resp.status(), StatusCode::CREATED);
    // Chacun voit sa propre catégorie.
    assert_eq!(custom_categories(&pool, &a).await[0]["name"], "Streaming");
    assert_eq!(custom_categories(&pool, &b).await[0]["name"], "Musique");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-006, case = "sans clé : le comportement est inchangé (idempotence opt-in)")]
async fn without_key_creation_is_unaffected(pool: PgPool) {
    let web = account(&pool, "idem-nokey@example.com").await;
    assert_eq!(
        create_category(&pool, &web, "Streaming").await.status(),
        StatusCode::CREATED
    );
    assert_eq!(custom_categories(&pool, &web).await.len(), 1);
}

// --- REQ-SYN-001 (revue F1) : collision d'id client distincte du doublon de nom ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-001, case = "id client déjà pris → 409, jamais un message de doublon de nom")]
async fn client_provided_id_collision_is_conflict(pool: PgPool) {
    let web = account(&pool, "syn-cat-idcol@example.com").await;
    let id = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        send(
            &pool,
            "POST",
            "/api/v1/categories",
            Some(&web),
            Some(json!({ "id": id, "name": "Streaming" })),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    // Même id, nom **différent** (sans clé d'idempotence) : collision de clé primaire → 409 (pas 422).
    let conflict = send(
        &pool,
        "POST",
        "/api/v1/categories",
        Some(&web),
        Some(json!({ "id": id, "name": "Musique" })),
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    // Le doublon de **nom**, lui, reste un 422 (unicité métier CAT-004) — comportement inchangé.
    assert_eq!(
        create_category(&pool, &web, "Streaming").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

// --- REQ-SYN-006 (revue F3/F5) : concurrence réelle + rejeu après erreur retryable ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-006, case = "concurrence : deux rejeux simultanés → un seul effet de bord")]
async fn concurrent_replays_produce_a_single_side_effect(pool: PgPool) {
    let web = account(&pool, "idem-conc@example.com").await;
    // Deux requêtes **strictement simultanées** avec la même clé + le même corps.
    let (r1, r2) = tokio::join!(
        create_with_key(&pool, &web, "Streaming", "conc-key"),
        create_with_key(&pool, &web, "Streaming", "conc-key"),
    );
    // La réservation atomique (`insert ... on conflict do nothing`) garantit un unique effet de bord :
    // au plus une catégorie est créée, quel que soit l'entrelacement.
    assert_eq!(custom_categories(&pool, &web).await.len(), 1);
    // Au moins une requête réussit (201) ; l'autre est soit un rejeu (201) soit un conflit en cours (409).
    let statuses = [r1.status(), r2.status()];
    assert!(
        statuses.contains(&StatusCode::CREATED),
        "au moins une création réussie, obtenu {statuses:?}"
    );
    assert!(
        statuses
            .iter()
            .all(|s| *s == StatusCode::CREATED || *s == StatusCode::CONFLICT),
        "chaque réponse est 201 ou 409, obtenu {statuses:?}"
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-006, case = "erreur retryable relâchée : rejeu ultérieur avec la même clé réussit")]
async fn retryable_error_releases_the_key(pool: PgPool) {
    let web = account(&pool, "idem-retry@example.com").await;
    // Corps invalide (nom vide) avec une clé : 422 — la réservation est **relâchée** (non mémorisée).
    let invalid = create_with_key(&pool, &web, "   ", "retry-key").await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // Même clé, corps valide : la création réussit (la clé n'a pas été bloquée par l'échec précédent).
    let ok = create_with_key(&pool, &web, "Streaming", "retry-key").await;
    assert_eq!(ok.status(), StatusCode::CREATED);
    assert_eq!(custom_categories(&pool, &web).await.len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-006, case = "réservation PENDING orpheline (handler mort) récupérée après TTL")]
async fn stale_pending_reservation_is_reclaimed(pool: PgPool) {
    let email = "idem-stale@example.com";
    let web = account(&pool, email).await;
    let user_id: uuid::Uuid = sqlx::query_scalar("select id from users where email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    // Simule un handler mort entre `reserve` et `complete`/`release` : une réservation `PENDING`
    // orpheline vieille de 10 minutes (> TTL 5 min).
    sqlx::query(
        "insert into idempotency_keys \
             (user_id, idempotency_key, request_fingerprint, response_status, response_body, created_at) \
         values ($1, 'stale', 'orphan', 0, '', now() - interval '10 minutes')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    // La clé n'est **pas** bloquée à vie : la réservation orpheline est récupérée, la création réussit.
    let r = create_with_key(&pool, &web, "Streaming", "stale").await;
    assert_eq!(r.status(), StatusCode::CREATED);
    assert_eq!(custom_categories(&pool, &web).await.len(), 1);
}
