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
use crate::billing::BillingUnit;
use crate::money::{CurrencyCode, Money};

/// Coût **mensuel normalisé** d'un abonnement (REQ-STA-001), dans sa propre devise.
///
/// La méthode de normalisation d'un cycle (quotidien / hebdomadaire / annuel) vers un mois est une
/// **convention capturée sur l'application d'origine** — pas une évidence mathématique. Facteurs gelés
/// dans `e2e/fixtures/oracles/REQ-STA-001-monthly.json` (Wallos 5.4.2 `getPricePerMonth`, vérifiés en
/// exécutant le PHP de l'image pinnée) :
/// - **jour** : `prix × 30 / intervalle` (30 jours/mois) ;
/// - **semaine** : `prix × 4.35 / intervalle` (4.35 semaines/mois) ;
/// - **mois** : `prix / intervalle` ;
/// - **année** : `prix / (12 × intervalle)` (12 mois/an).
///
/// Arithmétique **décimale exacte** (R4) : les facteurs `30`, `4.35`, `12` sont représentables sans
/// perte. L'arrondi d'affichage relève de REQ-CUR-005 (hors de cette normalisation). La conversion
/// entre devises (agrégats multi-devises) relève des `REQ-STA-*` suivantes ; ici le résultat reste
/// dans la devise de l'abonnement.
#[requirement(REQ-STA-001)]
#[must_use]
pub fn monthly_cost(price: Money, cycle: crate::billing::BillingCycle) -> Money {
    let amount = price.amount();
    let currency = price.currency();
    let interval = Decimal::from(cycle.interval());
    // Facteurs de normalisation capturés (décimaux exacts) : 30 j/mois, 4.35 sem/mois, 12 mois/an.
    let monthly = match cycle.unit() {
        BillingUnit::Day => amount * Decimal::from(30) / interval,
        BillingUnit::Week => amount * Decimal::new(435, 2) / interval,
        BillingUnit::Month => amount / interval,
        BillingUnit::Year => amount / (Decimal::from(12) * interval),
    };
    // Toujours ≥ 0 (prix ≥ 0, facteurs et intervalle > 0) : le repli `zero` n'est jamais atteint,
    // il évite seulement un `unwrap` (R5).
    Money::new(monthly, currency).unwrap_or_else(|_| Money::zero(currency))
}

/// Coût mensuel normalisé **arrondi pour l'affichage** aux décimales de la devise — la valeur **exacte
/// affichée par l'application d'origine** (REQ-STA-001 acceptation), composant la normalisation
/// (facteurs capturés) et l'arrondi bancaire (REQ-CUR-005) au nombre de décimales de la devise
/// (REQ-CUR-007 ; 2 pour EUR/USD, 0 pour JPY). [`monthly_cost`] reste la valeur **brute** (précision
/// pleine) que les agrégats consomment.
#[requirement(REQ-STA-001)]
#[must_use]
pub fn monthly_cost_rounded(price: Money, cycle: crate::billing::BillingCycle) -> Money {
    let raw = monthly_cost(price, cycle);
    let decimals = crate::currencies::find(price.currency().as_str()).map_or(2, |c| c.decimals);
    raw.round_to(decimals)
}

/// Coût **annuel normalisé** d'un abonnement (REQ-STA-002), dans sa propre devise.
///
/// **Relation capturée sur l'application d'origine** : `annuel = mensuel × 12` (Wallos 5.4.2
/// `stats_calculations.php` — `$totalCostPerYear = $totalCostPerMonth * 12`). Les deux indicateurs
/// ne peuvent donc pas diverger : le coût annuel est **exactement** douze fois le coût mensuel
/// normalisé (valeur brute, précision pleine). Arithmétique décimale exacte (R4).
#[requirement(REQ-STA-002)]
#[must_use]
pub fn yearly_cost(price: Money, cycle: crate::billing::BillingCycle) -> Money {
    let monthly = monthly_cost(price, cycle);
    let currency = monthly.currency();
    // Toujours ≥ 0 : repli `zero` jamais atteint (évite un `unwrap`, R5).
    Money::new(monthly.amount() * Decimal::from(12), currency)
        .unwrap_or_else(|_| Money::zero(currency))
}

