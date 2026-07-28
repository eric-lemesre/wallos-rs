//! Devise de référence d'un foyer (REQ-CUR-001).
//!
//! La devise de référence est **la devise dans laquelle tous les agrégats sont exprimés**. Elle
//! n'altère **jamais** les montants saisis : chaque abonnement conserve son montant et sa devise
//! d'origine (acceptance #2) ; la référence ne sert qu'à présenter les totaux convertis (REQ-CUR-004).
//! Modifier la devise de référence recalcule les agrégats dans la nouvelle devise (acceptance #1),
//! sans toucher aux données saisies.

use wallos_req_macros::requirement;

use crate::DomainError;
use crate::money::CurrencyCode;

/// Devise de référence d'un foyer : la devise cible de tous les agrégats.
///
/// Newtype sur [`CurrencyCode`] pour distinguer, au niveau du type, « la devise de référence du foyer »
/// d'« une devise d'un montant saisi » — on ne peut pas confondre l'une avec l'autre par erreur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceCurrency(CurrencyCode);

impl ReferenceCurrency {
    /// Code par défaut, appliqué tant que le foyer n'a rien choisi (aligné sur la valeur par défaut
    /// de la base). L'euro est un choix neutre, aligné sur l'application d'origine.
    pub const DEFAULT_CODE: &'static str = "EUR";

    /// Construit une devise de référence à partir d'un code, validé contre le référentiel supporté.
    ///
    /// # Errors
    /// [`DomainError`] si le code n'est pas une devise supportée (majuscules, ISO 4217 du référentiel).
    #[requirement(REQ-CUR-001)]
    pub fn parse(code: &str) -> Result<Self, DomainError> {
        Ok(Self(CurrencyCode::new(code)?))
    }

    /// Enveloppe une devise déjà validée.
    #[must_use]
    #[requirement(REQ-CUR-001)]
    pub const fn new(code: CurrencyCode) -> Self {
        Self(code)
    }

    /// Code devise cible des agrégats.
    #[must_use]
    #[requirement(REQ-CUR-001)]
    pub const fn code(&self) -> CurrencyCode {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-CUR-001, case = "devise de référence validée + accesseur")]
    fn parses_and_exposes_code() {
        let rc = ReferenceCurrency::parse("USD").unwrap();
        assert_eq!(rc.code(), CurrencyCode::new("USD").unwrap());
        // Le code par défaut est lui-même une devise supportée.
        assert!(ReferenceCurrency::parse(ReferenceCurrency::DEFAULT_CODE).is_ok());
    }

    #[test]
    #[verifies(REQ-CUR-001, case = "devise hors référentiel refusée")]
    fn rejects_unsupported_currency() {
        assert!(ReferenceCurrency::parse("ZZZ").is_err());
        assert!(ReferenceCurrency::parse("eur").is_err()); // minuscules refusées
    }
}
