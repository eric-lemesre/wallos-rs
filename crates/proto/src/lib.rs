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

/// Requête d'appairage d'un appareil natif (REQ-AUT-005).
///
/// Comme l'authentification web, mais émet un jeton propre à l'appareil (corps de réponse) au lieu
/// d'un cookie ; l'appareil fournit un libellé et sa plateforme.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateDeviceSessionRequest {
    /// Adresse e-mail du compte.
    #[schema(example = "user@example.com", format = "email")]
    pub email: String,
    /// Mot de passe.
    #[schema(format = "password")]
    pub password: String,
    /// Libellé lisible de l'appareil (choisi par l'utilisateur ou dérivé du matériel).
    #[schema(example = "MacBook de Léa")]
    pub label: String,
    /// Plateforme de l'appareil.
    #[schema(example = "desktop")]
    pub platform: String,
}

/// Jeton d'appareil émis à l'appairage (REQ-AUT-005).
///
/// Renvoyé **une seule fois** : la coquille native le stocke via `PlatformAdapter.secureStore`,
/// jamais en clair côté serveur (seule son empreinte SHA-256 est conservée).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct DeviceToken {
    /// Jeton opaque à présenter en `Authorization: Bearer`.
    pub token: String,
}

/// Requête de changement de mot de passe (REQ-AUT-007).
///
/// Exige le mot de passe actuel (sinon `403`) ; le nouveau doit respecter la politique (REQ-AUT-003).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ChangePasswordRequest {
    /// Mot de passe actuel, revérifié côté serveur.
    #[schema(format = "password")]
    pub current_password: String,
    /// Nouveau mot de passe (longueur minimale + non compromis, REQ-AUT-003).
    #[schema(format = "password", min_length = 12)]
    pub new_password: String,
}

/// Résumé d'un appareil appairé, pour la liste de gestion (REQ-AUT-006).
///
/// `id` (UUID) et `last_seen_at` (RFC 3339) sont sérialisés en chaînes pour rester indépendants des
/// features `utoipa`/`chrono` ; `current` distingue l'appareil à l'origine de la requête courante.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct DeviceSummary {
    /// Identifiant de l'appareil (UUID), clé de révocation.
    pub id: String,
    /// Libellé lisible.
    pub label: String,
    /// Plateforme de l'appareil.
    pub platform: String,
    /// Dernière activité (horodatage RFC 3339).
    pub last_seen_at: String,
    /// Vrai si cet appareil est celui qui a émis la requête courante.
    pub current: bool,
}

/// Un montant en devise pour l'agrégation multi-devises (REQ-CUR-004).
///
/// `amount` est une **chaîne décimale** (règle R4 / REQ-CUR-002) : jamais un nombre JSON, qui
/// introduirait une imprécision flottante sur un montant.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct MoneyInput {
    /// Montant en chaîne décimale (ex. `"12.34"`).
    #[schema(example = "12.34")]
    pub amount: String,
    /// Code devise ISO 4217 (ex. `"EUR"`).
    #[schema(example = "EUR")]
    pub currency: String,
}

/// Requête d'agrégation multi-devises vers une devise cible (REQ-CUR-004).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AggregateRequest {
    /// Devise cible de l'agrégat (code ISO 4217).
    #[schema(example = "EUR")]
    pub target: String,
    /// Montants à convertir puis sommer.
    pub amounts: Vec<MoneyInput>,
}

/// Résultat d'une agrégation convertie, en **mode dégradé** explicite (REQ-CUR-004).
///
/// `total` est une chaîne décimale (R4). `complete` est faux dès qu'un montant a été **exclu** faute
/// de taux : un total incomplet n'est jamais présenté comme un zéro silencieux. `as_of` porte la
/// date de validité **la plus ancienne** parmi les taux utilisés — la fraîcheur à afficher quand le
/// fournisseur est indisponible et que l'on retombe sur le dernier taux connu.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ConvertedTotalResponse {
    /// Total converti dans la devise cible (chaîne décimale, précision exacte).
    #[schema(example = "142.50")]
    pub total: String,
    /// Devise du total (code ISO 4217).
    #[schema(example = "EUR")]
    pub currency: String,
    /// Nombre de montants effectivement convertis et inclus.
    pub converted: u32,
    /// Nombre de montants exclus faute de taux connu.
    pub excluded: u32,
    /// Vrai si tous les montants ont pu être convertis (agrégat complet).
    pub complete: bool,
    /// Date de validité la plus ancienne des taux utilisés (`YYYY-MM-DD`), ou absente si aucun taux
    /// daté n'a servi (ensemble vide ou conversions en devise identique uniquement).
    #[schema(example = "2026-07-20")]
    pub as_of: Option<String>,
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

    #[test]
    #[verifies(REQ-CUR-002, case = "montant sérialisé en chaîne")]
    fn amounts_serialise_as_json_strings() {
        // Un montant en entrée transite en CHAÎNE décimale, jamais en nombre JSON.
        let input = serde_json::to_value(MoneyInput {
            amount: "12.34".to_string(),
            currency: "EUR".to_string(),
        })
        .unwrap();
        assert!(input["amount"].is_string());
        assert_eq!(input["amount"], "12.34");

        // Idem pour un total en sortie.
        let output = serde_json::to_value(ConvertedTotalResponse {
            total: "142.50".to_string(),
            currency: "EUR".to_string(),
            converted: 3,
            excluded: 0,
            complete: true,
            as_of: Some("2026-07-20".to_string()),
        })
        .unwrap();
        assert!(output["total"].is_string());
        assert_eq!(output["total"], "142.50");
    }

    #[test]
    #[verifies(REQ-CUR-002, case = "un nombre JSON est refusé")]
    fn amount_as_json_number_is_rejected() {
        // Un nombre JSON pour un montant est refusé au parsing (le champ est une chaîne) : un montant
        // ne peut jamais voyager en nombre flottant, même côté désérialisation.
        let from_number: Result<MoneyInput, _> =
            serde_json::from_value(serde_json::json!({ "amount": 12.34, "currency": "EUR" }));
        assert!(from_number.is_err());

        // La forme correcte (chaîne) se relit sans perte.
        let from_string: MoneyInput =
            serde_json::from_value(serde_json::json!({ "amount": "12.34", "currency": "EUR" }))
                .unwrap();
        assert_eq!(from_string.amount, "12.34");
    }
}
