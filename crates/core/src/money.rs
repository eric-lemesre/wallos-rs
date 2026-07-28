//! Modèle monétaire.
//!
//! Tous les montants utilisent `rust_decimal::Decimal`. Aucun flottant n'est autorisé
//! dans ce module ni dans tout `core`.
//
// L'unique usage d'`unsafe` de ce crate est dans `CurrencyCode::as_str` et est
// justifié par l'ADR `docs/adr/0005-currency-code-as-str.md`.
#![allow(unsafe_code)]

use rust_decimal::{Decimal, RoundingStrategy};
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

    /// Arrondit le montant à `decimals` décimales, en **arrondi bancaire** (REQ-CUR-005).
    ///
    /// L'arrondi n'intervient qu'à cette étape d'affichage : la conversion et l'agrégation
    /// conservent la précision maximale (REQ-CUR-004), jamais arrondie en cours de calcul. La règle
    /// est le *round half to even* (les demis vont vers le chiffre pair, ce qui évite le biais
    /// systématique du *half up*). Le nombre de décimales **dépend de la devise** : il est fourni par
    /// l'appelant depuis le référentiel (REQ-CUR-007), p. ex. 2 pour EUR/USD, 0 pour JPY.
    #[must_use]
    #[requirement(REQ-CUR-005)]
    pub fn round_to(&self, decimals: u32) -> Self {
        Self {
            amount: self
                .amount
                .round_dp_with_strategy(decimals, RoundingStrategy::MidpointNearestEven),
            currency: self.currency,
        }
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

    #[test]
    #[verifies(REQ-CUR-005, case = "arrondi bancaire (half to even)")]
    fn rounds_half_to_even() {
        // Vecteurs figés : e2e/fixtures/oracles/REQ-CUR-005-rounding.json.
        let eur = CurrencyCode::new("EUR").unwrap();
        let round = |v: &str, dp: u32| {
            Money::new(v.parse::<Decimal>().unwrap(), eur)
                .unwrap()
                .round_to(dp)
                .amount()
        };
        // Les demis vont vers le chiffre pair (jamais un biais systématique « half up »).
        assert_eq!(round("0.125", 2), "0.12".parse::<Decimal>().unwrap());
        assert_eq!(round("0.135", 2), "0.14".parse::<Decimal>().unwrap());
        assert_eq!(round("0.5", 0), Decimal::ZERO);
        assert_eq!(round("1.5", 0), "2".parse::<Decimal>().unwrap());
        assert_eq!(round("2.5", 0), "2".parse::<Decimal>().unwrap());
        assert_eq!(round("3.5", 0), "4".parse::<Decimal>().unwrap());
        // Hors demi : arrondi au plus proche.
        assert_eq!(round("9.994", 2), "9.99".parse::<Decimal>().unwrap());
        assert_eq!(round("9.996", 2), "10.00".parse::<Decimal>().unwrap());
    }

    #[test]
    #[verifies(REQ-CUR-005, case = "décimales selon la devise ; original inchangé")]
    fn rounding_depends_on_decimals_and_preserves_source() {
        let source = Money::new(
            "1234.5678".parse::<Decimal>().unwrap(),
            CurrencyCode::new("EUR").unwrap(),
        )
        .unwrap();
        // 2 décimales (EUR) vs 0 décimale (JPY) : le nombre de décimales est fourni par l'appelant.
        assert_eq!(
            source.round_to(2).amount(),
            "1234.57".parse::<Decimal>().unwrap()
        );
        assert_eq!(
            source.round_to(0).amount(),
            "1235".parse::<Decimal>().unwrap()
        );
        // L'arrondi ne mute pas le montant d'origine (précision maximale conservée jusqu'à l'affichage).
        assert_eq!(source.amount(), "1234.5678".parse::<Decimal>().unwrap());
        // La devise est préservée.
        assert_eq!(
            source.round_to(2).currency(),
            CurrencyCode::new("EUR").unwrap()
        );
    }

    #[test]
    #[verifies(REQ-CUR-002, case = "somme exacte à deux décimales")]
    fn two_decimal_sum_is_exact() {
        // Le piège flottant classique : 0.10 + 0.20 vaut 0.30 EXACTEMENT en décimal (un flottant
        // binaire donnerait 0.30000000000000004). C'est la garantie même de REQ-CUR-002.
        let a: Decimal = "0.10".parse().unwrap();
        let b: Decimal = "0.20".parse().unwrap();
        assert_eq!(a + b, "0.30".parse::<Decimal>().unwrap());

        // Somme réaliste de montants à deux décimales : le total est exact, sans erreur de représentation.
        let eur = CurrencyCode::new("EUR").unwrap();
        let m1 = Money::new("19.99".parse().unwrap(), eur).unwrap();
        let m2 = Money::new("0.01".parse().unwrap(), eur).unwrap();
        let total = m1.amount() + m2.amount();
        assert_eq!(total, "20.00".parse::<Decimal>().unwrap());
    }
}
