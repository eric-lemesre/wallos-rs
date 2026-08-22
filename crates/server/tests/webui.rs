//! Tests d'intégration du service de l'interface web par le serveur (REQ-OPS-003).
//!
//! L'interface compilée est simulée par un répertoire temporaire (index.html + un actif
//! empreinte sous assets/). La base n'est jamais contactée : pool paresseux.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;
use wallos_req_macros::verifies;
use wallos_server::webui::{self, WebUi};
use wallos_server::{CronToken, EncryptionKey, app_with_db_cron_key_webui};
use wallos_storage::Db;

fn lazy_db() -> Db {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .expect("pool paresseux");
    Db::from_pool(pool)
}

/// Pose une interface compilée factice sur disque et rend son répertoire.
fn fake_dist(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wallos-webui-{}-{tag}", std::process::id()));
    let assets = dir.join("assets");
    std::fs::create_dir_all(&assets).expect("mkdir");
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html><title>wallos-ui</title>",
    )
    .expect("index");
    std::fs::write(assets.join("app-abc123.js"), "console.log('ui')").expect("asset");
    dir
}

fn app(ui: &WebUi) -> Router {
    app_with_db_cron_key_webui(lazy_db(), CronToken(None), EncryptionKey(None), ui)
}

async fn get(router: Router, uri: &str) -> axum::http::Response<Body> {
    router
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("requête"),
        )
        .await
        .expect("réponse")
}

#[tokio::test]
#[verifies(REQ-OPS-003, case = "la racine sert le document d'entrée")]
async fn root_serves_index() {
    let ui = webui::detect(Some(fake_dist("root").to_str().expect("utf8")));
    let response = get(app(&ui), "/").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("corps");
    assert!(String::from_utf8_lossy(&body).contains("wallos-ui"));
}

#[tokio::test]
#[verifies(REQ-OPS-003, case = "une route interne de l'interface se replie sur le document d'entrée")]
async fn spa_route_falls_back_to_index() {
    let ui = webui::detect(Some(fake_dist("spa").to_str().expect("utf8")));
    let response = get(app(&ui), "/abonnements/42").await;
    assert_eq!(response.status(), StatusCode::OK);
    let no_cache = response
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        no_cache.contains("no-cache"),
        "le document d'entrée n'est pas mis en cache : {no_cache:?}"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("corps");
    assert!(String::from_utf8_lossy(&body).contains("wallos-ui"));
}

#[tokio::test]
#[verifies(REQ-OPS-003, case = "une route d'API inexistante rend une erreur structurée, jamais l'interface")]
async fn unknown_api_route_is_problem_not_html() {
    let ui = webui::detect(Some(fake_dist("api404").to_str().expect("utf8")));
    let response = get(app(&ui), "/api/v1/inexistant").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        content_type.contains("application/problem+json"),
        "erreur applicative structurée attendue : {content_type:?}"
    );
}

#[tokio::test]
#[verifies(REQ-OPS-003, case = "actif empreinte : cache durable et en-têtes de sécurité conservés")]
async fn hashed_asset_is_cached_and_keeps_security_headers() {
    let ui = webui::detect(Some(fake_dist("asset").to_str().expect("utf8")));
    let response = get(app(&ui), "/assets/app-abc123.js").await;
    assert_eq!(response.status(), StatusCode::OK);
    let cache = response
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        cache.contains("immutable"),
        "cache durable attendu : {cache:?}"
    );
    assert!(
        response
            .headers()
            .contains_key(header::CONTENT_SECURITY_POLICY),
        "les en-têtes de sécurité restent appliqués aux actifs"
    );
}

#[tokio::test]
#[verifies(REQ-OPS-003, case = "sans interface compilée : API pleinement fonctionnelle, absence signalée")]
async fn absent_ui_keeps_api_functional() {
    let ui = webui::detect(None);
    match &ui {
        WebUi::Disabled { reason } => {
            assert!(reason.contains("API seule"), "absence signalée : {reason}");
        }
        WebUi::Enabled(_) => panic!("sans WEBUI_DIR, l'interface est désactivée"),
    }
    let router = app(&ui);
    let health = get(router.clone(), "/api/v1/health").await;
    assert_eq!(health.status(), StatusCode::OK);
    let root = get(router, "/").await;
    assert_eq!(root.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[verifies(REQ-OPS-003, case = "répertoire sans document d'entrée : interface désactivée, raison énoncée")]
async fn dir_without_index_disables_ui() {
    let dir = std::env::temp_dir().join(format!("wallos-webui-{}-vide", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ui = webui::detect(Some(dir.to_str().expect("utf8")));
    match ui {
        WebUi::Disabled { reason } => assert!(reason.contains("index.html"), "raison : {reason}"),
        WebUi::Enabled(_) => panic!("sans index.html, l'interface est désactivée"),
    }
}
