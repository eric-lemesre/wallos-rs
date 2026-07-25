//! Garde-fou d'isolation : le contexte d'appelant `Actor`.
//!
//! REQ-SEC-001 — « Isolation stricte des données entre comptes ». AGENTS.md §9 impose que tout
//! repository requière un contexte d'appelant, afin qu'aucune requête ne puisse omettre la clause
//! de propriétaire. Ce type est le prérequis d'ADR 0006, à définir **avant** toute implémentation
//! `storage`.
//!
//! Conformément à ADR 0012, l'unité de propriété et d'isolation est le **foyer** (`household`),
//! pas le compte individuel : l'`Actor` porte donc à la fois l'identité de l'utilisateur et son
//! foyer. Les repositories filtrent sur `household_id`.

use uuid::Uuid;
use wallos_req_macros::requirement;

/// Contexte d'appelant authentifié.
///
/// Type opaque : ses composants ne sont accessibles qu'en lecture, jamais construits par erreur
/// sans les deux identifiants. Passé en premier argument de toute méthode de repository (ADR 0006).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actor {
    user_id: Uuid,
    household_id: Uuid,
}

impl Actor {
    /// Construit un contexte d'appelant à partir de l'utilisateur et de son foyer.
    #[must_use]
    #[requirement(REQ-SEC-001)]
    pub const fn new(user_id: Uuid, household_id: Uuid) -> Self {
        Self {
            user_id,
            household_id,
        }
    }

    /// Identifiant de l'utilisateur authentifié.
    #[must_use]
    #[requirement(REQ-SEC-001)]
    pub const fn user_id(&self) -> Uuid {
        self.user_id
    }

    /// Identifiant du foyer de l'utilisateur — clé d'isolation des données (ADR 0012).
    #[must_use]
    #[requirement(REQ-SEC-001)]
    pub const fn household_id(&self) -> Uuid {
        self.household_id
    }

    /// Indique si cet acteur appartient au foyer propriétaire d'une donnée.
    ///
    /// Point de décision unique de l'isolation : un accès dont le foyer ne correspond pas doit être
    /// traité comme *inexistant* (`404`), jamais comme *interdit* (`403`), afin de ne pas divulguer
    /// l'existence de la ressource (AGENTS.md §9).
    #[must_use]
    #[requirement(REQ-SEC-001)]
    pub fn owns(&self, resource_household_id: Uuid) -> bool {
        self.household_id == resource_household_id
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    fn actor_in(household: Uuid) -> Actor {
        Actor::new(Uuid::from_u128(1), household)
    }

    #[test]
    #[verifies(REQ-SEC-001)]
    fn exposes_user_and_household() {
        let user = Uuid::from_u128(7);
        let household = Uuid::from_u128(42);
        let actor = Actor::new(user, household);
        assert_eq!(actor.user_id(), user);
        assert_eq!(actor.household_id(), household);
    }

    #[test]
    #[verifies(REQ-SEC-001)]
    fn owns_only_its_own_household() {
        let household = Uuid::from_u128(42);
        let actor = actor_in(household);
        assert!(actor.owns(household));
    }

    #[test]
    #[verifies(REQ-SEC-001)]
    fn does_not_own_a_foreign_household() {
        let actor = actor_in(Uuid::from_u128(42));
        assert!(!actor.owns(Uuid::from_u128(43)));
    }
}
