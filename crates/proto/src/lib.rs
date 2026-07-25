#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Types partagés et schémas API.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
