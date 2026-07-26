#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used))]
//! Types partagés et schémas API.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wallos_core::requirement;

/// Réponse d'état du serveur.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    /// Nom du service.
    pub service: String,
    /// Version semver.
    pub version: String,
    /// État.
    pub status: String,
}

/// Requête de création de compte (REQ-AUT-001).
///
/// Le mot de passe transite en clair sur le canal TLS puis est haché argon2id côté serveur ;
/// il n'est jamais stocké en clair.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateAccountRequest {
    /// Adresse e-mail du compte.
    #[schema(example = "user@example.com", format = "email")]
    pub email: String,
    /// Mot de passe (longueur minimale vérifiée côté serveur, REQ-AUT-003).
    #[schema(
        example = "correct horse battery staple",
        min_length = 12,
        format = "password"
    )]
    pub password: String,
}

/// Requête d'authentification (REQ-AUT-002).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateSessionRequest {
    /// Adresse e-mail du compte.
    #[schema(example = "user@example.com", format = "email")]
    pub email: String,
    /// Mot de passe.
    #[schema(format = "password")]
    pub password: String,
}

/// Représentation du compte authentifié courant (REQ-AUT-002).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CurrentUser {
    /// Adresse e-mail du compte courant.
    pub email: String,
}

/// Détail d'erreur conforme à la RFC 9457 (`application/problem+json`).
///
/// Schéma d'erreur unique de l'API : toute réponse d'erreur adopte ce format,
/// sans jamais divulguer de trace d'exécution, de requête SQL ni de chemin de
/// fichier (AGENTS.md §6, REQ-SEC-002).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Problem {
    /// URI de référence identifiant le type de problème.
    #[serde(rename = "type")]
    #[schema(example = "about:blank")]
    pub type_uri: String,
    /// Résumé court et lisible du type de problème, stable pour un `type` donné.
    pub title: String,
    /// Code de statut HTTP généré par l'origine.
    pub status: u16,
    /// Explication spécifique à cette occurrence du problème.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// URI de référence identifiant l'occurrence spécifique du problème.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

/// Construit un `Problem` RFC 9457 à partir d'un statut, d'un type et d'un titre.
#[requirement(REQ-SEC-002)]
pub fn problem(status: u16, type_uri: impl Into<String>, title: impl Into<String>) -> Problem {
    Problem {
        type_uri: type_uri.into(),
        title: title.into(),
        status,
        detail: None,
        instance: None,
    }
}

impl Problem {
    /// Renseigne l'URI d'occurrence (`instance`).
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    /// Renseigne le détail spécifique à l'occurrence (`detail`).
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wallos_core::verifies;

    #[test]
    #[verifies(REQ-SEC-002)]
    fn problem_serialises_rfc9457_fields() {
        let value = serde_json::to_value(
            problem(404, "about:blank", "Not Found").with_instance("/api/v1/x"),
        )
        .unwrap();

        // Le champ RFC est bien `type`, pas `type_uri`.
        assert_eq!(value["type"], "about:blank");
        assert_eq!(value["title"], "Not Found");
        assert_eq!(value["status"], 404);
        assert_eq!(value["instance"], "/api/v1/x");
        // `detail` absent est omis de la sérialisation.
        assert!(value.get("detail").is_none());
    }

    #[test]
    #[verifies(REQ-SEC-002)]
    fn problem_roundtrips_with_detail() {
        let original = problem(500, "about:blank", "Internal Server Error")
            .with_detail("boom")
            .with_instance("/api/v1/y");
        let json = serde_json::to_string(&original).unwrap();
        let parsed: Problem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, 500);
        assert_eq!(parsed.detail.as_deref(), Some("boom"));
    }
}
