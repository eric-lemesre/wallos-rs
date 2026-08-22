//! Service de l'interface web par le serveur (REQ-OPS-003).
//!
//! Un seul processus sert l'API et l'interface sur la même origine : pas de second serveur web,
//! pas de CORS, pas de cookies tiers. L'interface compilée (build Vite de `frontend/shells/web`)
//! est désignée par la variable [`WEBUI_DIR_VAR`] ; en son absence — variable non renseignée ou
//! répertoire sans `index.html` — l'API reste pleinement fonctionnelle et l'absence est signalée
//! au démarrage.
//!
//! Règles de service : les routes internes de l'interface se replient sur le document d'entrée
//! (routage côté client) ; une route d'API inexistante rend l'erreur applicative structurée,
//! jamais le document ; les actifs versionnés par empreinte (`/assets/…`) sont mis en cache
//! durablement, le document d'entrée jamais ; les en-têtes de sécurité (REQ-SEC-006) restent
//! posés par la couche commune, appliquée après coup dans `app_with_db_cron_key_webui`.

use std::convert::Infallible;
use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::response::Response;
use tower_http::services::ServeDir;
use wallos_core::requirement;

/// Nom de la variable d'environnement désignant le répertoire de l'interface compilée.
pub const WEBUI_DIR_VAR: &str = "WEBUI_DIR";

/// Interface web : servie depuis un répertoire, ou absente (API seule).
#[derive(Debug, Clone)]
pub enum WebUi {
    /// Interface compilée présente : servie depuis ce répertoire.
    Enabled(PathBuf),
    /// Interface absente : l'API reste seule servie, la raison est journalisée au démarrage.
    Disabled {
        /// Raison de l'absence, énoncée au démarrage.
        reason: String,
    },
}

/// Détecte l'interface compilée depuis la valeur brute de [`WEBUI_DIR_VAR`].
///
/// L'interface n'est servie que si le répertoire existe **et** contient `index.html` ; toute
/// autre situation est une absence signalée, jamais une erreur : l'API prime.
#[requirement(REQ-OPS-003)]
#[must_use]
pub fn detect(raw: Option<&str>) -> WebUi {
    match raw.filter(|v| !v.is_empty()) {
        None => WebUi::Disabled {
            reason: format!("{WEBUI_DIR_VAR} non renseignée : interface web non servie, API seule"),
        },
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if dir.join("index.html").is_file() {
                WebUi::Enabled(dir)
            } else {
                WebUi::Disabled {
                    reason: format!(
                        "{WEBUI_DIR_VAR} sans index.html : interface web non servie, API seule"
                    ),
                }
            }
        }
    }
}

/// Attache le service de l'interface au routeur d'API (repli), selon la détection.
///
/// Sans interface : le repli reste l'erreur structurée (aucun changement de comportement).
/// Avec interface : les fichiers du répertoire sont servis, une route inconnue hors `/api/` se
/// replie sur le document d'entrée, une route `/api/` inconnue garde l'erreur structurée.
#[requirement(REQ-OPS-003)]
pub fn attach(router: Router, ui: &WebUi) -> Router {
    match ui {
        WebUi::Disabled { .. } => router.fallback(crate::not_found),
        WebUi::Enabled(dir) => {
            let index = dir.join("index.html");
            let spa = tower::service_fn(move |request: Request<Body>| {
                let index = index.clone();
                let uri = request.uri().clone();
                async move { Ok::<_, Infallible>(spa_fallback(index, uri).await) }
            });
            let static_service = ServeDir::new(dir).fallback(spa);
            router
                .fallback_service(static_service)
                .layer(axum::middleware::from_fn(
                    |request: Request<Body>, next: axum::middleware::Next| async move {
                        let path = request.uri().path().to_string();
                        let response = next.run(request).await;
                        cache_policy(&path, response)
                    },
                ))
        }
    }
}

/// Repli du service statique : erreur structurée pour l'API, document d'entrée pour le reste.
async fn spa_fallback(index: PathBuf, uri: axum::http::Uri) -> Response {
    if uri.path().starts_with("/api/") {
        return crate::not_found(uri).await;
    }
    match tokio::fs::read(&index).await {
        Ok(bytes) => {
            let built = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(bytes));
            built.unwrap_or_else(|_| {
                let mut fallback = Response::new(Body::empty());
                *fallback.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                fallback
            })
        }
        Err(_) => crate::not_found(uri).await,
    }
}

/// Politique de cache : actifs empreinte durables, document d'entrée jamais mis en cache.
fn cache_policy(path: &str, mut response: Response) -> Response {
    if response.status() != StatusCode::OK {
        return response;
    }
    let is_asset = path.starts_with("/assets/");
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("text/html"));
    if is_asset {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    } else if is_html {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    response
}
