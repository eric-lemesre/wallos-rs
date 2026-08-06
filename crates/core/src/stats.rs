//! Agrégats statistiques déterministes.
//!
//! REQ-STA-008 — « Détermination des agrégats » : un agrégat est une **fonction pure**
//! d'un jeu de données et d'une **date de référence fournie explicitement**. Aucun accès
//! à l'horloge système n'est autorisé dans ce module (porte `cargo xtask lint-clock`).
//!
//! Ce module pose le socle de reproductibilité que les exigences `REQ-STA-*` ultérieures
//! (normalisation mensuelle, répartitions, échéancier) consommeront. Il reste volontairement
//! générique et ne préempte aucune sémantique métier `oracle: legacy`.

use std::collections::HashMap;

use chrono::{Datelike, Days, Months, NaiveDate};
use rust_decimal::Decimal;
use uuid::Uuid;
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
    // `Money::new` ne rejette que le négatif ; le résultat est toujours ≥ 0 (prix ≥ 0, facteurs et
    // intervalle > 0), donc il **ne peut pas échouer** ici. Le repli est **purement mécanique**
    // (satisfaction de R5, pas d'`unwrap`) — il n'a **aucune** sémantique métier et ne couvre pas un
    // éventuel overflow décimal (propriété partagée par toute l'arithmétique `Decimal` du dépôt).
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
    // Résultat toujours ≥ 0 : `Money::new` ne peut pas échouer ici. Repli **purement mécanique** (R5),
    // sans sémantique métier (cf. `monthly_cost`).
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
/// Un abonnement **désactivé** (REQ-SUB-008), **terminé** (date de fin dépassée à `reference`,
/// REQ-SUB-009) **ou en période d'essai gratuit** (REQ-SUB-010, essai non encore terminé à `reference`)
/// est **exclu de tous les agrégats** : il est conservé mais ne pèse sur aucun total. Seuls les
/// abonnements actifs, non terminés et hors essai contribuent leur prix. C'est la primitive de domaine
/// que les exigences `REQ-STA-*` consomment ; côté API, la vue liste applique la même règle au niveau
/// des lignes stockées (`server::subscriptions::active_amounts`).
#[requirement(REQ-SUB-008)]
#[requirement(REQ-SUB-009)]
#[requirement(REQ-SUB-010)]
#[requirement(REQ-STA-003)]
pub fn billable_amounts(subscriptions: &[Subscription], reference: NaiveDate) -> Vec<Money> {
    subscriptions
        .iter()
        .filter(|s| s.is_active() && !s.has_ended(reference) && !s.is_in_trial(reference))
        .map(|s| *s.price())
        .collect()
}

/// Fenêtre d'activité d'un abonnement pour la série d'évolution (REQ-STA-006), avec son **coût mensuel
/// déjà normalisé** (REQ-STA-001) et exprimé dans une **devise commune** — la conversion multi-devises
/// relève de l'appelant (le domaine reste sans I/O ni table de taux).
#[derive(Debug, Clone)]
pub struct CostSpan {
    /// Début d'activité : date de premier paiement (l'abonnement n'existe pas avant).
    pub start: NaiveDate,
    /// Fin d'activité **incluse**, le cas échéant (date de fin programmée, REQ-SUB-009).
    pub end: Option<NaiveDate>,
    /// Coût mensuel normalisé, dans la devise commune de la série.
    pub monthly: Decimal,
}

/// Point de la série d'évolution : premier jour du mois considéré + total mensuel **à cette date**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthlyCostPoint {
    /// Premier jour du mois du point.
    pub month: NaiveDate,
    /// Somme des coûts mensuels des abonnements actifs ce mois-là (devise commune).
    pub total: Decimal,
}

/// Premier jour du mois de `date` (jamais `None` : le 1 existe dans tout mois).
#[requirement(REQ-STA-006)]
fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

