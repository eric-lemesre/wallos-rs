#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Couche d'envoi des notifications.

#![forbid(unsafe_code)]

/// Résumé d'un canal de notification.
#[derive(Debug, Clone)]
pub struct NotificationChannel {
    pub name: String,
}

impl NotificationChannel {
    /// Crée un canal.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
