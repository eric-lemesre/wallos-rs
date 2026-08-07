//! Règle déterministe des rappels avant échéance (REQ-NOT-001).
//!
//! Un rappel est émis quand une échéance **entre dans la fenêtre** du délai configuré. Le comportement
//! de référence, capturé sur l'application d'origine (cron `sendnotifications.php`, gelé dans
//! `e2e/fixtures/oracles/REQ-NOT-001-reminders.json`), est un **déclenchement exact** : le rappel part
//! quand le nombre de jours calendaires jusqu'à la prochaine échéance est **exactement** égal au délai
//! (`difference === daysToCompare`), et non pour toute la fenêtre `[0, N]`. Un cron quotidien émet ainsi
//! chaque rappel **une fois**, le jour J−N.
//!
//! Module **pur** (REQ-STA-008) : la date « aujourd'hui » est fournie par l'appelant, aucun accès à
//! l'horloge (porte `cargo xtask lint-clock`). Le regroupement de plusieurs rappels d'un même compte en
//! un seul message (critère d'acceptation #2) relève de l'appelant.

use chrono::NaiveDate;
use uuid::Uuid;
use wallos_req_macros::requirement;

/// Délai de rappel par défaut, en jours (parité Wallos : `notification_settings.days` défaut 1).
pub const DEFAULT_REMINDER_LEAD_DAYS: u32 = 1;

/// Un abonnement candidat au rappel : sa prochaine échéance et son délai de rappel effectif.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderCandidate {
    /// Identifiant de l'abonnement.
    pub subscription_id: Uuid,
    /// Prochaine échéance (calculée par l'appelant, ≥ aujourd'hui pour un abonnement actif).
    pub next_due: NaiveDate,
    /// Délai de rappel effectif (jours avant l'échéance).
    pub lead_days: u32,
}

/// Un rappel **dû** aujourd'hui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueReminder {
    /// Abonnement concerné.
    pub subscription_id: Uuid,
    /// Date de l'échéance qui déclenche le rappel.
    pub due_date: NaiveDate,
    /// Jours calendaires jusqu'à l'échéance (égal au délai de rappel, par construction).
    pub days_until: u32,
}

/// Rappels **dus à `today`** (REQ-NOT-001).
///
/// Un candidat est dû lorsque le nombre de jours calendaires jusqu'à sa prochaine échéance est
/// **exactement** égal à son délai de rappel (déclenchement exact, oracle Wallos) et que l'échéance
/// n'est pas passée (`next_due >= today`). Résultat déterministe, ordonné par `(due_date, id)`.
///
/// Fonction **pure** : `today` est fourni (testable sans horloge, REQ-STA-008).
///
/// Le déclenchement **exact** vaut garde anti-rétroactif (REQ-NOT-002 critère #3) : une occurrence
/// dont la date de rappel est passée n'est jamais « rattrapée » — au premier démarrage comme après
/// une interruption, seuls les rappels du jour partent.
#[must_use]
#[requirement(REQ-NOT-001)]
#[requirement(REQ-NOT-002)]
pub fn due_reminders(candidates: &[ReminderCandidate], today: NaiveDate) -> Vec<DueReminder> {
    let mut due: Vec<DueReminder> = candidates
        .iter()
        .filter_map(|c| {
            if c.next_due < today {
                return None;
            }
            let days = u32::try_from((c.next_due - today).num_days()).ok()?;
            (days == c.lead_days).then_some(DueReminder {
                subscription_id: c.subscription_id,
                due_date: c.next_due,
                days_until: days,
            })
        })
        .collect();
    due.sort_by(|a, b| {
        a.due_date.cmp(&b.due_date).then_with(|| {
            a.subscription_id
                .as_bytes()
                .cmp(b.subscription_id.as_bytes())
        })
    });
    due
}

#[cfg(test)]
mod tests {
    use super::*;
    use wallos_req_macros::verifies;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn candidate(id: u128, next_due: NaiveDate, lead: u32) -> ReminderCandidate {
        ReminderCandidate {
            subscription_id: Uuid::from_u128(id),
            next_due,
            lead_days: lead,
        }
    }

    #[test]
    #[verifies(REQ-NOT-001, case = "déclenchement exact : échéance à exactement N jours (pas la fenêtre)")]
    fn fires_only_on_the_exact_lead_day() {
        let today = ymd(2026, 8, 6);
        let cands = [
            candidate(1, ymd(2026, 8, 7), 1),  // à 1 jour, délai 1 -> dû
            candidate(2, ymd(2026, 8, 6), 1),  // à 0 jour, délai 1 -> PAS dû (fenêtre exclue)
            candidate(3, ymd(2026, 8, 10), 1), // à 4 jours, délai 1 -> pas dû
        ];
        let due = due_reminders(&cands, today);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].subscription_id, Uuid::from_u128(1));
        assert_eq!(due[0].days_until, 1);
    }

    #[test]
    #[verifies(REQ-NOT-001, case = "délai par-abonnement respecté")]
    fn honours_per_subscription_lead() {
        let today = ymd(2026, 8, 6);
        // Échéance à 3 jours ; seul un candidat de délai 3 est dû.
        let cands = [
            candidate(1, ymd(2026, 8, 9), 3),
            candidate(2, ymd(2026, 8, 9), 1),
        ];
        let due = due_reminders(&cands, today);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].subscription_id, Uuid::from_u128(1));
    }

    #[test]
    #[verifies(REQ-NOT-001, case = "une échéance passée ne déclenche pas")]
    fn past_due_never_fires() {
        let today = ymd(2026, 8, 6);
        let cands = [candidate(1, ymd(2026, 8, 5), 0)];
        assert!(due_reminders(&cands, today).is_empty());
    }

    #[test]
    #[verifies(REQ-NOT-001, case = "plusieurs abonnements dus le même jour, ordonnés déterministe")]
    fn several_due_same_day_are_all_returned_ordered() {
        let today = ymd(2026, 8, 6);
        let cands = [
            candidate(2, ymd(2026, 8, 7), 1),
            candidate(1, ymd(2026, 8, 7), 1),
        ];
        let due = due_reminders(&cands, today);
        // Deux rappels dus le même jour (regroupés par l'appelant), ordonnés par id.
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].subscription_id, Uuid::from_u128(1));
        assert_eq!(due[1].subscription_id, Uuid::from_u128(2));
    }
}