/// Dernier jour du mois de `date` : (1er du mois + 1 mois) − 1 jour.
#[requirement(REQ-STA-006)]
fn last_of_month(date: NaiveDate) -> NaiveDate {
    let first = first_of_month(date);
    first
        .checked_add_months(Months::new(1))
        .and_then(|d| d.checked_sub_days(Days::new(1)))
        .unwrap_or(date)
}

/// Série d'évolution du coût mensuel sur `months` mois **s'achevant au mois de `anchor`** (inclus),
/// du plus ancien au plus récent (REQ-STA-006).
///
/// Chaque point somme les coûts des abonnements **actifs à ce mois-là** — c.-à-d. dont la fenêtre
/// d'activité `[start, end]` **recoupe** le mois : `start <= dernier jour du mois` ET (`end` absente OU
/// `end >= premier jour du mois`). La série reflète ainsi l'historique via les dates de début/fin
/// (un abonnement récent n'apparaît que sur les derniers points ; un abonnement terminé disparaît après
/// sa date de fin), **jamais l'état courant projeté** sur toute la période (acceptation STA-006).
///
/// Fonction **pure** (REQ-STA-008) : `anchor` est fourni par l'appelant, aucun accès à l'horloge. Un
/// `months` nul donne une série vide.
#[requirement(REQ-STA-006)]
#[must_use]
pub fn monthly_cost_evolution(
    spans: &[CostSpan],
    anchor: NaiveDate,
    months: u32,
) -> Vec<MonthlyCostPoint> {
    let anchor_month = first_of_month(anchor);
    // Du plus ancien au plus récent : `anchor_month - (months-1)` … `anchor_month`.
    (0..months)
        .rev()
        .filter_map(|back| anchor_month.checked_sub_months(Months::new(back)))
        .map(|month_first| {
            let month_last = last_of_month(month_first);
            let total = spans
                .iter()
                .filter(|s| s.start <= month_last && s.end.is_none_or(|end| end >= month_first))
                .map(|s| s.monthly)
                .sum();
            MonthlyCostPoint {
                month: month_first,
                total,
            }
        })
        .collect()
}

/// Contribution d'un abonnement à une **répartition** sur un axe (REQ-STA-004).
///
/// `key` identifie le bucket de l'axe (catégorie **ou** payeur) ; `None` signifie « sans catégorie »
/// / « sans payeur » — l'abonnement n'est jamais écarté, il alimente un bucket explicite (critère
/// d'acceptation #2). `monthly` est le coût mensuel **déjà normalisé** (REQ-STA-001) et exprimé dans
/// la **devise commune** de la répartition — la conversion multi-devises relève de l'appelant (le
/// domaine reste sans I/O ni table de taux, comme [`CostSpan`]).
#[derive(Debug, Clone)]
pub struct RepartitionSlice {
    /// Clé du bucket d'axe ; `None` = sans catégorie / sans payeur.
    pub key: Option<Uuid>,
    /// Coût mensuel normalisé, dans la devise commune de la répartition.
    pub monthly: Decimal,
}

/// Part d'un bucket dans une répartition (REQ-STA-004) : sa clé, le total accumulé en **précision
/// pleine** et le nombre d'abonnements contributeurs. Le libellé (nom de catégorie/payeur, ou
/// « (aucun) » pour `key == None`) est résolu par l'appelant — le domaine ne connaît que les clés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepartitionShare {
    /// Clé du bucket ; `None` = sans catégorie / sans payeur.
    pub key: Option<Uuid>,
    /// Somme des coûts mensuels des abonnements du bucket (devise commune, précision pleine).
    pub total: Decimal,
    /// Nombre d'abonnements contributeurs de ce bucket.
    pub count: usize,
}

