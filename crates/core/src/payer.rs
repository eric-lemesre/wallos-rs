//! Modèle de payeur (REQ-SUB-017).
//!
//! Un payeur est une **étiquette nominative** du foyer (parité avec la table `household` de Wallos,
//! oracle REQ-SUB-017-payer.json) : aucun login, aucun compte. Dimension d'analyse et de filtre — un
//! abonnement référence **au plus un** payeur, optionnel (`Subscription::payer`, un `Option<Uuid>`).
//! Entité **possédée** : l'isolation par foyer (§9) est portée par le repository (`&Actor`) ; le modèle
//! pur ne porte que l'identité métier (id + nom). Le nom est obligatoire (non vide) et borné.

use uuid::Uuid;
use wallos_req_macros::requirement;

use crate::DomainError;

/// Un payeur (personne du foyer à qui une dépense est rattachée).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payer {
    id: Uuid,
    name: String,
}

/// Longueur maximale d'un nom de payeur (caractères Unicode).
pub const NAME_MAX_LEN: usize = 100;

impl Payer {
    /// Construit un payeur. Le nom est **normalisé** (espaces de bord retirés) ; il ne doit pas être
    /// vide, ni dépasser [`NAME_MAX_LEN`] caractères.
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si le nom (après normalisation) est vide ou trop long.
    #[requirement(REQ-SUB-017)]
    pub fn new(id: Uuid, name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err(DomainError::InvalidArgument(
                "payer name must not be empty".to_string(),
            ));
        }
        if name.chars().count() > NAME_MAX_LEN {
            return Err(DomainError::InvalidArgument(format!(
                "payer name exceeds {NAME_MAX_LEN} characters"
            )));
        }
        Ok(Self { id, name })
    }

    /// Identifiant stable.
    #[must_use]
    #[requirement(REQ-SUB-017)]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// Nom du payeur.
    #[must_use]
    #[requirement(REQ-SUB-017)]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-SUB-017, case = "nom obligatoire + accesseurs")]
    fn builds_and_exposes_fields() {
        let id = Uuid::from_u128(1);
        let payer = Payer::new(id, "Alex").unwrap();
        assert_eq!(payer.id(), id);
        assert_eq!(payer.name(), "Alex");
    }

    #[test]
    #[verifies(REQ-SYN-001, case = "l'identifiant (UUID) fourni est conservé tel quel")]
    fn identity_uuid_is_preserved() {
        let client_id = Uuid::from_u128(0xdead_beef);
        let payer = Payer::new(client_id, "Sam").unwrap();
        assert_eq!(payer.id(), client_id);
    }

    #[test]
    #[verifies(REQ-SUB-017, case = "nom vide refusé")]
    fn empty_name_is_rejected() {
        assert!(Payer::new(Uuid::from_u128(1), "").is_err());
        assert!(Payer::new(Uuid::from_u128(1), "   ").is_err());
    }

    #[test]
    #[verifies(REQ-SUB-017, case = "nom normalisé (trim) et borné")]
    fn name_is_trimmed_and_length_bounded() {
        let payer = Payer::new(Uuid::from_u128(1), "  Alex  ").unwrap();
        assert_eq!(payer.name(), "Alex");
        assert!(Payer::new(Uuid::from_u128(1), "a".repeat(NAME_MAX_LEN)).is_ok());
        assert!(Payer::new(Uuid::from_u128(1), "a".repeat(NAME_MAX_LEN + 1)).is_err());
    }
}
