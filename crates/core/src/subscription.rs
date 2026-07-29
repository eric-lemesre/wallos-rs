//! Modèle de données d'un abonnement (REQ-SUB-001).
//!
//! Fonde le schéma, l'OpenAPI et le client TypeScript généré — **toute omission se propage partout**.
//! Le modèle cible est **capturé** sur l'application d'origine (table `subscriptions` de Wallos 5.4.2,
//! oracle figé) : nom, montant + devise (`price`/`currency_id`), cycle de facturation
//! (`cycle`/`frequency` → [`BillingCycle`], REQ-SUB-003), date de premier paiement, catégorie, moyen
//! de paiement, payeur, logo, URL, notes, état actif (`inactive`).
//!
//! Les références (catégorie / moyen de paiement / payeur) sont des [`Uuid`] optionnels : les entités
//! liées (CAT, référentiels) introduiront leurs propres types d'identifiant, que le modèle adoptera.
//! Le builder nomme chaque référence au site d'appel, ce qui évite d'intervertir deux identifiants.

use chrono::NaiveDate;
use uuid::Uuid;
use wallos_req_macros::requirement;

use crate::DomainError;
use crate::billing::BillingCycle;
use crate::money::Money;

/// Un abonnement suivi par un compte.
///
/// Construit par [`Subscription::new`] (champs obligatoires ; actif par défaut) puis affiné par les
/// méthodes `with_*` (champs optionnels). Aucun champ n'est normalisé silencieusement à la relecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    id: Uuid,
    name: String,
    price: Money,
    cycle: BillingCycle,
    first_payment: NaiveDate,
    category: Option<Uuid>,
    payment_method: Option<Uuid>,
    payer: Option<Uuid>,
    logo: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    active: bool,
    end_date: Option<NaiveDate>,
}