/// Coût annuel normalisé **arrondi pour l'affichage** aux décimales de la devise (REQ-STA-002 +
/// REQ-CUR-005/CUR-007) — la valeur affichée par l'application d'origine. [`yearly_cost`] reste la
/// valeur brute.
#[requirement(REQ-STA-002)]
#[must_use]
pub fn yearly_cost_rounded(price: Money, cycle: crate::billing::BillingCycle) -> Money {
    let raw = yearly_cost(price, cycle);
    let decimals = crate::currencies::find(price.currency().as_str()).map_or(2, |c| c.decimals);
    raw.round_to(decimals)
}

/// Montants entrant dans les agrégats statistiques à la date `reference` (REQ-SUB-008 / REQ-SUB-009).
///
/// Un abonnement **désactivé** (REQ-SUB-008) ou **terminé** (date de fin dépassée à `reference`,
/// REQ-SUB-009) est **exclu de tous les agrégats** : il est conservé mais ne pèse sur aucun total.
/// Seuls les abonnements actifs et non terminés contribuent leur prix. C'est la primitive de domaine
/// que les exigences `REQ-STA-*` consomment ; côté API, la vue liste applique la même règle au niveau
/// des lignes stockées (`server::subscriptions::active_amounts`).
#[requirement(REQ-SUB-008)]
#[requirement(REQ-SUB-009)]
pub fn billable_amounts(subscriptions: &[Subscription], reference: NaiveDate) -> Vec<Money> {
    subscriptions
        .iter()
        .filter(|s| s.is_active() && !s.has_ended(reference))
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

    fn ref_day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
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

    fn sub_cycle(price: &str, unit: BillingUnit, interval: u32) -> Subscription {
        Subscription::new(
            uuid::Uuid::new_v4(),
            "X",
            Money::new(price.parse().unwrap(), eur()).unwrap(),
            BillingCycle::from_parts(unit, interval).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        )
        .unwrap()
    }

    #[test]
    #[verifies(REQ-STA-001, case = "facteurs de normalisation mensuelle capturés sur Wallos")]
    fn monthly_cost_matches_the_wallos_oracle() {
        // Vecteurs gelés dans e2e/fixtures/oracles/REQ-STA-001-monthly.json (Wallos 5.4.2
        // getPricePerMonth, vérifiés en exécutant le PHP de l'image pinnée).
        let cases: &[(BillingUnit, u32, &str, &str)] = &[
            (BillingUnit::Year, 1, "120.00", "10"),
            (BillingUnit::Year, 1, "99.99", "8.3325"),
            (BillingUnit::Year, 2, "240.00", "10"),
            (BillingUnit::Week, 1, "10.00", "43.5"),
            (BillingUnit::Week, 2, "10.00", "21.75"),
            (BillingUnit::Day, 1, "1.00", "30"),
            (BillingUnit::Day, 7, "7.00", "30"),
            (BillingUnit::Month, 1, "15.00", "15"),
            (BillingUnit::Month, 3, "15.00", "5"),
        ];
        for (unit, interval, price, expected) in cases {
            let sub = sub_cycle(price, *unit, *interval);
            let normalized = monthly_cost(*sub.price(), sub.cycle());
            assert_eq!(
                normalized.amount(),
                expected.parse::<Decimal>().unwrap(),
                "{unit:?} interval={interval} price={price}"
            );
            // La normalisation ne change pas la devise (pas de conversion ici).
            assert_eq!(normalized.currency(), eur());
        }
    }

    #[test]
    #[verifies(REQ-STA-001, case = "coût mensuel affiché = normalisation + arrondi devise (EUR, 2 déc.)")]
    fn monthly_cost_rounded_matches_the_wallos_display() {
        // Valeur AFFICHÉE (arrondi bancaire aux décimales EUR) : combine le facteur capturé et REQ-CUR-005.
        let cases: &[(BillingUnit, u32, &str, &str)] = &[
            (BillingUnit::Year, 1, "120.00", "10.00"),
            (BillingUnit::Year, 1, "99.99", "8.33"), // 8.3325 arrondi à 2 déc. -> 8.33
            (BillingUnit::Week, 1, "10.00", "43.50"),
            (BillingUnit::Week, 2, "10.00", "21.75"),
            (BillingUnit::Month, 3, "15.00", "5.00"),
        ];
        for (unit, interval, price, expected) in cases {
            let sub = sub_cycle(price, *unit, *interval);
            let shown = monthly_cost_rounded(*sub.price(), sub.cycle());
            assert_eq!(
                shown.amount(),
                expected.parse::<Decimal>().unwrap(),
                "{unit:?} interval={interval} price={price}"
            );
        }
    }

    #[test]
    #[verifies(REQ-STA-002, case = "coût annuel = mensuel × 12, valeurs capturées sur Wallos")]
    fn yearly_cost_matches_the_wallos_oracle() {
        // Vecteurs gelés dans e2e/fixtures/oracles/REQ-STA-002-yearly.json (annuel = mensuel*12,
        // vérifié en exécutant le PHP de l'image pinnée).
        let cases: &[(BillingUnit, u32, &str, &str)] = &[
            (BillingUnit::Year, 1, "120.00", "120"),
            (BillingUnit::Year, 1, "99.99", "99.99"),
            (BillingUnit::Week, 1, "10.00", "522"),
            (BillingUnit::Week, 2, "10.00", "261"),
            (BillingUnit::Day, 1, "1.00", "360"),
            (BillingUnit::Month, 1, "15.00", "180"),
            (BillingUnit::Month, 3, "15.00", "60"),
        ];
        for (unit, interval, price, expected) in cases {
            let sub = sub_cycle(price, *unit, *interval);
            let yearly = yearly_cost(*sub.price(), sub.cycle());
            assert_eq!(
                yearly.amount(),
                expected.parse::<Decimal>().unwrap(),
                "{unit:?} interval={interval} price={price}"
            );
        }
    }

    #[test]
    #[verifies(REQ-STA-002, case = "relation stable : annuel ≡ mensuel × 12 (les deux ne divergent pas)")]
    fn yearly_is_always_twelve_times_monthly() {
        // La cohérence entre les deux indicateurs (REQ-STA-002) : pour tout cycle/prix, annuel = mensuel*12.
        for unit in [
            BillingUnit::Day,
            BillingUnit::Week,
            BillingUnit::Month,
            BillingUnit::Year,
        ] {
            for interval in [1u32, 2, 3, 7] {
                let sub = sub_cycle("42.37", unit, interval);
                let monthly = monthly_cost(*sub.price(), sub.cycle());
                let yearly = yearly_cost(*sub.price(), sub.cycle());
                assert_eq!(yearly.amount(), monthly.amount() * Decimal::from(12));
            }
        }
    }

    #[test]
    #[verifies(REQ-SUB-008, case = "abonnement désactivé exclu des agrégats")]
    fn billable_amounts_excludes_inactive() {
        let subs = [
            subscription("Actif A", "10.00", true),
            subscription("Désactivé", "5.00", false),
            subscription("Actif B", "20.00", true),
        ];
        let amounts = billable_amounts(&subs, ref_day());
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
        assert!(billable_amounts(&subs, ref_day()).is_empty());
    }

    #[test]
    #[verifies(REQ-SUB-009, case = "abonnement terminé exclu des agrégats")]
    fn billable_amounts_excludes_ended() {
        // Terminé au 2026-05-31 (< reference 2026-06-01) -> exclu ; l'autre a une fin future -> inclus.
        let ended = subscription("Terminé", "5.00", true)
            .with_end_date(NaiveDate::from_ymd_opt(2026, 5, 31).unwrap());
        let ongoing = subscription("En cours", "20.00", true)
            .with_end_date(NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
        let amounts = billable_amounts(&[ended, ongoing], ref_day());
        assert_eq!(amounts.len(), 1);
        assert_eq!(amounts[0].amount(), "20.00".parse().unwrap());
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
