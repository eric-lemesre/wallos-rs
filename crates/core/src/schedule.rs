//! Calcul des échéances (REQ-SUB-012 — cycle mensuel).
//!
//! **Ancrage + clamp** (ADR 0022, override délibéré du débordement PHP de Wallos) : l'occurrence *k*
//! est calculée depuis l'**ancre** (date de premier paiement), `ancre + k × intervalle`, avec clamp
//! au dernier jour du mois quand le jour d'origine n'existe pas (`checked_add_months`). La prochaine
//! échéance est la plus petite occurrence **strictement postérieure** à la date de référence — le
//! calcul depuis l'ancre garantit le retour au 31 mars après un clamp au 28 février (jamais ancré au 28).
//!
//! Le modèle est une **date** (`NaiveDate`) : immunisé aux changements d'heure par construction (une
//! date calendaire n'a pas d'heure). La facturation à l'heure près est hors périmètre (cf. revue).

use chrono::{Days, Months, NaiveDate};
use wallos_req_macros::requirement;

use crate::billing::{BillingCycle, BillingUnit};

/// `k`-ième occurrence depuis l'ancre : `ancre + k × intervalle` (unité du cycle), ancrée et clampée.
///
/// Moteur **multi-unités** (jour, semaine, mois, année — REQ-SUB-013) : arithmétique de jours pour
/// jour/semaine (pas de clamp requis), `checked_add_months` pour mois/année (clamp fin de mois, dont
/// l'année 29 févr → 28 févr, ADR 0022). `None` si le calcul déborde la plage représentable.
#[requirement(REQ-SUB-013)]
fn occurrence(anchor: NaiveDate, cycle: BillingCycle, k: u32) -> Option<NaiveDate> {
    let steps = cycle.interval().checked_mul(k)?;
    match cycle.unit() {
        BillingUnit::Day => anchor.checked_add_days(Days::new(u64::from(steps))),
        BillingUnit::Week => anchor.checked_add_days(Days::new(u64::from(steps) * 7)),
        BillingUnit::Month => anchor.checked_add_months(Months::new(steps)),
        BillingUnit::Year => anchor.checked_add_months(Months::new(steps.checked_mul(12)?)),
    }
}

/// Borne d'itération : garde-fou contre un rattrapage démesuré (100 000 occurrences ≈ 273 ans en
/// cycle quotidien) ou une entrée pathologique. Au-delà, `next_due` renvoie `None`.
const MAX_STEPS: u32 = 100_000;

