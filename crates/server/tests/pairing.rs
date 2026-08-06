//! Tests d'intégration de l'appairage et de la synchronisation initiale (REQ-SYN-008).
//!
//! Un appareil **appairé** (jeton d'appareil, REQ-AUT-005) récupère l'intégralité des données du compte
//! via le delta incrémental (`GET /sync/changes`, REQ-SYN-003), **paginé** et **reprenable**. On vérifie
//! ici le versant API : le jeton `Authorization: Bearer` autorise le drain complet, et la pagination par
//! curseur reprend sans repartir de zéro. La reprenabilité côté client est couverte par les tests
//! unitaires `initialSync`.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::HashSet;
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
    auth: Option<(&str, &str)>,
    body: Option<Value>,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some((name, value)) = auth {
        builder = builder.header(name, value);
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

/// Crée un compte et renvoie le cookie de session.
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

/// Appaire un appareil et renvoie son jeton Bearer (REQ-AUT-005).
async fn pair_device(pool: &PgPool, email: &str) -> String {
    let r = send(
        pool,
        "POST",
        "/api/v1/device-sessions",
        None,
        Some(json!({ "email": email, "password": PASSWORD, "label": "Appareil de test", "platform": "desktop" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    body_json(r).await["token"].as_str().unwrap().to_string()
}

async fn create_payer(pool: &PgPool, cookie: &str, name: &str) -> String {
    let r = send(
        pool,
        "POST",
        "/api/v1/payers",
        Some((header::COOKIE.as_str(), cookie)),
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    body_json(r).await["id"].as_str().unwrap().to_string()
}

/// Draine tout le delta pour un appareil appairé (jeton Bearer), par pages de `page`, en suivant le
/// curseur — reprenable par construction. Renvoie l'ensemble des `entity_type:id` rencontrés.
async fn drain_with_token(pool: &PgPool, token: &str, page: u32) -> HashSet<String> {
    let bearer = format!("Bearer {token}");
    let mut seen = HashSet::new();
    let mut cursor: Option<String> = None;
    loop {
        let uri = match &cursor {
            Some(c) => format!("/api/v1/sync/changes?limit={page}&cursor={c}"),
            None => format!("/api/v1/sync/changes?limit={page}"),
        };
        let body = body_json(
            send(
                pool,
                "GET",
                &uri,
                Some((header::AUTHORIZATION.as_str(), &bearer)),
                None,
            )
            .await,
        )
        .await;
        for c in body["changes"].as_array().unwrap() {
            seen.insert(format!(
                "{}:{}",
                c["entity_type"].as_str().unwrap(),
                c["id"].as_str().unwrap()
            ));
        }
        if !body["has_more"].as_bool().unwrap() {
            break;
        }
        cursor = Some(body["next_cursor"].as_str().unwrap().to_string());
    }
    seen
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-008, case = "un appareil appairé récupère l'intégralité des données, paginée")]
async fn paired_device_retrieves_all_data(pool: PgPool) {
    let web = account(&pool, "syn008-full@example.com").await;
    // Trois payeurs, en plus des 16 catégories par défaut (REQ-CAT-002).
    let p1 = create_payer(&pool, &web, "Alex").await;
    let p2 = create_payer(&pool, &web, "Bob").await;
    let p3 = create_payer(&pool, &web, "Cleo").await;

    let token = pair_device(&pool, "syn008-full@example.com").await;
    // Drain paginé (2 par page) via le jeton d'appareil : couvre tout.
    let seen = drain_with_token(&pool, &token, 2).await;
    for id in [&p1, &p2, &p3] {
        assert!(
            seen.contains(&format!("payer:{id}")),
            "payeur {id} manquant"
        );
    }
    assert!(seen.iter().filter(|k| k.starts_with("category:")).count() >= 16);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-008, case = "reprise depuis un curseur : pas de re-fetch depuis l'origine")]
async fn initial_sync_resumes_from_cursor(pool: PgPool) {
    let web = account(&pool, "syn008-resume@example.com").await;
    create_payer(&pool, &web, "Alex").await;
    create_payer(&pool, &web, "Bob").await;
    let token = pair_device(&pool, "syn008-resume@example.com").await;
    let bearer = format!("Bearer {token}");

    // Première page (interruption simulée : on ne draine que la 1re page, on garde son curseur).
    let first = body_json(
        send(
            &pool,
            "GET",
            "/api/v1/sync/changes?limit=5",
            Some((header::AUTHORIZATION.as_str(), &bearer)),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(first["has_more"], true);
    let cursor = first["next_cursor"].as_str().unwrap().to_string();
    let first_ids: HashSet<String> = first["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();

    // Reprise depuis le curseur : la page suivante ne contient AUCUN élément déjà vu (pas de recouvrement).
    let next = body_json(
        send(
            &pool,
            "GET",
            &format!("/api/v1/sync/changes?limit=5&cursor={cursor}"),
            Some((header::AUTHORIZATION.as_str(), &bearer)),
            None,
        )
        .await,
    )
    .await;
    for c in next["changes"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap().to_string();
        assert!(!first_ids.contains(&id), "élément {id} rejoué à la reprise");
    }
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-008, case = "un jeton d'appareil invalide est refusé (401)")]
async fn invalid_device_token_is_unauthorized(pool: PgPool) {
    assert_eq!(
        send(
            &pool,
            "GET",
            "/api/v1/sync/changes",
            Some((header::AUTHORIZATION.as_str(), "Bearer nope")),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