/// Répartit des contributions par clé d'axe (REQ-STA-004), du bucket le plus lourd au plus léger.
///
/// Chaque `slice` alimente **exactement un** bucket (sa `key`), si bien que la **somme des totaux des
/// parts est exactement égale à la somme des contributions** — sans écart d'arrondi, l'accumulation
/// restant en précision décimale pleine (critère d'acceptation #1 ; c'est la mécanique capturée sur
/// l'application d'origine — `categoryCost`/`memberCost` de `stats_calculations.php`). Les
/// contributions de clé `None` sont regroupées dans un unique bucket « sans » explicite, **jamais
/// omis** (critère #2).
///
/// Ordre : coût **décroissant** (comme l'application d'origine, `usort $b['y'] <=> $a['y']`), puis
/// départage **déterministe** stable — nombre de contributeurs décroissant, puis clé (`Some` avant
/// `None`, `Some` entre eux par octets d'UUID) — pour un rendu reproductible sur totaux égaux.
///
/// Fonction **pure** (REQ-STA-008) : aucun accès à l'horloge, résultat fonction des seules entrées.
#[requirement(REQ-STA-004)]
#[must_use]
pub fn repartition(slices: &[RepartitionSlice]) -> Vec<RepartitionShare> {
    let mut buckets: HashMap<Option<Uuid>, (Decimal, usize)> = HashMap::new();
    for slice in slices {
        let entry = buckets.entry(slice.key).or_insert((Decimal::ZERO, 0));
        entry.0 += slice.monthly;
        entry.1 += 1;
    }
    let mut shares: Vec<RepartitionShare> = buckets
        .into_iter()
        .map(|(key, (total, count))| RepartitionShare { key, total, count })
        .collect();
    shares.sort_by(|a, b| {
        // Total décroissant, puis nombre de contributeurs décroissant, puis clé (Some avant None,
        // Some par octets d'UUID) — ordre total déterministe, indépendant de l'itération du HashMap.
        b.total
            .cmp(&a.total)
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| match (a.key, b.key) {
                (Some(x), Some(y)) => x.as_bytes().cmp(y.as_bytes()),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
    });
    shares
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
        sub_cycle_cur(price, unit, interval, "EUR")
    }

    fn sub_cycle_cur(price: &str, unit: BillingUnit, interval: u32, code: &str) -> Subscription {
        Subscription::new(
            uuid::Uuid::new_v4(),
            "X",
            Money::new(price.parse().unwrap(), CurrencyCode::new(code).unwrap()).unwrap(),
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
    #[verifies(REQ-STA-001, case = "l'arrondi d'affichage suit les décimales de la DEVISE (JPY=0, borne half-to-even)")]
    fn rounding_uses_the_currency_decimals() {
        // JPY = 0 décimale (REQ-CUR-007) : exerce le chemin `round_to(decimals)` avec decimals ≠ 2 —
        // un bug renvoyant un `2` codé en dur passerait tous les autres vecteurs (EUR). Revue STA-002 F3.
        let jpy_day = sub_cycle_cur("100", BillingUnit::Day, 7, "JPY"); // mensuel brut 428.5714…
        assert_eq!(
            monthly_cost_rounded(*jpy_day.price(), jpy_day.cycle()).amount(),
            Decimal::from(429), // 428.57 -> 429 (0 décimale)
        );
        assert_eq!(
            yearly_cost_rounded(*jpy_day.price(), jpy_day.cycle()).amount(),
            Decimal::from(5143), // 5142.857 -> 5143 (0 décimale)
        );
        // Borne half-to-even à 0 décimale : 2.5 -> 2 (pair), pas 3.
        let jpy_half = sub_cycle_cur("5", BillingUnit::Month, 2, "JPY"); // mensuel brut 2.5
        assert_eq!(
            monthly_cost_rounded(*jpy_half.price(), jpy_half.cycle()).amount(),
            Decimal::from(2),
        );
        // Contraste : le MÊME abonnement en EUR (2 décimales) donne 428.57, prouvant que les décimales
        // viennent bien de la devise et ne sont pas codées en dur.
        let eur_day = sub_cycle_cur("100", BillingUnit::Day, 7, "EUR");
        assert_eq!(
            monthly_cost_rounded(*eur_day.price(), eur_day.cycle()).amount(),
            "428.57".parse::<Decimal>().unwrap(),
        );
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
    #[verifies(REQ-SUB-010, case = "abonnement en essai gratuit exclu des agrégats jusqu'à la fin d'essai")]
    fn billable_amounts_excludes_in_trial() {
        // En essai jusqu'au 2026-07-01 : exclu au 2026-06-01 (avant), compté au 2026-07-01 (fin d'essai).
        let trial = subscription("Essai", "10.00", true)
            .with_trial_end(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
        let normal = subscription("Normal", "20.00", true);
        let during = billable_amounts(&[trial.clone(), normal.clone()], ref_day());
        assert_eq!(during.len(), 1);
        assert_eq!(during[0].amount(), "20.00".parse().unwrap());
        // À la fin d'essai (inclus), l'abonnement compte de nouveau.
        let after = billable_amounts(&[trial], NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].amount(), "10.00".parse().unwrap());
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

    #[test]
    #[verifies(REQ-STA-003, case = "règle transverse : les trois états sont exclus simultanément, seuls les actifs pèsent")]
    fn billable_amounts_excludes_every_non_active_state_at_once() {
        // Un jeu couvrant les trois exclusions + un actif témoin, à la même date de référence.
        let disabled = subscription("Désactivé", "1.00", false);
        let ended = subscription("Terminé", "2.00", true)
            .with_end_date(NaiveDate::from_ymd_opt(2026, 5, 31).unwrap());
        let in_trial = subscription("Essai", "4.00", true)
            .with_trial_end(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
        let active = subscription("Actif", "8.00", true);
        let amounts = billable_amounts(&[disabled, ended, in_trial, active], ref_day());
        // Seul l'actif (8.00) contribue : les trois autres états sont exclus « selon la règle propre ».
        assert_eq!(amounts.len(), 1);
        assert_eq!(amounts[0].amount(), "8.00".parse().unwrap());
    }

    #[test]
    #[verifies(REQ-STA-003, case = "critère #2 : un abonnement réactivé est immédiatement réintégré")]
    fn reactivated_subscription_is_immediately_reincluded() {
        // Désactivé : exclu. Réactivé (with_active(true)) : réintégré au recalcul suivant, sans étape.
        let disabled = subscription("Bascule", "12.00", true).with_active(false);
        assert!(billable_amounts(std::slice::from_ref(&disabled), ref_day()).is_empty());
        let reactivated = disabled.with_active(true);
        let amounts = billable_amounts(std::slice::from_ref(&reactivated), ref_day());
        assert_eq!(amounts.len(), 1);
        assert_eq!(amounts[0].amount(), "12.00".parse().unwrap());
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

    // --- Évolution du coût sur douze mois (REQ-STA-006) ---

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn span(start: NaiveDate, end: Option<NaiveDate>, monthly: &str) -> CostSpan {
        CostSpan {
            start,
            end,
            monthly: monthly.parse().unwrap(),
        }
    }

    fn totals(points: &[MonthlyCostPoint]) -> Vec<Decimal> {
        points.iter().map(|p| p.total).collect()
    }

    #[test]
    #[verifies(REQ-STA-006)]
    fn series_covers_the_requested_months_ending_at_anchor() {
        let s = [span(ymd(2020, 1, 1), None, "10")];
        let points = monthly_cost_evolution(&s, ymd(2026, 6, 15), 12);
        assert_eq!(points.len(), 12);
        // Du plus ancien (juillet 2025) au plus récent (juin 2026), premiers de mois.
        assert_eq!(points.first().unwrap().month, ymd(2025, 7, 1));
        assert_eq!(points.last().unwrap().month, ymd(2026, 6, 1));
        // Abonnement actif tout du long : total constant.
        assert!(totals(&points).iter().all(|t| *t == dec("10")));
    }

    #[test]
    #[verifies(REQ-STA-006)]
    fn recent_subscription_appears_only_from_its_start_month() {
        // Commence en avril 2026 : nul avant, 25 à partir d'avril (le mois du start compte).
        let s = [span(ymd(2026, 4, 10), None, "25")];
        let points = monthly_cost_evolution(&s, ymd(2026, 6, 1), 12);
        // Points: 2025-07 .. 2026-06. Avril 2026 est l'indice 9.
        assert_eq!(points[8].month, ymd(2026, 3, 1));
        assert_eq!(points[8].total, dec("0"));
        assert_eq!(points[9].month, ymd(2026, 4, 1));
        assert_eq!(points[9].total, dec("25"));
        assert_eq!(points[11].total, dec("25"));
    }

    #[test]
    #[verifies(REQ-STA-006)]
    fn ended_subscription_disappears_after_its_end_month() {
        // Se termine le 15 mars 2026 : compte jusqu'en mars (recoupe le mois), nul ensuite.
        let s = [span(ymd(2020, 1, 1), Some(ymd(2026, 3, 15)), "40")];
        let points = monthly_cost_evolution(&s, ymd(2026, 6, 1), 12);
        // mars 2026 = indice 8, avril = 9.
        assert_eq!(points[8].month, ymd(2026, 3, 1));
        assert_eq!(points[8].total, dec("40"));
        assert_eq!(points[9].month, ymd(2026, 4, 1));
        assert_eq!(points[9].total, dec("0"));
    }

    #[test]
    #[verifies(REQ-STA-006)]
    fn point_sums_subscriptions_live_that_month_not_current_state() {
        // Deux abonnements : A actif depuis toujours, B seulement de févr. à avril 2026.
        let s = [
            span(ymd(2020, 1, 1), None, "10"),
            span(ymd(2026, 2, 1), Some(ymd(2026, 4, 30)), "5"),
        ];
        let points = monthly_cost_evolution(&s, ymd(2026, 6, 1), 12);
        // janv 2026 (idx 6) = 10 ; févr (7) = 15 ; mars (8) = 15 ; avril (9) = 15 ; mai (10) = 10.
        assert_eq!(points[6].total, dec("10"));
        assert_eq!(points[7].total, dec("15"));
        assert_eq!(points[9].total, dec("15"));
        assert_eq!(points[10].total, dec("10"));
    }

    #[test]
    #[verifies(REQ-STA-006)]
    fn zero_months_yields_empty_series() {
        let s = [span(ymd(2020, 1, 1), None, "10")];
        assert!(monthly_cost_evolution(&s, ymd(2026, 6, 1), 0).is_empty());
    }

    // --- Répartition par catégorie / par payeur (REQ-STA-004) ---

    fn slice(key: Option<Uuid>, monthly: &str) -> RepartitionSlice {
        RepartitionSlice {
            key,
            monthly: monthly.parse().unwrap(),
        }
    }

    fn share_of(shares: &[RepartitionShare], key: Option<Uuid>) -> &RepartitionShare {
        shares.iter().find(|s| s.key == key).unwrap()
    }

    #[test]
    #[verifies(REQ-STA-004, case = "somme des parts = total général, sans écart d'arrondi")]
    fn shares_sum_to_the_grand_total() {
        // Exemple travaillé gelé (REQ-STA-004-repartition.json) : Streaming = 10 + 10, sans axe = 5.
        let streaming = Uuid::from_u128(1);
        let slices = [
            slice(Some(streaming), "10"),
            slice(Some(streaming), "10"),
            slice(None, "5"),
        ];
        let shares = repartition(&slices);
        // Deux buckets : Streaming (20, 2 contributeurs) et « (aucun) » (5, 1).
        assert_eq!(shares.len(), 2);
        assert_eq!(share_of(&shares, Some(streaming)).total, dec("20"));
        assert_eq!(share_of(&shares, Some(streaming)).count, 2);
        assert_eq!(share_of(&shares, None).total, dec("5"));
        assert_eq!(share_of(&shares, None).count, 1);
        // Invariant #1 : la somme des parts égale exactement la somme des contributions.
        let grand_total: Decimal = slices.iter().map(|s| s.monthly).sum();
        let sum_of_parts: Decimal = shares.iter().map(|s| s.total).sum();
        assert_eq!(sum_of_parts, grand_total);
        assert_eq!(sum_of_parts, dec("25"));
    }

    #[test]
    #[verifies(REQ-STA-004, case = "abonnements sans axe regroupés dans une entrée explicite, jamais omis")]
    fn missing_axis_is_grouped_into_an_explicit_bucket() {
        // Aucun abonnement n'a de clé : tout atterrit dans l'unique bucket « (aucun) », jamais écarté.
        let slices = [slice(None, "7.30"), slice(None, "2.70")];
        let shares = repartition(&slices);
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].key, None);
        assert_eq!(shares[0].total, dec("10.00"));
        assert_eq!(shares[0].count, 2);
    }

    #[test]
    #[verifies(REQ-STA-004, case = "parts triées par coût décroissant")]
    fn shares_are_sorted_by_cost_descending() {
        let a = Uuid::from_u128(10);
        let b = Uuid::from_u128(20);
        // b plus lourd que a ; le bucket « (aucun) » le plus léger.
        let slices = [slice(Some(a), "3"), slice(Some(b), "8"), slice(None, "1")];
        let shares = repartition(&slices);
        let totals: Vec<Decimal> = shares.iter().map(|s| s.total).collect();
        assert_eq!(totals, vec![dec("8"), dec("3"), dec("1")]);
        assert_eq!(shares[0].key, Some(b));
        assert_eq!(shares[2].key, None);
    }

    #[test]
    #[verifies(REQ-STA-004, case = "totaux égaux : ordre déterministe (Some avant None, puis octets d'UUID)")]
    fn equal_totals_break_ties_deterministically() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        // Trois buckets à total égal : départage Some(a) < Some(b) < None.
        let slices = [slice(Some(b), "5"), slice(None, "5"), slice(Some(a), "5")];
        let first = repartition(&slices);
        let keys: Vec<Option<Uuid>> = first.iter().map(|s| s.key).collect();
        assert_eq!(keys, vec![Some(a), Some(b), None]);
        // Déterminisme : un ordre d'entrée différent produit le même résultat.
        let reordered = [slice(None, "5"), slice(Some(a), "5"), slice(Some(b), "5")];
        assert_eq!(repartition(&reordered), first);
    }

    #[test]
    #[verifies(REQ-STA-004, case = "aucune contribution -> répartition vide")]
    fn empty_input_yields_empty_repartition() {
        assert!(repartition(&[]).is_empty());
    }

    proptest! {
        #[test]
        #[verifies(REQ-STA-004, case = "invariant somme=total sur entrées aléatoires")]
        fn sum_of_shares_always_equals_sum_of_slices(
            raw in proptest::collection::vec((proptest::option::of(0u128..8), 0i64..1_000_000), 0..64)
        ) {
            let slices: Vec<RepartitionSlice> = raw
                .iter()
                .map(|(k, cents)| RepartitionSlice {
                    key: k.map(Uuid::from_u128),
                    monthly: Decimal::new(*cents, 2),
                })
                .collect();
            let shares = repartition(&slices);
            let sum_slices: Decimal = slices.iter().map(|s| s.monthly).sum();
            let sum_shares: Decimal = shares.iter().map(|s| s.total).sum();
            prop_assert_eq!(sum_slices, sum_shares);
            let count_slices = slices.len();
            let count_shares: usize = shares.iter().map(|s| s.count).sum();
            prop_assert_eq!(count_slices, count_shares);
        }
    }
}