/// Prochaine échéance strictement postérieure à `after`, pour un abonnement démarré à `anchor`.
///
/// Rattrapage inclus : si plusieurs échéances sont passées, renvoie la première encore future.
///
/// Renvoie `None` si le calcul déborde la plage de dates représentable **ou** si la borne
/// [`MAX_STEPS`] est atteinte (rattrapage démesuré / entrée pathologique).
#[requirement(REQ-SUB-012)]
pub fn next_due(anchor: NaiveDate, cycle: BillingCycle, after: NaiveDate) -> Option<NaiveDate> {
    // Boucle bornée par construction. Les occurrences sont strictement croissantes ; pour une entrée
    // réelle on sort bien avant la borne (celle-ci ne mord que sur un rattrapage de plusieurs siècles).
    for k in 0..MAX_STEPS {
        let occ = occurrence(anchor, cycle, k)?;
        if occ > after {
            return Some(occ);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;
    use crate::billing::BillingUnit;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn monthly(interval: u32) -> BillingCycle {
        BillingCycle::from_parts(BillingUnit::Month, interval).unwrap()
    }

    fn cycle(unit: BillingUnit, interval: u32) -> BillingCycle {
        BillingCycle::from_parts(unit, interval).unwrap()
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "31 janv -> 28 févr (clamp, non bissextile)")]
    fn end_of_month_clamps_to_february() {
        assert_eq!(
            next_due(day(2025, 1, 31), monthly(1), day(2025, 1, 31)),
            Some(day(2025, 2, 28))
        );
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "31 janv -> 29 févr (bissextile)")]
    fn end_of_month_clamps_to_leap_february() {
        assert_eq!(
            next_due(day(2024, 1, 31), monthly(1), day(2024, 1, 31)),
            Some(day(2024, 2, 29))
        );
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "revient au 31 mars, pas ancré au 28 févr")]
    fn returns_to_31_after_clamp() {
        // Depuis l'échéance clampée au 28 févr, la suivante est le 31 mars (recalcul depuis l'ancre).
        assert_eq!(
            next_due(day(2025, 1, 31), monthly(1), day(2025, 2, 28)),
            Some(day(2025, 3, 31))
        );
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "prochaine après une date en cours de mois")]
    fn next_after_mid_month() {
        assert_eq!(
            next_due(day(2025, 1, 31), monthly(1), day(2025, 2, 15)),
            Some(day(2025, 2, 28))
        );
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "intervalle 3 (trimestriel) clampé")]
    fn quarterly_clamps() {
        // 31 janv + 3 mois -> 30 avril (avril n'a pas de 31).
        assert_eq!(
            next_due(day(2025, 1, 31), monthly(3), day(2025, 1, 31)),
            Some(day(2025, 4, 30))
        );
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "rattrapage : strictement postérieure")]
    fn catches_up_to_first_future_occurrence() {
        // Plusieurs échéances passées : renvoie la première future (31 mai), pas une date passée.
        assert_eq!(
            next_due(day(2025, 1, 31), monthly(1), day(2025, 5, 15)),
            Some(day(2025, 5, 31))
        );
    }

    #[test]
    #[verifies(REQ-SUB-013, case = "jour et semaine : arithmétique de jours")]
    fn day_and_week_units() {
        // Jour : +1 et +10 jours.
        assert_eq!(
            next_due(day(2025, 1, 1), cycle(BillingUnit::Day, 1), day(2025, 1, 1)),
            Some(day(2025, 1, 2))
        );
        assert_eq!(
            next_due(
                day(2025, 1, 1),
                cycle(BillingUnit::Day, 10),
                day(2025, 1, 1)
            ),
            Some(day(2025, 1, 11))
        );
        // Semaine : +1 semaine = +7 jours (pas +1, ni /7).
        assert_eq!(
            next_due(
                day(2025, 1, 1),
                cycle(BillingUnit::Week, 1),
                day(2025, 1, 1)
            ),
            Some(day(2025, 1, 8))
        );
        assert_eq!(
            next_due(
                day(2025, 1, 1),
                cycle(BillingUnit::Week, 2),
                day(2025, 1, 1)
            ),
            Some(day(2025, 1, 15))
        );
    }

    #[test]
    #[verifies(REQ-SUB-013, case = "année : 29 févr -> 28 févr (clamp ancré, ADR 0022, pas le débordement Wallos)")]
    fn yearly_leap_day_clamps() {
        // Oracle figé : Wallos déborde au 1er mars ; subtrack clampe au 28 févr (cohérence SUB-012).
        assert_eq!(
            next_due(
                day(2024, 2, 29),
                cycle(BillingUnit::Year, 1),
                day(2024, 2, 29)
            ),
            Some(day(2025, 2, 28))
        );
        // Année non bissextile ordinaire, et intervalle 2 (bisannuel).
        assert_eq!(
            next_due(
                day(2025, 1, 1),
                cycle(BillingUnit::Year, 1),
                day(2025, 1, 1)
            ),
            Some(day(2026, 1, 1))
        );
        assert_eq!(
            next_due(
                day(2025, 3, 15),
                cycle(BillingUnit::Year, 2),
                day(2025, 3, 15)
            ),
            Some(day(2027, 3, 15))
        );
    }

    #[test]
    #[verifies(REQ-SUB-013, case = "hebdomadaire : aucune dérive de jour de semaine sur un an")]
    fn weekly_no_weekday_drift_over_a_year() {
        use chrono::Datelike;
        // Le 1er janv 2025 est un mercredi. Toute échéance hebdomadaire tombe un mercredi (52 fois/an).
        let anchor = day(2025, 1, 1);
        let anchor_weekday = anchor.weekday();
        let mut after = anchor;
        for _ in 0..52 {
            let occ = next_due(anchor, cycle(BillingUnit::Week, 1), after).unwrap();
            assert_eq!(
                occ.weekday(),
                anchor_weekday,
                "dérive de jour de semaine à {occ}"
            );
            after = occ;
        }
        // Après 52 semaines, on est bien un an plus loin, même jour de semaine (mercredi).
        assert_eq!(after, day(2025, 12, 31));
        assert_eq!(after.weekday(), anchor_weekday);
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "date de référence antérieure à l'ancre -> ancre")]
    fn after_before_anchor_returns_anchor() {
        // Si `after` précède l'ancre, la première échéance est l'ancre elle-même (occurrence k=0).
        assert_eq!(
            next_due(day(2025, 1, 31), monthly(1), day(2024, 12, 31)),
            Some(day(2025, 1, 31))
        );
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "date de référence = occurrence exacte -> suivante (strict)")]
    fn after_equal_to_occurrence_skips_to_next() {
        // after = 31 mars (= occurrence k=2) : « strictement postérieure » -> 30 avril, pas 31 mars.
        assert_eq!(
            next_due(day(2025, 1, 31), monthly(1), day(2025, 3, 31)),
            Some(day(2025, 4, 30))
        );
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "intervalle annuel démesuré -> None (overflow borné)")]
    fn huge_yearly_interval_overflows_to_none() {
        // interval × 12 déborde u32 -> occurrence renvoie None -> next_due renvoie None (jamais un panic).
        let cycle = cycle(BillingUnit::Year, u32::MAX);
        assert_eq!(next_due(day(2025, 1, 1), cycle, day(2025, 1, 1)), None);
    }

    #[test]
    #[verifies(REQ-SUB-012, case = "date immunisée aux changements d'heure")]
    fn date_is_dst_immune() {
        // 2025-03-30 est un jour de passage à l'heure d'été en zone Europe. Au niveau DATE, l'échéance
        // est exactement le 30 mars, sans décalage (le modèle n'a pas d'heure).
        assert_eq!(
            next_due(day(2025, 1, 30), monthly(1), day(2025, 3, 1)),
            Some(day(2025, 3, 30))
        );
    }
}
