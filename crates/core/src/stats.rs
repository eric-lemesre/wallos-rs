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
use crate::Subscription;
use crate::money::{CurrencyCode, Money};

/// Montants entrant dans les agrégats statistiques (REQ-SUB-008).
///
/// Un abonnement **désactivé** est **exclu de tous les agrégats** : il est conservé mais ne pèse sur
/// aucun total. Seuls les abonnements actifs contribuent leur prix. C'est le point de sélection unique
/// que les exigences `REQ-STA-*` (normalisation, répartitions) et la vue liste (REQ-SUB-006) partagent.
#[requirement(REQ-SUB-008)]
pub fn billable_amounts(subscriptions: &[Subscription]) -> Vec<Money> {
    subscriptions
        .iter()
        .filter(|s| s.is_active())
        .map(|s| *s.price())
        .collect()
}

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
    use crate::billing::{BillingCycle, BillingUnit};

    fn eur() -> CurrencyCode {
        CurrencyCode::new("EUR").unwrap()
    }

    fn subscription(name: &str, price: &str, active: bool) -> Subscription {
        let sub = Subscription::new(
            uuid::Uuid::new_v4(),
            name,
            Money::new(price.parse().unwrap(), eur()).unwrap(),
            BillingCycle::from_parts(BillingUnit::Month, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        )
        .unwrap();
        sub.with_active(active)
    }

    #[test]
    #[verifies(REQ-SUB-008, case = "abonnement désactivé exclu des agrégats")]
    fn billable_amounts_excludes_inactive() {
        let subs = [
            subscription("Actif A", "10.00", true),
            subscription("Désactivé", "5.00", false),
            subscription("Actif B", "20.00", true),
        ];
        let amounts = billable_amounts(&subs);
        // Seuls les deux actifs contribuent ; le désactivé (5.00) est exclu.
        assert_eq!(amounts.len(), 2);
        let total: Decimal = amounts.iter().map(Money::amount).sum();
        assert_eq!(total, "30.00".parse().unwrap());
    }

    #[test]
    #[verifies(REQ-SUB-008, case = "tous désactivés -> agrégat vide")]
    fn billable_amounts_all_inactive_is_empty() {
        let subs = [
            subscription("X", "10.00", false),
            subscription("Y", "20.00", false),
        ];
        assert!(billable_amounts(&subs).is_empty());
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
