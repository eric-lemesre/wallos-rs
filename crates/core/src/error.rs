//! Erreurs du domaine.

/// Erreur métier de haut niveau.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("valeur monétaire invalide: {0}")]
    InvalidMoney(String),
    #[error("date invalide: {0}")]
    InvalidDate(String),
    #[error("argument invalide: {0}")]
    InvalidArgument(String),
}