impl Subscription {
    /// Construit un abonnement à partir de ses champs **obligatoires** ; il est actif par défaut.
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si le `name` est vide (ou seulement des espaces) — le nom est
    /// obligatoire dans l'application d'origine (`name TEXT NOT NULL`).
    #[requirement(REQ-SUB-001)]
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        price: Money,
        cycle: BillingCycle,
        first_payment: NaiveDate,
    ) -> Result<Self, DomainError> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err(DomainError::InvalidArgument(
                "subscription name must not be empty".to_string(),
            ));
        }
        Ok(Self {
            id,
            name,
            price,
            cycle,
            first_payment,
            category: None,
            payment_method: None,
            payer: None,
            logo: None,
            url: None,
            notes: None,
            active: true,
            end_date: None,
        })
    }

    /// Fixe une **date de fin** (annulation programmée, REQ-SUB-009) : aucune échéance n'est produite
    /// au-delà de cette date, et l'abonnement est considéré « terminé » une fois la date dépassée.
    #[must_use]
    #[requirement(REQ-SUB-009)]
    pub const fn with_end_date(mut self, end_date: NaiveDate) -> Self {
        self.end_date = Some(end_date);
        self
    }

    /// Rattache une catégorie.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn with_category(mut self, category: Uuid) -> Self {
        self.category = Some(category);
        self
    }

    /// Rattache un moyen de paiement.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn with_payment_method(mut self, payment_method: Uuid) -> Self {
        self.payment_method = Some(payment_method);
        self
    }

    /// Rattache un payeur (membre du foyer).
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn with_payer(mut self, payer: Uuid) -> Self {
        self.payer = Some(payer);
        self
    }

    /// Renseigne le logo (référence d'image ou substitut).
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn with_logo(mut self, logo: impl Into<String>) -> Self {
        self.logo = Some(logo.into());
        self
    }

    /// Renseigne l'URL du service.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Renseigne des notes libres.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Fixe l'état actif (un abonnement désactivé est conservé mais exclu des agrégats, REQ-SUB-008).
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Identifiant stable de l'abonnement.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// Nom.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Montant et devise du prix.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub const fn price(&self) -> &Money {
        &self.price
    }

    /// Cycle de facturation.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub const fn cycle(&self) -> BillingCycle {
        self.cycle
    }

    /// Date de premier paiement.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub const fn first_payment(&self) -> NaiveDate {
        self.first_payment
    }

    /// Catégorie rattachée, le cas échéant.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub const fn category(&self) -> Option<Uuid> {
        self.category
    }

    /// Moyen de paiement rattaché, le cas échéant.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub const fn payment_method(&self) -> Option<Uuid> {
        self.payment_method
    }

    /// Payeur rattaché, le cas échéant.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub const fn payer(&self) -> Option<Uuid> {
        self.payer
    }

    /// Logo, le cas échéant.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn logo(&self) -> Option<&str> {
        self.logo.as_deref()
    }

    /// URL du service, le cas échéant.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Notes libres, le cas échéant.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    /// Vrai si l'abonnement est actif.
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Éligibilité aux rappels d'échéance (REQ-SUB-008).
    ///
    /// Un abonnement **désactivé ne déclenche aucun rappel** : il est conservé mais dormant.
    /// L'ordonnanceur de notifications (REQ-NOT-002) consomme ce prédicat ; le domaine ne connaît pas
    /// l'horloge (la décision « la fin approche » est prise ailleurs, avec la date injectée).
    #[must_use]
    #[requirement(REQ-SUB-008)]
    pub const fn emits_reminder(&self) -> bool {
        self.active
    }

    /// Date de fin (annulation programmée), le cas échéant.
    #[must_use]
    #[requirement(REQ-SUB-009)]
    pub const fn end_date(&self) -> Option<NaiveDate> {
        self.end_date
    }

    /// Vrai si l'abonnement est **terminé** à la date `reference` (date de fin **strictement** dépassée).
    ///
    /// Un abonnement terminé est conservé mais apparaît « terminé » et est exclu des agrégats (REQ-SUB-009).
    #[must_use]
    #[requirement(REQ-SUB-009)]
    pub fn has_ended(&self, reference: NaiveDate) -> bool {
        self.end_date.is_some_and(|end| reference > end)
    }

    /// Prochaine échéance après modification (REQ-SUB-004), strictement postérieure à `after`, et
    /// **bornée par la date de fin** (REQ-SUB-009) : aucune échéance postérieure à `end_date` n'est
    /// produite.
    ///
    /// **Recalculée à partir de la date de premier paiement et du cycle courant** (dérivée pure via
    /// [`next_due`](crate::schedule::next_due)) : après un changement de cycle ou de date de départ,
    /// l'échéance est **ré-ancrée** sur `first_payment`, jamais dérivée de l'ancienne échéance.
    /// `None` si le calcul déborde la plage représentable **ou** si la prochaine échéance tomberait
    /// après la date de fin.
    #[must_use]
    #[requirement(REQ-SUB-004)]
    pub fn next_due_after(&self, after: NaiveDate) -> Option<NaiveDate> {
        let occ = crate::schedule::next_due(self.first_payment, self.cycle, after)?;
        match self.end_date {
            Some(end) if occ > end => None, // aucune échéance au-delà de la date de fin (SUB-009)
            _ => Some(occ),
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use wallos_req_macros::verifies;

    use super::*;
    use crate::billing::BillingUnit;
    use crate::money::{CurrencyCode, Money};

    fn money(units: &str, code: &str) -> Money {
        Money::new(
            units.parse::<Decimal>().unwrap(),
            CurrencyCode::new(code).unwrap(),
        )
        .unwrap()
    }

    fn cycle() -> BillingCycle {
        BillingCycle::from_parts(BillingUnit::Month, 1).unwrap()
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
    }

    #[test]
    #[verifies(REQ-SUB-001, case = "champs obligatoires + défauts")]
    fn required_fields_and_defaults() {
        let id = Uuid::from_u128(1);
        let sub = Subscription::new(id, "Netflix", money("9.99", "EUR"), cycle(), day()).unwrap();
        assert_eq!(sub.id(), id);
        assert_eq!(sub.name(), "Netflix");
        assert_eq!(sub.price(), &money("9.99", "EUR"));
        assert_eq!(sub.cycle(), cycle());
        assert_eq!(sub.first_payment(), day());
        // Défauts : actif, aucune référence ni champ optionnel.
        assert!(sub.is_active());
        assert_eq!(sub.category(), None);
        assert_eq!(sub.payment_method(), None);
        assert_eq!(sub.payer(), None);
        assert_eq!(sub.logo(), None);
        assert_eq!(sub.url(), None);
        assert_eq!(sub.notes(), None);
    }

    #[test]
    #[verifies(REQ-SYN-001, case = "l'identifiant (UUID) fourni est conservé tel quel")]
    fn identity_uuid_is_preserved() {
        // Un UUID généré côté client (offline-first) est conservé à l'identique, jamais réécrit :
        // préalable à la réplication (REQ-SYN-001).
        let client_id = Uuid::from_u128(0xdead_beef);
        let sub =
            Subscription::new(client_id, "Netflix", money("9.99", "EUR"), cycle(), day()).unwrap();
        assert_eq!(sub.id(), client_id);
    }

    #[test]
    #[verifies(REQ-SUB-001, case = "nom vide refusé")]
    fn empty_name_is_rejected() {
        assert!(
            Subscription::new(Uuid::from_u128(1), "", money("1", "EUR"), cycle(), day()).is_err()
        );
        assert!(
            Subscription::new(Uuid::from_u128(1), "   ", money("1", "EUR"), cycle(), day())
                .is_err()
        );
    }

    #[test]
    #[verifies(REQ-SUB-001, case = "nom normalisé (trim)")]
    fn name_is_trimmed() {
        // Espaces de bord retirés avant stockage (cohérent avec les catégories).
        let sub = Subscription::new(
            Uuid::from_u128(1),
            "  Netflix  ",
            money("1", "EUR"),
            cycle(),
            day(),
        )
        .unwrap();
        assert_eq!(sub.name(), "Netflix");
    }

    #[test]
    #[verifies(REQ-SUB-004, case = "échéance recalculée depuis first_payment + nouveau cycle")]
    fn next_due_recomputed_from_first_payment_on_cycle_change() {
        let start = day();
        // Cycle initial mensuel : après le 2026-06-20, prochaine échéance mensuelle ancrée au 31.
        let monthly = Subscription::new(
            Uuid::from_u128(1),
            "Netflix",
            money("9.99", "EUR"),
            BillingCycle::from_parts(BillingUnit::Month, 1).unwrap(),
            start,
        )
        .unwrap();
        let after = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        assert_eq!(
            monthly.next_due_after(after),
            NaiveDate::from_ymd_opt(2026, 6, 30) // 31 juin -> clamp 30 (ancré sur le 31 de départ)
        );

        // Passage à un cycle ANNUEL, même first_payment : l'échéance est ré-ancrée sur le 31 janvier,
        // pas dérivée de l'ancienne échéance mensuelle.
        let yearly = Subscription::new(
            Uuid::from_u128(1),
            "Netflix",
            money("9.99", "EUR"),
            BillingCycle::from_parts(BillingUnit::Year, 1).unwrap(),
            start,
        )
        .unwrap();
        assert_eq!(
            yearly.next_due_after(after),
            NaiveDate::from_ymd_opt(2027, 1, 31)
        );
    }

    #[test]
    #[verifies(REQ-SUB-009, case = "aucune échéance au-delà de la date de fin")]
    fn no_due_date_after_end_date() {
        // Mensuel depuis le 2026-01-15, fin au 2026-03-31 : après le 2026-03-01 -> 2026-03-15 (<= fin, OK).
        let sub = Subscription::new(
            Uuid::from_u128(1),
            "N",
            money("1", "EUR"),
            BillingCycle::from_parts(BillingUnit::Month, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        )
        .unwrap()
        .with_end_date(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap());
        assert_eq!(
            sub.next_due_after(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
            NaiveDate::from_ymd_opt(2026, 3, 15)
        );
        // Après le 2026-03-15, la prochaine occurrence (2026-04-15) tombe APRÈS la fin -> aucune échéance.
        assert_eq!(
            sub.next_due_after(NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()),
            None
        );
    }

    #[test]
    #[verifies(REQ-SUB-009, case = "échéance tombant exactement sur la date de fin conservée")]
    fn due_date_exactly_on_end_date_is_kept() {
        // « aucune échéance POSTÉRIEURE à la date de fin » : une échéance le jour même de la fin est valide.
        let sub = Subscription::new(
            Uuid::from_u128(1),
            "N",
            money("1", "EUR"),
            BillingCycle::from_parts(BillingUnit::Month, 1).unwrap(),
            NaiveDate::from_ymd_opt(2030, 1, 15).unwrap(),
        )
        .unwrap()
        .with_end_date(NaiveDate::from_ymd_opt(2030, 2, 15).unwrap());
        // Après le 2030-01-20, la prochaine occurrence est le 2030-02-15, **égale** à la date de fin.
        assert_eq!(
            sub.next_due_after(NaiveDate::from_ymd_opt(2030, 1, 20).unwrap()),
            NaiveDate::from_ymd_opt(2030, 2, 15)
        );
    }

    #[test]
    #[verifies(REQ-SUB-009, case = "accesseur de date de fin")]
    fn end_date_accessor_returns_the_set_value() {
        let d = NaiveDate::from_ymd_opt(2030, 6, 30).unwrap();
        let sub = Subscription::new(Uuid::from_u128(1), "N", money("1", "EUR"), cycle(), day())
            .unwrap()
            .with_end_date(d);
        assert_eq!(sub.end_date(), Some(d));
        // Sans date de fin : None.
        assert_eq!(
            Subscription::new(Uuid::from_u128(2), "N", money("1", "EUR"), cycle(), day())
                .unwrap()
                .end_date(),
            None
        );
    }

    #[test]
    #[verifies(REQ-SUB-009, case = "terminé une fois la date de fin dépassée")]
    fn has_ended_after_end_date() {
        let sub = Subscription::new(Uuid::from_u128(1), "N", money("1", "EUR"), cycle(), day())
            .unwrap()
            .with_end_date(NaiveDate::from_ymd_opt(2026, 6, 30).unwrap());
        // Le jour de la fin : pas encore terminé ; le lendemain : terminé.
        assert!(!sub.has_ended(NaiveDate::from_ymd_opt(2026, 6, 30).unwrap()));
        assert!(sub.has_ended(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()));
        // Sans date de fin : jamais terminé.
        let perpetual =
            Subscription::new(Uuid::from_u128(2), "P", money("1", "EUR"), cycle(), day()).unwrap();
        assert!(!perpetual.has_ended(NaiveDate::from_ymd_opt(2999, 1, 1).unwrap()));
    }

    #[test]
    #[verifies(REQ-SUB-008, case = "abonnement désactivé n'émet aucun rappel")]
    fn inactive_subscription_emits_no_reminder() {
        let active =
            Subscription::new(Uuid::from_u128(1), "N", money("1", "EUR"), cycle(), day()).unwrap();
        assert!(active.emits_reminder());
        assert!(!active.with_active(false).emits_reminder());
    }

    #[test]
    #[verifies(REQ-SUB-004, case = "recalcul déterministe")]
    fn next_due_after_is_deterministic() {
        let sub = Subscription::new(
            Uuid::from_u128(1),
            "Netflix",
            money("9.99", "EUR"),
            BillingCycle::from_parts(BillingUnit::Month, 1).unwrap(),
            day(),
        )
        .unwrap();
        let after = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        assert_eq!(sub.next_due_after(after), sub.next_due_after(after));
        assert_eq!(
            sub.next_due_after(after),
            NaiveDate::from_ymd_opt(2026, 3, 31)
        );
    }

    #[test]
    #[verifies(REQ-SUB-001, case = "tous les champs optionnels préservés")]
    fn all_optional_fields_are_preserved() {
        let cat = Uuid::from_u128(2);
        let pm = Uuid::from_u128(3);
        let payer = Uuid::from_u128(4);
        let sub = Subscription::new(
            Uuid::from_u128(1),
            "Spotify",
            money("5.99", "USD"),
            cycle(),
            day(),
        )
        .unwrap()
        .with_category(cat)
        .with_payment_method(pm)
        .with_payer(payer)
        .with_logo("spotify.png")
        .with_url("https://spotify.com")
        .with_notes("famille")
        .with_active(false);
        assert_eq!(sub.category(), Some(cat));
        assert_eq!(sub.payment_method(), Some(pm));
        assert_eq!(sub.payer(), Some(payer));
        assert_eq!(sub.logo(), Some("spotify.png"));
        assert_eq!(sub.url(), Some("https://spotify.com"));
        assert_eq!(sub.notes(), Some("famille"));
        assert!(!sub.is_active());
    }
}
