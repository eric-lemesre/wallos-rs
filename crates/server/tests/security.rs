//! Tests d'intégration des en-têtes de sécurité HTTP (REQ-SEC-006).
//!
//! Toute réponse de la modalité web porte une CSP **sans directive permissive de script en ligne**
//! (critère #1), ainsi que les en-têtes de durcissement conventionnels. Vérifié sur une route servie
//! **et** sur le repli 404 (le middleware s'applique à toutes les réponses).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use wallos_req_macros::verifies;
use wallos_server::app;

async fn get(uri: &str) -> axum::http::Response<Body> {
    app()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn header<'a>(res: &'a axum::http::Response<Body>, name: &str) -> &'a str {
    res.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

#[tokio::test]
#[verifies(REQ-SEC-006, case = "CSP présente, script-src strict (aucune directive de script en ligne)")]
async fn response_carries_a_content_security_policy_without_inline_script() {
    let res = get("/api/v1/health").await;
    assert_eq!(res.status(), StatusCode::OK);
    let csp = header(&res, "content-security-policy");
    assert!(!csp.is_empty(), "CSP absente");
    // Le script est strict : `'self'`, jamais de directive en ligne/eval permissive (critère #1).
    assert!(
        csp.contains("script-src 'self'"),
        "script-src manquant: {csp}"
    );
    assert!(
        !csp.contains("script-src 'self' 'unsafe-inline'"),
        "script-src permissif (unsafe-inline): {csp}"
    );
    assert!(!csp.contains("unsafe-eval"), "unsafe-eval présent: {csp}");
    // Défense en profondeur : pas d'encadrement, pas de MIME-sniffing.
    assert!(
        csp.contains("frame-ancestors 'none'"),
        "frame-ancestors manquant: {csp}"
    );
}

#[tokio::test]
#[verifies(REQ-SEC-006, case = "en-têtes de durcissement conventionnels")]
async fn response_carries_hardening_headers() {
    let res = get("/api/v1/health").await;
    assert_eq!(header(&res, "x-content-type-options"), "nosniff");
    assert_eq!(header(&res, "x-frame-options"), "DENY");
    assert_eq!(header(&res, "referrer-policy"), "no-referrer");
}

#[tokio::test]
#[verifies(REQ-SEC-006, case = "en-têtes appliqués aussi au repli 404")]
async fn security_headers_apply_to_the_not_found_fallback() {
    let res = get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    // Le middleware couvre toutes les réponses, y compris les erreurs.
    assert!(!header(&res, "content-security-policy").is_empty());
    assert_eq!(header(&res, "x-content-type-options"), "nosniff");
}
