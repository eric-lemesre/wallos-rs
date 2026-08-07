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
/// Rattrapage inclus (REQ-SUB-014) : si plusieurs échéances sont passées — client hors ligne des
/// semaines, premier démarrage tardif — renvoie la **première encore future**, sans jamais exposer
/// une date passée ; l'ordonnanceur n'émet donc rien de rétroactif (garde NOT-002, jamais de
/// rafale de rappels).
///
/// Renvoie `None` si le calcul déborde la plage de dates représentable **ou** si la borne
/// [`MAX_STEPS`] est atteinte (rattrapage démesuré / entrée pathologique).
#[requirement(REQ-SUB-012)]
#[requirement(REQ-SUB-014)]
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

/// Toutes les occurrences de paiement dans la fenêtre **`[from, to]` (bornes incluses)**, pour un
/// abonnement démarré à `anchor`, de cycle `cycle`, se terminant éventuellement à `end_date`, avec une
/// éventuelle fin d'essai gratuit `trial_end` (REQ-STA-005 — échéancier ; REQ-STA-003 — exclusion
/// transverse des états non actifs).
///
/// - Énumère depuis l'**ancre** (occurrences `k = 0, 1, …`, mêmes ancrage + clamp que [`next_due`],
///   ADR 0022) ; une même récurrence peut produire **plusieurs** occurrences dans la fenêtre.
/// - Ignore les occurrences antérieures à `from` (avant le début de fenêtre / l'ancre).
/// - **Respecte `end_date`** (REQ-SUB-009) : aucune occurrence strictement postérieure à la date de fin
///   n'est incluse (la borne haute effective est `min(to, end_date)`).
/// - **Respecte `trial_end`** (REQ-SUB-010, essai gratuit) : aucun paiement n'est dû pendant l'essai,
///   donc aucune occurrence **strictement antérieure** à `trial_end` n'est incluse (la borne basse
///   effective est `max(from, trial_end)`). Une occurrence tombant exactement sur `trial_end` est due
///   (l'essai est alors terminé, cohérent avec `Subscription::is_in_trial` : essai ⇔ `date < trial_end`).
/// - Bornée par [`MAX_STEPS`] : au-delà, l'énumération s'arrête (garde-fou anti-emballement).
///
/// L'appelant est responsable d'exclure les abonnements **inactifs** (oracle Wallos `inactive = 0`,
/// REQ-SUB-008) : cette fonction pure ne connaît que les dates.
#[must_use]
#[requirement(REQ-STA-005)]
#[requirement(REQ-STA-003)]
pub fn occurrences_in_range(
    anchor: NaiveDate,
    cycle: BillingCycle,
    from: NaiveDate,
    to: NaiveDate,
    end_date: Option<NaiveDate>,
    trial_end: Option<NaiveDate>,
) -> Vec<NaiveDate> {
    // Borne haute effective : la fin de fenêtre, resserrée par la date de fin d'abonnement si elle est
    // antérieure (REQ-SUB-009). Si `end_date` précède le début de fenêtre, le résultat sera vide.
    let upper = match end_date {
        Some(end) if end < to => end,
        _ => to,
    };
    // Borne basse effective : le début de fenêtre, repoussé à la fin d'essai si elle est postérieure
    // (REQ-SUB-010 : rien n'est facturé pendant l'essai, REQ-STA-003). Si `trial_end` dépasse la borne
    // haute, le résultat sera vide.
    let lower = match trial_end {
        Some(trial) if trial > from => trial,
        _ => from,
    };
    let mut out = Vec::new();
    for k in 0..MAX_STEPS {
        // Occurrences strictement croissantes : dès qu'on dépasse la borne haute, on peut s'arrêter.
        let Some(occ) = occurrence(anchor, cycle, k) else {
            break;
        };
        if occ > upper {
            break;
        }
        if occ >= lower {
            out.push(occ);
        }
    }
    out
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
    #[verifies(REQ-STA-005, case = "plusieurs occurrences d'un même abonnement dans la fenêtre")]
    fn lists_every_occurrence_in_window() {
        // Mensuel ancré au 15 : sur [2025-01-01, 2025-03-31], trois échéances (15 janv/févr/mars).
        assert_eq!(
            occurrences_in_range(
                day(2025, 1, 15),
                monthly(1),
                day(2025, 1, 1),
                day(2025, 3, 31),
                None,
                None
            ),
            vec![day(2025, 1, 15), day(2025, 2, 15), day(2025, 3, 15)]
        );
        // Hebdomadaire sur 30 jours : plusieurs occurrences (ancre incluse car >= from).
        assert_eq!(
            occurrences_in_range(
                day(2025, 1, 1),
                cycle(BillingUnit::Week, 1),
                day(2025, 1, 1),
                day(2025, 1, 31),
                None,
                None
            ),
            vec![
                day(2025, 1, 1),
                day(2025, 1, 8),
                day(2025, 1, 15),
                day(2025, 1, 22),
                day(2025, 1, 29)
            ]
        );
    }

    #[test]
    #[verifies(REQ-STA-005, case = "date de fin dans la fenêtre : aucune occurrence postérieure")]
    fn stops_at_end_date_within_window() {
        // Mensuel ancré au 15, fenêtre jan→avril, mais fin d'abonnement au 2025-02-20 : seules les
        // occurrences <= 20 févr (15 janv, 15 févr) — jamais 15 mars/avril (REQ-SUB-009).
        assert_eq!(
            occurrences_in_range(
                day(2025, 1, 15),
                monthly(1),
                day(2025, 1, 1),
                day(2025, 4, 30),
                Some(day(2025, 2, 20)),
                None,
            ),
            vec![day(2025, 1, 15), day(2025, 2, 15)]
        );
        // Date de fin **antérieure** à la fenêtre : aucune occurrence.
        assert!(
            occurrences_in_range(
                day(2025, 1, 15),
                monthly(1),
                day(2025, 3, 1),
                day(2025, 4, 30),
                Some(day(2025, 2, 1)),
                None,
            )
            .is_empty()
        );
    }

    #[test]
    #[verifies(REQ-STA-005, case = "occurrences avant la fenêtre exclues ; clamp fin de mois conservé")]
    fn excludes_occurrences_before_window_and_keeps_clamp() {
        // Ancre au 31 janv, fenêtre févr→mars : 31 janv est avant `from` (exclu) ; 28 févr (clamp) et
        // 31 mars sont inclus.
        assert_eq!(
            occurrences_in_range(
                day(2025, 1, 31),
                monthly(1),
                day(2025, 2, 1),
                day(2025, 3, 31),
                None,
                None
            ),
            vec![day(2025, 2, 28), day(2025, 3, 31)]
        );
    }

    #[test]
    #[verifies(REQ-STA-005, case = "fenêtre vide (to < ancre) -> aucune occurrence")]
    fn empty_when_window_precedes_anchor() {
        assert!(
            occurrences_in_range(
                day(2025, 6, 1),
                monthly(1),
                day(2025, 1, 1),
                day(2025, 5, 31),
                None,
                None
            )
            .is_empty()
        );
    }

    #[test]
    #[verifies(REQ-STA-003, case = "essai gratuit : aucune occurrence antérieure à la fin d'essai (borne basse)")]
    fn trial_end_excludes_occurrences_during_trial() {
        // Mensuel ancré au 10, essai jusqu'au 2025-03-10 : les échéances des 10 janv/févr tombent
        // PENDANT l'essai (rien n'est facturé) et sont exclues ; l'occurrence du 10 mars (= fin d'essai,
        // essai terminé) et celles d'après sont dues.
        assert_eq!(
            occurrences_in_range(
                day(2025, 1, 10),
                monthly(1),
                day(2025, 1, 1),
                day(2025, 4, 30),
                None,
                Some(day(2025, 3, 10)),
            ),
            vec![day(2025, 3, 10), day(2025, 4, 10)]
        );
    }

    #[test]
    #[verifies(REQ-STA-003, case = "essai gratuit couvrant toute la fenêtre -> échéancier vide")]
    fn trial_covering_window_yields_no_occurrence() {
        // Essai jusqu'au 2025-06-01, fenêtre jan→avril : toutes les occurrences sont pendant l'essai.
        assert!(
            occurrences_in_range(
                day(2025, 1, 10),
                monthly(1),
                day(2025, 1, 1),
                day(2025, 4, 30),
                None,
                Some(day(2025, 6, 1)),
            )
            .is_empty()
        );
    }

    #[test]
    #[verifies(REQ-STA-003, case = "essai déjà terminé avant la fenêtre -> aucune restriction")]
    fn trial_ended_before_window_has_no_effect() {
        // Essai terminé au 2024-12-01 (avant `from`) : la borne basse reste `from`, toutes les
        // occurrences de la fenêtre sont dues (identique à `trial_end = None`).
        let with_past_trial = occurrences_in_range(
            day(2025, 1, 15),
            monthly(1),
            day(2025, 1, 1),
            day(2025, 3, 31),
            None,
            Some(day(2024, 12, 1)),
        );
        let without_trial = occurrences_in_range(
            day(2025, 1, 15),
            monthly(1),
            day(2025, 1, 1),
            day(2025, 3, 31),
            None,
            None,
        );
        assert_eq!(with_past_trial, without_trial);
        assert_eq!(
            with_past_trial,
            vec![day(2025, 1, 15), day(2025, 2, 15), day(2025, 3, 15)]
        );
    }

    #[test]
    #[verifies(REQ-STA-003, case = "essai + date de fin : les deux bornes se composent")]
    fn trial_and_end_date_compose() {
        // Essai jusqu'au 2025-02-10 (borne basse) ET fin d'abonnement au 2025-03-31 (borne haute) :
        // seules les occurrences dans [10 févr, 31 mars] sont dues (10 févr, 10 mars).
        assert_eq!(
            occurrences_in_range(
                day(2025, 1, 10),
                monthly(1),
                day(2025, 1, 1),
                day(2025, 5, 31),
                Some(day(2025, 3, 31)),
                Some(day(2025, 2, 10)),
            ),
            vec![day(2025, 2, 10), day(2025, 3, 10)]
        );
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

    #[test]
    #[verifies(REQ-SUB-014, case = "plusieurs échéances dépassées -> première strictement future, sans rafale")]
    fn catch_up_skips_all_past_occurrences() {
        // Abonnement mensuel ancré 18 mois plus tôt : 18 occurrences dépassées d'un coup.
        let due = next_due(day(2025, 1, 15), monthly(1), day(2026, 8, 6));
        assert_eq!(due, Some(day(2026, 8, 15)));
        // Strictement postérieure à la date courante, jamais une occurrence passée.
        assert!(due.unwrap() > day(2026, 8, 6));
    }

    #[test]
    #[verifies(REQ-SUB-014, case = "le rattrapage hebdomadaire converge aussi (pas de dérive)")]
    fn catch_up_weekly_lands_on_future_anchor_weekday() {
        // Hebdomadaire ancré un lundi (2025-01-06), ~1,5 an plus tard : premier lundi futur.
        let due = next_due(
            day(2025, 1, 6),
            cycle(BillingUnit::Week, 1),
            day(2026, 8, 6),
        )
        .unwrap();
        assert!(due > day(2026, 8, 6));
        // Toujours sur le jour d'ancrage : un nombre entier de semaines depuis l'ancre.
        assert_eq!((due - day(2025, 1, 6)).num_days() % 7, 0);
    }
}
