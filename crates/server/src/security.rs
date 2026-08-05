//! En-têtes de sécurité HTTP de la modalité web (REQ-SEC-006).
//!
//! Toute réponse émise par le serveur porte une **politique de sécurité de contenu** (CSP) et un jeu
//! d'en-têtes de durcissement conventionnels. La CSP **n'accorde aucune directive permissive de script
//! en ligne** (`script-src 'self'`, jamais `'unsafe-inline'`/`'unsafe-eval'`) — critère d'acceptation #1.
//!
//! Le second critère (capacités Tauri strictement nécessaires et justifiées) est **sans objet** : le
//! volet natif est hors périmètre (OQ-009, cf. ADR 0028/0032). SEC-006 est donc réduit au **CSP web**,
//! délivré ici en **en-tête HTTP** (source d'autorité, prime sur une balise `<meta>`) et répliqué dans
//! le document de production par le shell web (build Vite) pour la couche `ui`.

use axum::http::HeaderValue;
use axum::http::header::{
    CONTENT_SECURITY_POLICY, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::response::Response;
use wallos_core::requirement;

/// Politique de sécurité de contenu de la modalité web.
///
/// `script-src 'self'` **sans** `'unsafe-inline'`/`'unsafe-eval'` (critère #1). `style-src` autorise
/// `'unsafe-inline'` (attributs `style` en ligne des composants React) — le critère ne restreint que le
/// **script**. `frame-ancestors 'none'` interdit l'encadrement (anti-clickjacking, redondant avec
/// `X-Frame-Options`). `connect-src 'self'` : l'appli n'appelle que sa propre origine (`/api`).
pub const CONTENT_SECURITY_POLICY_VALUE: &str = "default-src 'self'; base-uri 'self'; \
     object-src 'none'; frame-ancestors 'none'; form-action 'self'; script-src 'self'; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'";

/// Ajoute les en-têtes de sécurité à **toute** réponse (middleware `map_response`).
///
/// Les valeurs sont statiques et toujours des en-têtes valides : `insert` (idempotent) écrase une
/// éventuelle valeur antérieure, sans jamais échouer.
#[requirement(REQ-SEC-006)]
pub async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY_VALUE),
    );
    // Empêche le MIME-sniffing (durcissement conventionnel).
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    // Anti-clickjacking (défense en profondeur avec `frame-ancestors 'none'`).
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    // Ne divulgue pas l'URL référente vers des tiers.
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
}
