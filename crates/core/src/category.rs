//! Modèle de catégorie (REQ-CAT-001).
//!
//! Dimension d'analyse principale des statistiques. Entité **possédée** : l'isolation par foyer
//! (§9) est portée par le repository (`&Actor`), comme les autres entités ; le modèle pur ne porte
//! que l'identité métier (id + nom). Le nom est obligatoire (non vide).

use uuid::Uuid;
use wallos_req_macros::requirement;

use crate::DomainError;

/// Une catégorie d'abonnements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    id: Uuid,
    name: String,
}

/// Longueur maximale d'un nom de catégorie (caractères Unicode).
pub const NAME_MAX_LEN: usize = 100;

impl Category {
    /// Construit une catégorie. Le nom est **normalisé** (espaces de bord retirés) ; il ne doit pas
    /// être vide, ni dépasser [`NAME_MAX_LEN`] caractères.
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si le nom (après normalisation) est vide ou trop long.
    #[requirement(REQ-CAT-001)]
    pub fn new(id: Uuid, name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err(DomainError::InvalidArgument(
                "category name must not be empty".to_string(),
            ));
        }
        if name.chars().count() > NAME_MAX_LEN {
            return Err(DomainError::InvalidArgument(format!(
                "category name exceeds {NAME_MAX_LEN} characters"
            )));
        }
        Ok(Self { id, name })
    }

    /// Identifiant stable.
    #[must_use]
    #[requirement(REQ-CAT-001)]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// Nom de la catégorie.
    #[must_use]
    #[requirement(REQ-CAT-001)]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-CAT-001, case = "nom obligatoire + accesseurs")]
    fn builds_and_exposes_fields() {
        let id = Uuid::from_u128(1);
        let c = Category::new(id, "Streaming").unwrap();
        assert_eq!(c.id(), id);
        assert_eq!(c.name(), "Streaming");
    }

    #[test]
    #[verifies(REQ-CAT-001, case = "nom vide refusé")]
    fn empty_name_is_rejected() {
        assert!(Category::new(Uuid::from_u128(1), "").is_err());
        assert!(Category::new(Uuid::from_u128(1), "   ").is_err());
    }

    #[test]
    #[verifies(REQ-CAT-001, case = "nom normalisé (trim) et borné")]
    fn name_is_trimmed_and_length_bounded() {
        // Les espaces de bord sont retirés avant stockage (pas de "  X  " en base).
        let c = Category::new(Uuid::from_u128(1), "  Streaming  ").unwrap();
        assert_eq!(c.name(), "Streaming");
        // Un nom à la longueur maximale passe ; au-delà, il est refusé.
        assert!(Category::new(Uuid::from_u128(1), "a".repeat(NAME_MAX_LEN)).is_ok());
        assert!(Category::new(Uuid::from_u128(1), "a".repeat(NAME_MAX_LEN + 1)).is_err());
    }
}
