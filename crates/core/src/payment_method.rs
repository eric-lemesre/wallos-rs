//! Modèle de moyen de paiement (REQ-SUB-011).
//!
//! Dimension d'analyse et de filtre : un abonnement référence **au plus un** moyen de paiement,
//! optionnel (`Subscription::payment_method`, un `Option<Uuid>`). Entité **possédée** : l'isolation
//! par foyer (§9) est portée par le repository (`&Actor`) ; le modèle pur ne porte que l'identité
//! métier (id + nom). Le nom est obligatoire (non vide) et borné, comme pour les catégories.

use uuid::Uuid;
use wallos_req_macros::requirement;

use crate::DomainError;

/// Un moyen de paiement (carte, prélèvement, portefeuille…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentMethod {
    id: Uuid,
    name: String,
}

/// Longueur maximale d'un nom de moyen de paiement (caractères Unicode).
pub const NAME_MAX_LEN: usize = 100;

impl PaymentMethod {
    /// Construit un moyen de paiement. Le nom est **normalisé** (espaces de bord retirés) ; il ne doit
    /// pas être vide, ni dépasser [`NAME_MAX_LEN`] caractères.
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si le nom (après normalisation) est vide ou trop long.
    #[requirement(REQ-SUB-011)]
    pub fn new(id: Uuid, name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err(DomainError::InvalidArgument(
                "payment method name must not be empty".to_string(),
            ));
        }
        if name.chars().count() > NAME_MAX_LEN {
            return Err(DomainError::InvalidArgument(format!(
                "payment method name exceeds {NAME_MAX_LEN} characters"
            )));
        }
        Ok(Self { id, name })
    }

    /// Identifiant stable.
    #[must_use]
    #[requirement(REQ-SUB-011)]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// Nom du moyen de paiement.
    #[must_use]
    #[requirement(REQ-SUB-011)]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-SUB-011, case = "nom obligatoire + accesseurs")]
    fn builds_and_exposes_fields() {
        let id = Uuid::from_u128(1);
        let pm = PaymentMethod::new(id, "Carte de crédit").unwrap();
        assert_eq!(pm.id(), id);
        assert_eq!(pm.name(), "Carte de crédit");
    }

    #[test]
    #[verifies(REQ-SUB-011, case = "nom vide refusé")]
    fn empty_name_is_rejected() {
        assert!(PaymentMethod::new(Uuid::from_u128(1), "").is_err());
        assert!(PaymentMethod::new(Uuid::from_u128(1), "   ").is_err());
    }

    #[test]
    #[verifies(REQ-SUB-011, case = "nom normalisé (trim) et borné")]
    fn name_is_trimmed_and_length_bounded() {
        let pm = PaymentMethod::new(Uuid::from_u128(1), "  PayPal  ").unwrap();
        assert_eq!(pm.name(), "PayPal");
        assert!(PaymentMethod::new(Uuid::from_u128(1), "a".repeat(NAME_MAX_LEN)).is_ok());
        assert!(PaymentMethod::new(Uuid::from_u128(1), "a".repeat(NAME_MAX_LEN + 1)).is_err());
    }
}
