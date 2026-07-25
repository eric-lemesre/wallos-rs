#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! SDK HTTP Rust pour wallos-rs.

#![forbid(unsafe_code)]

/// Client API wallos.
#[derive(Debug, Clone)]
pub struct WallosClient {
    base_url: String,
}

impl WallosClient {
    /// Crée un client.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    /// URL de base.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}
