//! Cycle de facturation (REQ-SUB-003).
//!
//! Un cycle est un couple **(unité, intervalle)**. Toute la logique d'échéance (REQ-SUB-012/013)
//! en dépend. Oracle `legacy` : les unités sont **capturées** sur l'application d'origine (table
//! `cycles` de Wallos — voir `e2e/fixtures/oracles/REQ-SUB-003-cycles.json`) — Daily/Weekly/Monthly/
//! Yearly ; « One-time » y figure comme un abonnement **non récurrent** (jour = 0), donc hors cycle.

use std::num::NonZeroU32;

use wallos_req_macros::requirement;

use crate::DomainError;

/// Unité de récurrence d'un cycle de facturation, capturée sur l'application d'origine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BillingUnit {
    /// Jour (Wallos `cycles.id = 1`, « Daily »).
    Day,
    /// Semaine (Wallos `cycles.id = 2`, « Weekly »).
    Week,
    /// Mois (Wallos `cycles.id = 3`, « Monthly »).
    Month,
    /// Année (Wallos `cycles.id = 4`, « Yearly »).
    Year,
}

impl BillingUnit {
    /// Code stable (minuscule) de l'unité, pour la sérialisation API et l'import/export.
    #[must_use]
    #[requirement(REQ-SUB-003)]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }

    /// Parse une unité depuis son code stable.
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si le code n'est pas l'un de `day`/`week`/`month`/`year`.
    #[requirement(REQ-SUB-003)]
    pub fn parse(code: &str) -> Result<Self, DomainError> {
        match code {
            "day" => Ok(Self::Day),
            "week" => Ok(Self::Week),
            "month" => Ok(Self::Month),
            "year" => Ok(Self::Year),
            other => Err(DomainError::InvalidArgument(format!(
                "unknown billing unit: {other}"
            ))),
        }
    }
}

/// Cycle de facturation = **(unité, intervalle strictement positif)**.
///
/// L'intervalle est un [`NonZeroU32`] : un intervalle **nul est impossible au niveau du type**
/// (REQ-SUB-003), et un intervalle négatif l'est par l'entier non signé — la validité est donc
/// portée par le type, pas vérifiée à l'exécution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BillingCycle {
    unit: BillingUnit,
    interval: NonZeroU32,
}

impl BillingCycle {
    /// Construit un cycle à partir d'un intervalle **déjà** non nul (validité portée par le type).
    #[must_use]
    #[requirement(REQ-SUB-003)]
    pub const fn new(unit: BillingUnit, interval: NonZeroU32) -> Self {
        Self { unit, interval }
    }

    /// Construit un cycle depuis un intervalle brut, en refusant zéro.
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si `interval == 0`.
    #[requirement(REQ-SUB-003)]
    pub fn from_parts(unit: BillingUnit, interval: u32) -> Result<Self, DomainError> {
        NonZeroU32::new(interval)
            .map(|interval| Self::new(unit, interval))
            .ok_or_else(|| DomainError::InvalidArgument("billing interval must be > 0".to_string()))
    }

    /// Unité de récurrence.
    #[must_use]
    #[requirement(REQ-SUB-003)]
    pub const fn unit(&self) -> BillingUnit {
        self.unit
    }

    /// Intervalle (nombre d'unités entre deux échéances), toujours strictement positif.
    #[must_use]
    #[requirement(REQ-SUB-003)]
    pub const fn interval(&self) -> u32 {
        self.interval.get()
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    fn nz(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).unwrap()
    }

    #[test]
    #[verifies(REQ-SUB-003, case = "couvre les unités de l'application d'origine")]
    fn covers_legacy_cycle_units() {
        // Oracle figé (Wallos, table `cycles`) : Daily/Weekly/Monthly/Yearly.
        // Cf. e2e/fixtures/oracles/REQ-SUB-003-cycles.json.
        for (code, unit) in [
            ("day", BillingUnit::Day),
            ("week", BillingUnit::Week),
            ("month", BillingUnit::Month),
            ("year", BillingUnit::Year),
        ] {
            assert_eq!(BillingUnit::parse(code).unwrap(), unit);
            assert_eq!(unit.as_str(), code);
        }
        assert!(BillingUnit::parse("fortnight").is_err());
    }

    #[test]
    #[verifies(REQ-SUB-003, case = "cycle = (unité, intervalle > 0)")]
    fn cycle_exposes_unit_and_interval() {
        let c = BillingCycle::new(BillingUnit::Month, nz(3));
        assert_eq!(c.unit(), BillingUnit::Month);
        assert_eq!(c.interval(), 3);
    }

    #[test]
    #[verifies(REQ-SUB-003, case = "intervalle nul refusé")]
    fn zero_interval_is_rejected() {
        assert!(BillingCycle::from_parts(BillingUnit::Week, 0).is_err());
        // Un intervalle positif passe, et l'unité est préservée.
        let c = BillingCycle::from_parts(BillingUnit::Week, 2).unwrap();
        assert_eq!(c.unit(), BillingUnit::Week);
        assert_eq!(c.interval(), 2);
    }
}
