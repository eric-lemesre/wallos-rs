//! Logique déterministe des pierres tombales (REQ-SYN-002).
//!
//! Une **pierre tombale** trace la suppression d'une entité répliquée pour qu'un appareil hors ligne
//! applique la suppression au lieu de réintroduire une entité qu'il croit vivante. Le serveur **purge**
//! les pierres tombales plus anciennes que la fenêtre de **rétention** (ADR 0013) ; au-delà, un appareil
//! dont le dernier curseur `since` précède la fenêtre a pu **manquer** des suppressions désormais
//! purgées : il doit alors être contraint à une **resynchronisation complète**.
//!
//! Ce module est **pur** (REQ-STA-008) : la date « maintenant » et la fenêtre de rétention sont
//! **fournies par l'appelant**, aucun accès à l'horloge (porte `cargo xtask lint-clock`). La rétention
//! par défaut (30 j) est paramétrable côté serveur, jamais par le client (ADR 0013).

use chrono::{DateTime, Duration, Utc};
use wallos_req_macros::requirement;

/// Rétention par défaut des pierres tombales, en jours (ADR 0013). Valeur **paramétrable côté serveur**
/// (l'appelant fournit la fenêtre effective) ; ce défaut sûr évite qu'un appareil légèrement en retard
/// soit inutilement contraint à une resynchronisation complète.
pub const DEFAULT_TOMBSTONE_RETENTION_DAYS: i64 = 30;

/// Borne basse de rétention : les pierres tombales dont `deleted_at` est **strictement antérieur** à
/// cette date sont purgeables. `cutoff = now − retention`.
///
/// Fonction **pure** : `now` est fourni par l'appelant. Une rétention négative est ramenée à zéro
/// (jamais dans le futur — purgerait tout).
#[must_use]
#[requirement(REQ-SYN-002)]
pub fn retention_cutoff(now: DateTime<Utc>, retention_days: i64) -> DateTime<Utc> {
    now - Duration::days(retention_days.max(0))
}

/// Un curseur `since` est **périmé** — l'appareil doit se resynchroniser entièrement — s'il est
/// **strictement antérieur** à la borne de rétention (`since < now − retention`) : des pierres tombales
/// qu'il n'a jamais reçues ont pu être purgées, une synchronisation incrémentale serait silencieusement
/// incomplète (ADR 0013). Un `since` absent (première synchronisation) est également traité comme un
/// besoin de synchronisation complète par l'appelant.
///
/// Fonction **pure** : `now` et la fenêtre sont fournis (testable sans horloge, REQ-STA-008).
#[must_use]
#[requirement(REQ-SYN-002)]
pub fn requires_full_resync(since: DateTime<Utc>, now: DateTime<Utc>, retention_days: i64) -> bool {
    since < retention_cutoff(now, retention_days)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use wallos_req_macros::verifies;

    fn at(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    #[test]
    #[verifies(REQ-SYN-002, case = "borne de purge = maintenant − rétention")]
    fn cutoff_is_now_minus_retention() {
        assert_eq!(retention_cutoff(at(2026, 1, 31), 30), at(2026, 1, 1));
        // Rétention négative ramenée à zéro (borne = maintenant, ne purge pas le futur).
        assert_eq!(retention_cutoff(at(2026, 1, 31), -5), at(2026, 1, 31));
    }

    #[test]
    #[verifies(REQ-SYN-002, case = "curseur plus ancien que la rétention -> resync complet")]
    fn stale_cursor_forces_full_resync() {
        let now = at(2026, 2, 1);
        // 31 jours avant (> 30 j de rétention) : périmé.
        assert!(requires_full_resync(at(2026, 1, 1), now, 30));
    }

    #[test]
    #[verifies(REQ-SYN-002, case = "curseur dans la fenêtre -> synchronisation incrémentale")]
    fn recent_cursor_allows_incremental() {
        let now = at(2026, 2, 1);
        // 15 jours avant (< 30 j) : incrémental suffit.
        assert!(!requires_full_resync(at(2026, 1, 17), now, 30));
        // Exactement à la borne (now − 30 j) : non strictement antérieur -> pas de resync forcé.
        assert!(!requires_full_resync(retention_cutoff(now, 30), now, 30));
    }

    #[test]
    #[verifies(REQ-SYN-002, case = "la fenêtre de rétention est un paramètre (pas codée en dur)")]
    fn retention_window_is_a_parameter() {
        let now = at(2026, 2, 1);
        let since = at(2026, 1, 10); // 22 jours avant
        // Périmé sous une rétention de 7 j, valide sous 30 j : la fenêtre est bien injectée.
        assert!(requires_full_resync(since, now, 7));
        assert!(!requires_full_resync(since, now, 30));
    }
}
