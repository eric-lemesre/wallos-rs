//! Agrégats statistiques déterministes.
//!
//! REQ-STA-008 — « Détermination des agrégats » : un agrégat est une **fonction pure**
//! d'un jeu de données et d'une **date de référence fournie explicitement**. Aucun accès
//! à l'horloge système n'est autorisé dans ce module (porte `cargo xtask lint-clock`).
//!
//! Ce module pose le socle de reproductibilité que les exigences `REQ-STA-*` ultérieures
//! (normalisation mensuelle, répartitions, échéancier) consommeront. Il reste volontairement
//! générique et ne préempte aucune sémantique métier `oracle: legacy`.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use wallos_req_macros::requirement;

use crate::DomainError;
use crate::money::{CurrencyCode, Money};

/// Date de référence explicite d'un calcul d'agrégat.
///
/// Ce newtype rend structurellement impossible qu'un agrégat de `core` déduise la date
/// de l'horloge : elle est toujours fournie par l'appelant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsOf(NaiveDate);

impl AsOf {
    /// Construit une date de référence à partir d'une date civile.
    #[must_use]
    #[requirement(REQ-STA-008)]
    pub const fn new(date: NaiveDate) -> Self {
        Self(date)
    }

    /// Date civile sous-jacente.
    #[must_use]
    #[requirement(REQ-STA-008)]
    pub const fn date(self) -> NaiveDate {
        self.0
    }
}

/// Agrégat déterministe d'un ensemble de montants d'une devise donnée.
///
/// L'agrégat est estampillé par la date de référence pour laquelle il a été calculé,
/// ce qui garantit sa reproductibilité et sa traçabilité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aggregate {
    as_of: NaiveDate,
    currency: CurrencyCode,
    total: Decimal,
    count: usize,
}

impl Aggregate {
    /// Calcule l'agrégat d'un ensemble de montants exprimés dans la devise `base`.
    ///
    /// Fonction **pure** : le résultat ne dépend que de `as_of`, `base` et `amounts`.
    /// Un ensemble vide produit un agrégat neutre (total nul, comptage nul) dans `base`.
    ///
    /// # Errors
    /// Retourne `DomainError::InvalidArgument` si un montant n'est pas exprimé dans `base`
    /// (aucune conversion implicite : l'agrégation multi-devises relève de `REQ-CUR-003`).
    #[requirement(REQ-STA-008)]
    pub fn compute(
        as_of: AsOf,
        base: CurrencyCode,
        amounts: &[Money],
    ) -> Result<Self, DomainError> {
        let mut total = Decimal::ZERO;
        for m in amounts {
            if m.currency() != base {
                return Err(DomainError::InvalidArgument(format!(
                    "amount in {} does not match base currency {base}",
                    m.currency()
                )));
            }
            total += m.amount();
        }
        Ok(Self {
            as_of: as_of.date(),
            currency: base,
            total,
            count: amounts.len(),
        })
    }

    /// Date de référence de l'agrégat.
    #[must_use]
    #[requirement(REQ-STA-008)]
    pub const fn as_of(&self) -> NaiveDate {
        self.as_of
    }

    /// Devise dans laquelle l'agrégat est exprimé.
    #[must_use]
    #[requirement(REQ-STA-008)]
    pub const fn currency(&self) -> CurrencyCode {
        self.currency
    }

    /// Total agrégé, en précision décimale exacte.
    #[must_use]
    #[requirement(REQ-STA-008)]
    pub const fn total(&self) -> Decimal {
        self.total
    }

    /// Nombre de montants agrégés.
    #[must_use]
    #[requirement(REQ-STA-008)]
    pub const fn count(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use wallos_req_macros::verifies;

    use super::*;

    fn eur() -> CurrencyCode {
        CurrencyCode::new("EUR").unwrap()
    }

    fn usd() -> CurrencyCode {
        CurrencyCode::new("USD").unwrap()
    }

    fn ref_date() -> AsOf {
        AsOf::new(NaiveDate::from_ymd_opt(2026, 7, 25).unwrap())
    }

    fn money(units: i64, currency: CurrencyCode) -> Money {
        Money::new(Decimal::from(units), currency).unwrap()
    }

    #[test]
    #[verifies(REQ-STA-008)]
    fn as_of_round_trip() {
        let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        assert_eq!(AsOf::new(date).date(), date);
    }

    #[test]
    #[verifies(REQ-STA-008)]
    fn compute_is_deterministic() {
        let amounts = [money(10, eur()), money(5, eur()), money(0, eur())];
        let first = Aggregate::compute(ref_date(), eur(), &amounts).unwrap();
        let second = Aggregate::compute(ref_date(), eur(), &amounts).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.total(), Decimal::from(15));
        assert_eq!(first.count(), 3);
        assert_eq!(first.currency(), eur());
        assert_eq!(first.as_of(), ref_date().date());
    }

    #[test]
    #[verifies(REQ-STA-008)]
    fn compute_empty_is_neutral() {
        let agg = Aggregate::compute(ref_date(), eur(), &[]).unwrap();
        assert_eq!(agg.total(), Decimal::ZERO);
        assert_eq!(agg.count(), 0);
        assert_eq!(agg.currency(), eur());
        assert_eq!(agg.as_of(), ref_date().date());
    }

    #[test]
    #[verifies(REQ-STA-008)]
    fn compute_rejects_mixed_currency() {
        let amounts = [money(10, eur()), money(5, usd())];
        let err = Aggregate::compute(ref_date(), eur(), &amounts).unwrap_err();
        assert!(matches!(err, DomainError::InvalidArgument(_)));
    }

    proptest! {
        #[test]
        #[verifies(REQ-STA-008)]
        fn compute_deterministic_over_random_inputs(units in proptest::collection::vec(0i64..1_000_000, 0..64)) {
            let amounts: Vec<Money> = units.iter().map(|&u| money(u, eur())).collect();
            let first = Aggregate::compute(ref_date(), eur(), &amounts).unwrap();
            let second = Aggregate::compute(ref_date(), eur(), &amounts).unwrap();
            prop_assert_eq!(first, second);
            prop_assert_eq!(first.count(), units.len());
        }
    }
}
