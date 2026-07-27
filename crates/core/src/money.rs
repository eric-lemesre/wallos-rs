//! Modèle monétaire.
//!
//! Tous les montants utilisent `rust_decimal::Decimal`. Aucun flottant n'est autorisé
//! dans ce module ni dans tout `core`.
//
// L'unique usage d'`unsafe` de ce crate est dans `CurrencyCode::as_str` et est
// justifié par l'ADR `docs/adr/0005-currency-code-as-str.md`.
#![allow(unsafe_code)]

use rust_decimal::Decimal;
use wallos_req_macros::requirement;

/// Représente un montant positif dans une devise donnée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    amount: Decimal,
    currency: CurrencyCode,
}

impl Money {
    /// Crée un montant à partir d'un `Decimal`.
    ///
    /// # Errors
    /// Retourne `DomainError::InvalidMoney` si le montant est négatif.
    #[requirement(REQ-CUR-002)]
    pub fn new(amount: Decimal, currency: CurrencyCode) -> Result<Self, crate::DomainError> {
        if amount.is_sign_negative() {
            return Err(crate::DomainError::InvalidMoney(format!(
                "negative amount: {amount}"
            )));
        }
        Ok(Self { amount, currency })
    }

    /// Montant nul dans une devise donnée (élément neutre d'une somme).
    #[must_use]
    #[requirement(REQ-CUR-002)]
    pub const fn zero(currency: CurrencyCode) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }

    /// Montant brut (toujours positif ou nul).
    #[must_use]
    #[requirement(REQ-CUR-002)]
    pub const fn amount(&self) -> Decimal {
        self.amount
    }

    /// Devise.
    #[must_use]
    #[requirement(REQ-CUR-002)]
    pub const fn currency(&self) -> CurrencyCode {
        self.currency
    }
}

/// Code ISO 4217 à trois lettres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CurrencyCode([u8; 3]);

impl CurrencyCode {
    /// Parse un code devise depuis une chaîne ASCII.
    ///
    /// # Errors
    /// Retourne `DomainError::InvalidMoney` si le format est incorrect.
    #[requirement(REQ-CUR-001)]
    pub fn new(code: &str) -> Result<Self, crate::DomainError> {
        let bytes = code.as_bytes();
        if bytes.len() != 3 || !bytes.iter().all(|b| b.is_ascii_alphabetic()) {
            return Err(crate::DomainError::InvalidMoney(format!(
                "invalid currency code: {code}"
            )));
        }
        let mut arr = [0u8; 3];
        arr.copy_from_slice(bytes);
        Ok(Self(arr))
    }

    /// Code sous forme de chaîne.
    ///
    /// # Safety
    /// Le constructeur garantit que les octets sont ASCII alphabétique, donc
    /// `from_utf8_unchecked` est sûr.
    #[must_use]
    #[requirement(REQ-CUR-001)]
    pub fn as_str(&self) -> &str {
        // SAFETY: les octets sont toujours ASCII alphabétique (vérifié par `new`).
        unsafe { std::str::from_utf8_unchecked(&self.0) }
    }
}

impl std::fmt::Display for CurrencyCode {
    #[requirement(REQ-CUR-001)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-CUR-002, case = "rejet montant négatif")]
    fn money_rejects_negative() {
        let amount = Decimal::from(-1);
        assert!(Money::new(amount, CurrencyCode::new("EUR").unwrap()).is_err());
    }

    #[test]
    #[verifies(REQ-CUR-002, case = "accepte zéro")]
    fn money_accepts_zero() {
        let amount = Decimal::ZERO;
        let m = Money::new(amount, CurrencyCode::new("EUR").unwrap()).unwrap();
        assert_eq!(m.amount(), Decimal::ZERO);
    }

    #[test]
    #[verifies(REQ-CUR-001, case = "as_str renvoie la chaîne initiale")]
    fn currency_code_as_str_round_trip() {
        let code = CurrencyCode::new("EUR").unwrap();
        assert_eq!(code.as_str(), "EUR");
    }

    #[test]
    #[verifies(REQ-CUR-001, case = "Display = code ISO")]
    fn currency_code_display_matches_code() {
        assert_eq!(format!("{}", CurrencyCode::new("USD").unwrap()), "USD");
    }

    #[test]
    #[verifies(REQ-CUR-002, case = "zéro est neutre")]
    fn money_zero_is_neutral() {
        let z = Money::zero(CurrencyCode::new("EUR").unwrap());
        assert_eq!(z.amount(), Decimal::ZERO);
        assert_eq!(z.currency(), CurrencyCode::new("EUR").unwrap());
    }
}
