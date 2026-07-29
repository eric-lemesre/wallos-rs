//! Tests d'intégration des catégories (REQ-CAT-001).
//!
//! CRUD isolé par foyer : create/list/rename/delete n'affectent que les catégories de l'appelant.
//! Autorisation §9 : propriétaire 2xx, tiers authentifié 404 (jamais 403), anonyme 401.

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

async fn created_id(r: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["id"].as_str().unwrap().to_string()
}

// --- Parcours fonctionnels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CAT-001)]
async fn created_category_is_listed_immediately(pool: PgPool) {
    let web = account(&pool, "cat@example.com").await;
    let created = create_category(&pool, &web, "Streaming").await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let list = categories(&pool, &web).await;
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
    let first: Vec<String> = categories(&pool, &web)
        .await
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    // Ordre alphabétique déterministe, indépendant de l'ordre d'insertion.
    assert_eq!(first, vec!["Alpha", "Beta", "Gamma"]);
    // Stable : un second appel renvoie exactement le même ordre.
    let second: Vec<String> = categories(&pool, &web)
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
    assert_eq!(categories(&pool, &web).await.len(), 1);
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
    assert_eq!(categories(&pool, &web).await[0]["name"], "Musique");

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
    assert!(categories(&pool, &web).await.is_empty());
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
    assert!(categories(&pool, &bob).await.is_empty());
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
    assert_eq!(categories(&pool, &alice).await[0]["name"], "Alice Only");
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
    assert!(categories(&pool, &other).await.is_empty());
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
    let list = categories(&pool, &web).await;
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
    assert_eq!(categories(&pool, &web).await.len(), 0);
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
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
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
