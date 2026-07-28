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

impl Category {
    /// Construit une catégorie. Le nom ne doit pas être vide (ni uniquement des espaces).
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si le nom est vide.
    #[requirement(REQ-CAT-001)]
    pub fn new(id: Uuid, name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(DomainError::InvalidArgument(
                "category name must not be empty".to_string(),
            ));
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
}
