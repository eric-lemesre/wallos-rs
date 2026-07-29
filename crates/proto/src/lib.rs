#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used))]
//! Types partagés et schémas API.

#![forbid(unsafe_code)]

use std::fmt;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use wallos_core::billing::{BillingCycle, BillingUnit};
use wallos_core::currencies::SupportedCurrency;
use wallos_core::money::{CurrencyCode, Money};
use wallos_core::{DomainError, Subscription, requirement};

/// Enveloppe un secret (mot de passe, jeton d'appareil, clé de canal de notification) — REQ-SEC-003.
///
/// La protection est **portée par le type**, pas par une liste de champs à maintenir : `Debug` et
/// `Display` masquent **toujours** la valeur (`[REDACTED]`), y compris au niveau de trace le plus
/// détaillé, si bien qu'un secret ne peut pas se retrouver dans un journal même par inadvertance.
/// La sérialisation reste **transparente** (`#[serde(transparent)]`) : la valeur circule normalement
/// sur le canal TLS (corps de requête/réponse), jamais dans les logs. Les champs sensibles des DTO
/// annotent `#[schema(value_type = String)]` pour conserver un schéma OpenAPI inchangé (chaîne).
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret<T = String>(T);

impl<T> Secret<T> {
    /// Enveloppe une valeur secrète.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Expose la valeur en clair — à n'appeler que pour l'usage métier (hachage, vérification,
    /// émission au client), jamais pour journaliser.
    #[requirement(REQ-SEC-003)]
    pub const fn expose_secret(&self) -> &T {
        &self.0
    }
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> fmt::Debug for Secret<T> {
    /// Masque toujours la valeur (REQ-SEC-003) : `Debug` est le format employé par `tracing`.
    #[requirement(REQ-SEC-003)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

impl<T> fmt::Display for Secret<T> {
    #[requirement(REQ-SEC-003)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Réponse d'état du serveur.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    /// Nom du service.
    pub service: String,
    /// Version semver.
    pub version: String,
    /// État.
    pub status: String,
}

/// Requête de création de compte (REQ-AUT-001).
///
/// Le mot de passe transite en clair sur le canal TLS puis est haché argon2id côté serveur ;
/// il n'est jamais stocké en clair.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateAccountRequest {
    /// Adresse e-mail du compte.
    #[schema(example = "user@example.com", format = "email")]
    pub email: String,
    /// Mot de passe (longueur minimale vérifiée côté serveur, REQ-AUT-003).
    #[schema(
        value_type = String,
        example = "correct horse battery staple",
        min_length = 12,
        format = "password"
    )]
    pub password: Secret<String>,
}

/// Requête d'authentification (REQ-AUT-002).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateSessionRequest {
    /// Adresse e-mail du compte.
    #[schema(example = "user@example.com", format = "email")]
    pub email: String,
    /// Mot de passe.
    #[schema(value_type = String, format = "password")]
    pub password: Secret<String>,
}

/// Représentation du compte authentifié courant (REQ-AUT-002).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CurrentUser {
    /// Adresse e-mail du compte courant.
    pub email: String,
}

/// Requête d'appairage d'un appareil natif (REQ-AUT-005).
///
/// Comme l'authentification web, mais émet un jeton propre à l'appareil (corps de réponse) au lieu
/// d'un cookie ; l'appareil fournit un libellé et sa plateforme.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateDeviceSessionRequest {
    /// Adresse e-mail du compte.
    #[schema(example = "user@example.com", format = "email")]
    pub email: String,
    /// Mot de passe.
    #[schema(value_type = String, format = "password")]
    pub password: Secret<String>,
    /// Libellé lisible de l'appareil (choisi par l'utilisateur ou dérivé du matériel).
    #[schema(example = "MacBook de Léa")]
    pub label: String,
    /// Plateforme de l'appareil.
    #[schema(example = "desktop")]
    pub platform: String,
}

/// Jeton d'appareil émis à l'appairage (REQ-AUT-005).
///
/// Renvoyé **une seule fois** : la coquille native le stocke via `PlatformAdapter.secureStore`,
/// jamais en clair côté serveur (seule son empreinte SHA-256 est conservée).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct DeviceToken {
    /// Jeton opaque à présenter en `Authorization: Bearer`.
    #[schema(value_type = String)]
    pub token: Secret<String>,
}

/// Requête de changement de mot de passe (REQ-AUT-007).
///
/// Exige le mot de passe actuel (sinon `403`) ; le nouveau doit respecter la politique (REQ-AUT-003).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ChangePasswordRequest {
    /// Mot de passe actuel, revérifié côté serveur.
    #[schema(value_type = String, format = "password")]
    pub current_password: Secret<String>,
    /// Nouveau mot de passe (longueur minimale + non compromis, REQ-AUT-003).
    #[schema(value_type = String, format = "password", min_length = 12)]
    pub new_password: Secret<String>,
}

/// Résumé d'un appareil appairé, pour la liste de gestion (REQ-AUT-006).
///
/// `id` (UUID) et `last_seen_at` (RFC 3339) sont sérialisés en chaînes pour rester indépendants des
/// features `utoipa`/`chrono` ; `current` distingue l'appareil à l'origine de la requête courante.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct DeviceSummary {
    /// Identifiant de l'appareil (UUID), clé de révocation.
    pub id: String,
    /// Libellé lisible.
    pub label: String,
    /// Plateforme de l'appareil.
    pub platform: String,
    /// Dernière activité (horodatage RFC 3339).
    pub last_seen_at: String,
    /// Vrai si cet appareil est celui qui a émis la requête courante.
    pub current: bool,
}

/// Requête de calcul de la prochaine échéance (REQ-SUB-012).
///
/// `first_payment` est l'**ancre** (date de premier paiement) ; `after` est la date de référence — la
/// réponse est la première échéance strictement postérieure. Dates en `YYYY-MM-DD`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NextDueRequest {
    /// Date de premier paiement (ancre), `YYYY-MM-DD`.
    #[schema(example = "2025-01-31")]
    pub first_payment: String,
    /// Cycle de facturation.
    pub cycle: BillingCycleDto,
    /// Date de référence : la prochaine échéance est strictement postérieure, `YYYY-MM-DD`.
    #[schema(example = "2025-01-31")]
    pub after: String,
}

/// Prochaine échéance calculée (REQ-SUB-012).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NextDueResponse {
    /// Prochaine échéance, `YYYY-MM-DD`.
    #[schema(example = "2025-02-28")]
    pub next_payment: String,
}

/// Une catégorie d'abonnements exposée à l'interface (REQ-CAT-001).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CategoryDto {
    /// Identifiant stable (UUID).
    pub id: String,
    /// Nom de la catégorie.
    #[schema(example = "Streaming")]
    pub name: String,
}

/// Requête de création d'une catégorie (REQ-CAT-001).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateCategoryRequest {
    /// Identifiant (UUID) **généré côté client** (offline-first, REQ-SYN-001), le cas échéant. Absent
    /// ⇒ le serveur en génère un. Présent ⇒ il est **conservé tel quel**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Nom de la catégorie (non vide, ≤ 100 caractères ; les espaces de bord sont normalisés).
    #[schema(example = "Streaming", max_length = 100)]
    pub name: String,
}

impl CreateCategoryRequest {
    /// Résout l'identifiant : l'UUID **fourni par le client** s'il est présent et valide (préservé,
    /// REQ-SYN-001), sinon un nouvel UUID généré par le serveur.
    ///
    /// # Errors
    /// [`FieldError`] sur le champ `id` si un `id` est fourni mais n'est pas un UUID valide
    /// (l'appelant traduit en `422`).
    #[requirement(REQ-SYN-001)]
    pub fn resolve_id(&self) -> Result<Uuid, FieldError> {
        resolve_client_id(self.id.as_deref())
            .map_err(|()| FieldError::new("id", "identifiant UUID invalide"))
    }
}

/// Résout l'identifiant d'une entité créée : l'UUID **fourni par le client** (offline-first) s'il est
/// présent et valide — alors **conservé** (REQ-SYN-001) —, sinon un nouvel UUID généré par le serveur.
///
/// # Errors
/// `()` si `raw` est fourni mais n'est pas un UUID valide.
#[requirement(REQ-SYN-001)]
fn resolve_client_id(raw: Option<&str>) -> Result<Uuid, ()> {
    match raw {
        Some(s) => Uuid::parse_str(s).map_err(|_| ()),
        None => Ok(Uuid::new_v4()),
    }
}

/// Requête de renommage d'une catégorie (REQ-CAT-001).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RenameCategoryRequest {
    /// Nouveau nom de la catégorie (non vide, ≤ 100 caractères ; espaces de bord normalisés).
    #[schema(example = "Musique", max_length = 100)]
    pub name: String,
}

/// Un moyen de paiement exposé à l'interface (REQ-SUB-011).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct PaymentMethodDto {
    /// Identifiant stable (UUID).
    pub id: String,
    /// Nom du moyen de paiement.
    #[schema(example = "Carte de crédit")]
    pub name: String,
}

/// Requête de création d'un moyen de paiement (REQ-SUB-011).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreatePaymentMethodRequest {
    /// Identifiant (UUID) **généré côté client** (offline-first, REQ-SYN-001), le cas échéant. Absent
    /// ⇒ le serveur en génère un. Présent ⇒ il est **conservé tel quel**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Nom (non vide, ≤ 100 caractères ; les espaces de bord sont normalisés).
    #[schema(example = "Carte de crédit", max_length = 100)]
    pub name: String,
}

impl CreatePaymentMethodRequest {
    /// Résout l'identifiant : l'UUID **fourni par le client** s'il est présent et valide (préservé,
    /// REQ-SYN-001), sinon un nouvel UUID généré par le serveur.
    ///
    /// # Errors
    /// [`FieldError`] sur le champ `id` si un `id` est fourni mais n'est pas un UUID valide
    /// (l'appelant traduit en `422`).
    #[requirement(REQ-SYN-001)]
    pub fn resolve_id(&self) -> Result<Uuid, FieldError> {
        resolve_client_id(self.id.as_deref())
            .map_err(|()| FieldError::new("id", "identifiant UUID invalide"))
    }
}

/// Requête de renommage d'un moyen de paiement (REQ-SUB-011).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RenamePaymentMethodRequest {
    /// Nouveau nom (non vide, ≤ 100 caractères ; espaces de bord normalisés).
    #[schema(example = "PayPal", max_length = 100)]
    pub name: String,
}

/// Une devise du référentiel supporté exposée à l'interface (REQ-CUR-007).
///
/// `name` est le libellé par défaut (anglais) ; l'UI affiche le nom **localisé** par code (i18n).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CurrencyDto {
    /// Code ISO 4217.
    #[schema(example = "EUR")]
    pub code: String,
    /// Symbole d'affichage.
    #[schema(example = "€")]
    pub symbol: String,
    /// Libellé par défaut (localisé par l'UI).
    #[schema(example = "Euro")]
    pub name: String,
    /// Nombre de décimales d'affichage (unités mineures ISO 4217).
    #[schema(example = 2)]
    pub decimals: u32,
}

impl CurrencyDto {
    /// Projette une devise du référentiel `core` vers le DTO.
    #[must_use]
    #[requirement(REQ-CUR-007)]
    pub fn from_core(currency: &SupportedCurrency) -> Self {
        Self {
            code: currency.code.to_string(),
            symbol: currency.symbol.to_string(),
            name: currency.name.to_string(),
            decimals: currency.decimals,
        }
    }
}

/// Devise de référence du foyer (REQ-CUR-001) : devise cible de tous les agrégats.
///
/// Sert à la fois de **réponse** (`GET`) et de **corps de requête** (`PUT`). Le code est validé contre
/// le référentiel supporté côté serveur ; les montants saisis ne sont jamais altérés.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ReferenceCurrencyDto {
    /// Code ISO 4217 de la devise de référence.
    #[schema(example = "EUR")]
    pub currency: String,
}

/// Langue de l'utilisateur (REQ-I18N-001), en **réponse** de lecture.
///
/// `language` est absent (`None`) si l'utilisateur n'a rien choisi : l'UI applique alors la langue
/// système si elle est supportée (acceptance #2).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct LanguageResponse {
    /// Code de langue (`en`, `fr`), ou absent si non renseigné.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "fr")]
    pub language: Option<String>,
}

/// Requête de choix de langue (REQ-I18N-001) : le code doit être une langue supportée.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SetLanguageRequest {
    /// Code de langue souhaité (`en`, `fr`).
    #[schema(example = "fr")]
    pub language: String,
}

/// Cycle de facturation d'un abonnement (REQ-SUB-003) : couple (unité, intervalle).
///
/// `unit` est un code stable (`day`/`week`/`month`/`year`) ; `interval` est le nombre d'unités entre
/// deux échéances, strictement positif. Modèle capturé sur l'application d'origine (table `cycles`).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct BillingCycleDto {
    /// Unité de récurrence : `day`, `week`, `month` ou `year`.
    #[schema(example = "month")]
    pub unit: String,
    /// Nombre d'unités entre deux échéances (strictement positif).
    #[schema(example = 1, minimum = 1)]
    pub interval: u32,
}

/// Représentation d'un abonnement dans le contrat API (REQ-SUB-001).
///
/// Reflet 1:1 du modèle [`Subscription`] du domaine : aucun champ n'est perdu ni normalisé
/// silencieusement. Montant en **chaîne** décimale (R4/CUR-002) ; identifiants et date en chaînes
/// (indépendants des features `utoipa`) ; cycle imbriqué (REQ-SUB-003).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SubscriptionDto {
    /// Identifiant stable (UUID).
    pub id: String,
    /// Nom de l'abonnement.
    pub name: String,
    /// Montant du prix (chaîne décimale).
    #[schema(example = "9.99")]
    pub amount: String,
    /// Code devise ISO 4217 du prix.
    #[schema(example = "EUR")]
    pub currency: String,
    /// Cycle de facturation.
    pub cycle: BillingCycleDto,
    /// Date de premier paiement (`YYYY-MM-DD`).
    #[schema(example = "2026-01-31")]
    pub first_payment: String,
    /// Catégorie rattachée (UUID), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Moyen de paiement rattaché (UUID), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    /// Payeur rattaché (UUID), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    /// Logo (référence d'image ou substitut), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    /// URL du service, le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Notes libres, le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// État actif (un abonnement désactivé est conservé mais exclu des agrégats, REQ-SUB-008).
    /// Absent à la désérialisation ⇒ actif par défaut, aligné sur le domaine (`Subscription::new`).
    #[serde(default = "default_active")]
    pub active: bool,
    /// Prochaine échéance calculée (`YYYY-MM-DD`, REQ-SUB-002/012), champ **dérivé** : présent dans les
    /// réponses de lecture/création, absent du modèle pur (calculé avec l'horloge serveur).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_payment: Option<String>,
    /// Date de fin (annulation programmée, `YYYY-MM-DD`, REQ-SUB-009), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Vrai si l'abonnement est **terminé** (date de fin dépassée) — champ **dérivé** (horloge serveur),
    /// REQ-SUB-009. Un abonnement terminé est conservé mais exclu des agrégats.
    #[serde(default)]
    pub ended: bool,
}

/// Valeur par défaut de `active` (aligne le DTO sur le défaut du domaine : actif).
fn default_active() -> bool {
    true
}

/// Parse un UUID optionnel (chaîne) ou renvoie une erreur de domaine.
fn parse_optional_uuid(value: Option<String>, field: &str) -> Result<Option<Uuid>, DomainError> {
    value
        .map(|v| {
            Uuid::parse_str(&v)
                .map_err(|_| DomainError::InvalidArgument(format!("invalid {field} id: {v}")))
        })
        .transpose()
}

impl SubscriptionDto {
    /// Projette un abonnement du domaine vers le DTO (sans perte).
    #[must_use]
    #[requirement(REQ-SUB-001)]
    pub fn from_core(sub: &Subscription) -> Self {
        Self {
            id: sub.id().to_string(),
            name: sub.name().to_string(),
            amount: sub.price().amount().to_string(),
            currency: sub.price().currency().as_str().to_string(),
            cycle: BillingCycleDto {
                unit: sub.cycle().unit().as_str().to_string(),
                interval: sub.cycle().interval(),
            },
            first_payment: sub.first_payment().to_string(),
            category: sub.category().map(|u| u.to_string()),
            payment_method: sub.payment_method().map(|u| u.to_string()),
            payer: sub.payer().map(|u| u.to_string()),
            logo: sub.logo().map(String::from),
            url: sub.url().map(String::from),
            notes: sub.notes().map(String::from),
            active: sub.is_active(),
            next_payment: None,
            end_date: sub.end_date().map(|d| d.to_string()),
            ended: false,
        }
    }

    /// Comme [`from_core`](Self::from_core), avec les champs **dérivés** (horloge serveur) : prochaine
    /// échéance éventuelle (absente si l'abonnement est terminé, REQ-SUB-009) et état « terminé ».
    #[must_use]
    #[requirement(REQ-SUB-009)]
    pub fn from_core_derived(
        sub: &Subscription,
        next_payment: Option<NaiveDate>,
        ended: bool,
    ) -> Self {
        let mut dto = Self::from_core(sub);
        dto.next_payment = next_payment.map(|d| d.to_string());
        dto.ended = ended;
        dto
    }

    /// Reconstruit un abonnement du domaine depuis le DTO, en validant chaque champ.
    ///
    /// # Errors
    /// [`DomainError`] si un identifiant, un montant, une devise, un cycle ou une date est invalide.
    #[requirement(REQ-SUB-001)]
    pub fn into_core(self) -> Result<Subscription, DomainError> {
        let id = Uuid::parse_str(&self.id).map_err(|_| {
            DomainError::InvalidArgument(format!("invalid subscription id: {}", self.id))
        })?;
        let amount = self
            .amount
            .parse::<Decimal>()
            .map_err(|_| DomainError::InvalidMoney(format!("invalid amount: {}", self.amount)))?;
        let currency = CurrencyCode::new(&self.currency)?;
        let price = Money::new(amount, currency)?;
        let unit = BillingUnit::parse(&self.cycle.unit)?;
        let cycle = BillingCycle::from_parts(unit, self.cycle.interval)?;
        let first_payment =
            NaiveDate::parse_from_str(&self.first_payment, "%Y-%m-%d").map_err(|_| {
                DomainError::InvalidDate(format!("invalid date: {}", self.first_payment))
            })?;

        let mut sub = Subscription::new(id, self.name, price, cycle, first_payment)?;
        if let Some(category) = parse_optional_uuid(self.category, "category")? {
            sub = sub.with_category(category);
        }
        if let Some(payment_method) = parse_optional_uuid(self.payment_method, "payment_method")? {
            sub = sub.with_payment_method(payment_method);
        }
        if let Some(payer) = parse_optional_uuid(self.payer, "payer")? {
            sub = sub.with_payer(payer);
        }
        if let Some(logo) = self.logo {
            sub = sub.with_logo(logo);
        }
        if let Some(url) = self.url {
            sub = sub.with_url(url);
        }
        if let Some(notes) = self.notes {
            sub = sub.with_notes(notes);
        }
        if let Some(end) = self.end_date {
            let end = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
                .map_err(|_| DomainError::InvalidDate(format!("invalid end date: {end}")))?;
            // La date de fin ne peut précéder le premier paiement (REQ-SUB-009 : période vide interdite).
            if end < first_payment {
                return Err(DomainError::InvalidDate(
                    "end date is before first payment".to_string(),
                ));
            }
            sub = sub.with_end_date(end);
        }
        Ok(sub.with_active(self.active))
    }
}

/// Filtres de la liste d'abonnements (REQ-SUB-006), passés en **query string**, tous optionnels et
/// **conjonctifs**. `category`/`payer` sont des UUID ; `active` restreint à l'état ; `currency` est la
/// devise cible du total agrégé (défaut : `EUR`, en attendant la devise de référence REQ-CUR-001).
#[derive(Debug, Clone, Default, Deserialize, Serialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SubscriptionListQuery {
    /// Restreint à une catégorie (UUID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Restreint à un payeur (UUID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    /// Restreint à l'état actif (`true`) ou inactif (`false`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Devise cible du total agrégé (code ISO 4217 ; défaut `EUR`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[param(example = "EUR")]
    pub currency: Option<String>,
}

/// Liste d'abonnements filtrée avec son **total agrégé** (REQ-SUB-006).
///
/// Le `total` reflète le filtre : il agrège les montants des abonnements **actifs** retournés,
/// convertis dans la devise cible (réutilise l'agrégat multi-devises en mode dégradé, REQ-CUR-004) —
/// un abonnement désactivé est exclu du total (REQ-SUB-008) même s'il figure dans la liste.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct SubscriptionListResponse {
    /// Abonnements du foyer correspondant au filtre (ordre stable : nom puis id).
    pub subscriptions: Vec<SubscriptionDto>,
    /// Total agrégé des abonnements actifs de la liste, dans la devise cible.
    pub total: ConvertedTotalResponse,
}

/// Requête de création d'un abonnement (REQ-SUB-002). Mêmes champs que [`SubscriptionDto`] sans `id`
/// (généré par le serveur) ni `next_payment` (dérivé). Montant en **chaîne** décimale (R4).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateSubscriptionRequest {
    /// Identifiant (UUID) **généré côté client** (offline-first, REQ-SYN-001), le cas échéant. Absent
    /// ⇒ le serveur en génère un. Présent ⇒ il est **conservé tel quel**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Nom de l'abonnement.
    pub name: String,
    /// Montant du prix (chaîne décimale).
    #[schema(example = "9.99")]
    pub amount: String,
    /// Code devise ISO 4217.
    #[schema(example = "EUR")]
    pub currency: String,
    /// Cycle de facturation.
    pub cycle: BillingCycleDto,
    /// Date de premier paiement (`YYYY-MM-DD`).
    #[schema(example = "2025-01-31")]
    pub first_payment: String,
    /// Catégorie rattachée (UUID), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Moyen de paiement rattaché (UUID), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    /// Payeur rattaché (UUID), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    /// Logo, le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    /// URL du service, le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Notes libres, le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// État actif (défaut : actif).
    #[serde(default = "default_active")]
    pub active: bool,
    /// Date de fin (annulation programmée, `YYYY-MM-DD`, REQ-SUB-009), le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
}

impl CreateSubscriptionRequest {
    /// Résout l'identifiant : l'UUID **fourni par le client** s'il est présent et valide (préservé,
    /// REQ-SYN-001), sinon un nouvel UUID généré par le serveur.
    ///
    /// # Errors
    /// [`FieldError`] sur le champ `id` si un `id` est fourni mais n'est pas un UUID valide.
    #[requirement(REQ-SYN-001)]
    pub fn resolve_id(&self) -> Result<Uuid, FieldError> {
        resolve_client_id(self.id.as_deref())
            .map_err(|()| FieldError::new("id", "identifiant UUID invalide"))
    }

    /// Construit l'abonnement du domaine avec l'`id` fourni, en **validant chaque champ**.
    ///
    /// # Errors
    /// [`FieldError`] identifiant le **champ fautif** (montant, devise, cycle, date, id de référence).
    #[requirement(REQ-SUB-002)]
    pub fn into_core(self, id: Uuid) -> Result<Subscription, FieldError> {
        let amount = self
            .amount
            .parse::<Decimal>()
            .map_err(|_| FieldError::new("amount", "montant décimal invalide"))?;
        let currency = CurrencyCode::new(&self.currency)
            .map_err(|_| FieldError::new("currency", "devise hors référentiel"))?;
        let price = Money::new(amount, currency)
            .map_err(|_| FieldError::new("amount", "montant négatif"))?;
        let unit = BillingUnit::parse(&self.cycle.unit)
            .map_err(|_| FieldError::new("cycle.unit", "unité inconnue"))?;
        // Borne l'intervalle à la plage stockable (`integer` SQL) : jamais de clamp silencieux en base.
        if i32::try_from(self.cycle.interval).is_err() {
            return Err(FieldError::new("cycle.interval", "intervalle trop grand"));
        }
        let cycle = BillingCycle::from_parts(unit, self.cycle.interval)
            .map_err(|_| FieldError::new("cycle.interval", "intervalle doit être > 0"))?;
        let first_payment = NaiveDate::parse_from_str(&self.first_payment, "%Y-%m-%d")
            .map_err(|_| FieldError::new("first_payment", "date YYYY-MM-DD invalide"))?;

        let mut sub = Subscription::new(id, self.name, price, cycle, first_payment)
            .map_err(|_| FieldError::new("name", "nom obligatoire"))?;
        if let Some(c) = parse_optional_uuid(self.category, "category")
            .map_err(|_| FieldError::new("category", "identifiant invalide"))?
        {
            sub = sub.with_category(c);
        }
        if let Some(p) = parse_optional_uuid(self.payment_method, "payment_method")
            .map_err(|_| FieldError::new("payment_method", "identifiant invalide"))?
        {
            sub = sub.with_payment_method(p);
        }
        if let Some(p) = parse_optional_uuid(self.payer, "payer")
            .map_err(|_| FieldError::new("payer", "identifiant invalide"))?
        {
            sub = sub.with_payer(p);
        }
        if let Some(logo) = self.logo {
            sub = sub.with_logo(logo);
        }
        if let Some(url) = self.url {
            sub = sub.with_url(url);
        }
        if let Some(notes) = self.notes {
            sub = sub.with_notes(notes);
        }
        if let Some(end) = self.end_date {
            let end = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
                .map_err(|_| FieldError::new("end_date", "date YYYY-MM-DD invalide"))?;
            // La date de fin ne peut précéder le premier paiement (revue SUB-009 : période vide interdite).
            if end < first_payment {
                return Err(FieldError::new(
                    "end_date",
                    "date de fin antérieure au premier paiement",
                ));
            }
            sub = sub.with_end_date(end);
        }
        Ok(sub.with_active(self.active))
    }
}

/// Erreur de validation **par champ** (REQ-SUB-002, critère #2) : identifie le champ fautif.
#[derive(Debug, Clone)]
pub struct FieldError {
    /// Nom du champ en défaut.
    pub field: &'static str,
    /// Message lisible.
    pub message: &'static str,
}

impl FieldError {
    /// Construit une erreur de champ.
    #[must_use]
    pub const fn new(field: &'static str, message: &'static str) -> Self {
        Self { field, message }
    }
}

/// Un montant en devise pour l'agrégation multi-devises (REQ-CUR-004).
///
/// `amount` est une **chaîne décimale** (règle R4 / REQ-CUR-002) : jamais un nombre JSON, qui
/// introduirait une imprécision flottante sur un montant.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct MoneyInput {
    /// Montant en chaîne décimale (ex. `"12.34"`).
    #[schema(example = "12.34")]
    pub amount: String,
    /// Code devise ISO 4217 (ex. `"EUR"`).
    #[schema(example = "EUR")]
    pub currency: String,
}

/// Requête d'agrégation multi-devises vers une devise cible (REQ-CUR-004).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AggregateRequest {
    /// Devise cible de l'agrégat (code ISO 4217).
    #[schema(example = "EUR")]
    pub target: String,
    /// Montants à convertir puis sommer.
    pub amounts: Vec<MoneyInput>,
}

/// Résultat d'une agrégation convertie, en **mode dégradé** explicite (REQ-CUR-004).
///
/// `total` est une chaîne décimale (R4). `complete` est faux dès qu'un montant a été **exclu** faute
/// de taux : un total incomplet n'est jamais présenté comme un zéro silencieux. `as_of` porte la
/// date de validité **la plus ancienne** parmi les taux utilisés — la fraîcheur à afficher quand le
/// fournisseur est indisponible et que l'on retombe sur le dernier taux connu.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ConvertedTotalResponse {
    /// Total converti dans la devise cible (chaîne décimale, précision exacte).
    #[schema(example = "142.50")]
    pub total: String,
    /// Devise du total (code ISO 4217).
    #[schema(example = "EUR")]
    pub currency: String,
    /// Nombre de montants effectivement convertis et inclus.
    pub converted: u32,
    /// Nombre de montants exclus faute de taux connu.
    pub excluded: u32,
    /// Vrai si tous les montants ont pu être convertis (agrégat complet).
    pub complete: bool,
    /// Date de validité la plus ancienne des taux utilisés (`YYYY-MM-DD`), ou absente si aucun taux
    /// daté n'a servi (ensemble vide ou conversions en devise identique uniquement).
    #[schema(example = "2026-07-20")]
    pub as_of: Option<String>,
}

/// Détail d'erreur conforme à la RFC 9457 (`application/problem+json`).
///
/// Schéma d'erreur unique de l'API : toute réponse d'erreur adopte ce format,
/// sans jamais divulguer de trace d'exécution, de requête SQL ni de chemin de
/// fichier (AGENTS.md §6, REQ-SEC-002).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Problem {
    /// URI de référence identifiant le type de problème.
    #[serde(rename = "type")]
    #[schema(example = "about:blank")]
    pub type_uri: String,
    /// Résumé court et lisible du type de problème, stable pour un `type` donné.
    pub title: String,
    /// Code de statut HTTP généré par l'origine.
    pub status: u16,
    /// Explication spécifique à cette occurrence du problème.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// URI de référence identifiant l'occurrence spécifique du problème.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

/// Construit un `Problem` RFC 9457 à partir d'un statut, d'un type et d'un titre.
#[requirement(REQ-SEC-002)]
pub fn problem(status: u16, type_uri: impl Into<String>, title: impl Into<String>) -> Problem {
    Problem {
        type_uri: type_uri.into(),
        title: title.into(),
        status,
        detail: None,
        instance: None,
    }
}

impl Problem {
    /// Renseigne l'URI d'occurrence (`instance`).
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    /// Renseigne le détail spécifique à l'occurrence (`detail`).
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wallos_core::verifies;

    #[test]
    #[verifies(REQ-SEC-002)]
    fn problem_serialises_rfc9457_fields() {
        let value = serde_json::to_value(
            problem(404, "about:blank", "Not Found").with_instance("/api/v1/x"),
        )
        .unwrap();

        // Le champ RFC est bien `type`, pas `type_uri`.
        assert_eq!(value["type"], "about:blank");
        assert_eq!(value["title"], "Not Found");
        assert_eq!(value["status"], 404);
        assert_eq!(value["instance"], "/api/v1/x");
        // `detail` absent est omis de la sérialisation.
        assert!(value.get("detail").is_none());
    }

    #[test]
    #[verifies(REQ-SEC-002)]
    fn problem_roundtrips_with_detail() {
        let original = problem(500, "about:blank", "Internal Server Error")
            .with_detail("boom")
            .with_instance("/api/v1/y");
        let json = serde_json::to_string(&original).unwrap();
        let parsed: Problem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, 500);
        assert_eq!(parsed.detail.as_deref(), Some("boom"));
    }

    #[test]
    #[verifies(REQ-CUR-002, case = "montant sérialisé en chaîne")]
    fn amounts_serialise_as_json_strings() {
        // Un montant en entrée transite en CHAÎNE décimale, jamais en nombre JSON.
        let input = serde_json::to_value(MoneyInput {
            amount: "12.34".to_string(),
            currency: "EUR".to_string(),
        })
        .unwrap();
        assert!(input["amount"].is_string());
        assert_eq!(input["amount"], "12.34");

        // Idem pour un total en sortie.
        let output = serde_json::to_value(ConvertedTotalResponse {
            total: "142.50".to_string(),
            currency: "EUR".to_string(),
            converted: 3,
            excluded: 0,
            complete: true,
            as_of: Some("2026-07-20".to_string()),
        })
        .unwrap();
        assert!(output["total"].is_string());
        assert_eq!(output["total"], "142.50");
    }

    fn sample_subscription() -> Subscription {
        Subscription::new(
            Uuid::from_u128(1),
            "Netflix",
            Money::new("9.99".parse().unwrap(), CurrencyCode::new("EUR").unwrap()).unwrap(),
            BillingCycle::from_parts(BillingUnit::Month, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
        )
        .unwrap()
    }

    #[test]
    #[verifies(REQ-SUB-001, case = "round-trip DTO sans perte de champ")]
    fn subscription_dto_roundtrips_without_loss() {
        // Abonnement doté de TOUS les champs (obligatoires + optionnels).
        let original = sample_subscription()
            .with_category(Uuid::from_u128(2))
            .with_payment_method(Uuid::from_u128(3))
            .with_payer(Uuid::from_u128(4))
            .with_logo("netflix.png")
            .with_url("https://netflix.com")
            .with_notes("compte partagé")
            .with_active(false);

        // core -> DTO -> JSON -> DTO -> core : identité (aucun champ perdu ni normalisé).
        let dto = SubscriptionDto::from_core(&original);
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["amount"], "9.99"); // montant en chaîne (jamais un nombre JSON)
        assert_eq!(json["cycle"]["unit"], "month");
        assert_eq!(json["active"], false);
        let parsed: SubscriptionDto = serde_json::from_value(json).unwrap();
        let restored = parsed.into_core().unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    #[verifies(REQ-SUB-001, case = "round-trip minimal (sans optionnels)")]
    fn minimal_subscription_dto_roundtrips() {
        let original = sample_subscription();
        let restored = SubscriptionDto::from_core(&original).into_core().unwrap();
        assert_eq!(restored, original);
        // Les champs optionnels absents sont omis du JSON (jamais matérialisés à null).
        let json = serde_json::to_value(SubscriptionDto::from_core(&original)).unwrap();
        assert!(json.get("category").is_none());
        assert!(json.get("notes").is_none());
    }

    #[test]
    #[verifies(REQ-SUB-001, case = "active absent -> défaut actif")]
    fn active_defaults_to_true_when_omitted() {
        // Un DTO sans `active` se désérialise en actif, aligné sur `Subscription::new`.
        let dto: SubscriptionDto = serde_json::from_value(serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "X",
            "amount": "1.00",
            "currency": "EUR",
            "cycle": { "unit": "month", "interval": 1 },
            "first_payment": "2026-01-31"
        }))
        .unwrap();
        assert!(dto.active);
        assert!(dto.into_core().unwrap().is_active());
    }

    #[test]
    #[verifies(REQ-SUB-001, case = "DTO invalide rejeté")]
    fn invalid_subscription_dto_is_rejected() {
        let mut dto = SubscriptionDto::from_core(&sample_subscription());
        dto.amount = "not-a-number".to_string();
        assert!(dto.clone().into_core().is_err());
        let mut bad_date = SubscriptionDto::from_core(&sample_subscription());
        bad_date.first_payment = "31/01/2026".to_string();
        assert!(bad_date.into_core().is_err());
    }

    fn create_req() -> CreateSubscriptionRequest {
        CreateSubscriptionRequest {
            id: None,
            name: "Netflix".to_string(),
            amount: "9.99".to_string(),
            currency: "EUR".to_string(),
            cycle: BillingCycleDto {
                unit: "month".to_string(),
                interval: 1,
            },
            first_payment: "2025-01-31".to_string(),
            category: None,
            payment_method: None,
            payer: None,
            logo: None,
            url: None,
            notes: None,
            active: true,
            end_date: None,
        }
    }

    #[test]
    #[verifies(REQ-SUB-002, case = "validation par champ : chaque champ fautif nommé")]
    fn create_request_reports_the_faulty_field() {
        let id = Uuid::from_u128(1);
        let err = |mutate: &dyn Fn(&mut CreateSubscriptionRequest)| {
            let mut r = create_req();
            mutate(&mut r);
            r.into_core(id).unwrap_err().field
        };
        assert_eq!(err(&|r| r.amount = "-1".to_string()), "amount");
        assert_eq!(err(&|r| r.amount = "abc".to_string()), "amount");
        assert_eq!(err(&|r| r.currency = "ZZZ".to_string()), "currency");
        assert_eq!(
            err(&|r| r.cycle.unit = "fortnight".to_string()),
            "cycle.unit"
        );
        assert_eq!(err(&|r| r.cycle.interval = 0), "cycle.interval");
        // CRIT-1 : intervalle hors plage stockable rejeté (jamais de clamp silencieux en base).
        assert_eq!(err(&|r| r.cycle.interval = u32::MAX), "cycle.interval");
        assert_eq!(
            err(&|r| r.first_payment = "31/01/2025".to_string()),
            "first_payment"
        );
        assert_eq!(err(&|r| r.name = "  ".to_string()), "name");
        assert_eq!(
            err(&|r| r.category = Some("not-a-uuid".to_string())),
            "category"
        );
        // Cas valide : construit sans erreur.
        assert!(create_req().into_core(id).is_ok());
    }

    #[test]
    #[verifies(REQ-SUB-003, case = "cycle sérialisé (unité + intervalle)")]
    fn billing_cycle_roundtrips() {
        let dto = BillingCycleDto {
            unit: "month".to_string(),
            interval: 3,
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["unit"], "month");
        assert_eq!(json["interval"], 3);
        let parsed: BillingCycleDto =
            serde_json::from_value(serde_json::json!({ "unit": "week", "interval": 2 })).unwrap();
        assert_eq!(parsed.unit, "week");
        assert_eq!(parsed.interval, 2);
    }

    #[test]
    #[verifies(REQ-SEC-003, case = "Debug/Display masquent le secret")]
    fn secret_is_redacted_in_debug_and_display() {
        let s = Secret::new("hunter2-super-secret".to_string());
        // `Debug` (le format utilisé par tracing) ne révèle jamais la valeur.
        let dbg = format!("{s:?}");
        assert_eq!(dbg, "Secret([REDACTED])");
        assert!(!dbg.contains("hunter2"));
        // `Display` non plus.
        assert_eq!(format!("{s}"), "[REDACTED]");
        // Un DTO entier journalisé (Debug) masque le champ secret sans masquer le reste.
        let req = CreateSessionRequest {
            email: "user@example.com".to_string(),
            password: Secret::new("hunter2-super-secret".to_string()),
        };
        let dbg = format!("{req:?}");
        assert!(dbg.contains("user@example.com"));
        assert!(!dbg.contains("hunter2"));
        assert!(dbg.contains("[REDACTED]"));
    }

    #[test]
    #[verifies(REQ-SEC-003, case = "serde reste transparent")]
    fn secret_serialises_transparently() {
        // Sur le canal TLS, la valeur circule normalement (jamais masquée à la sérialisation) :
        // le masquage ne concerne QUE les journaux (Debug/Display).
        let req = CreateSessionRequest {
            email: "user@example.com".to_string(),
            password: Secret::new("hunter2".to_string()),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["password"], "hunter2");

        // Et un secret se relit depuis une chaîne JSON (désérialisation transparente).
        let parsed: CreateSessionRequest =
            serde_json::from_value(serde_json::json!({ "email": "a@b.c", "password": "pw" }))
                .unwrap();
        assert_eq!(parsed.password.expose_secret(), "pw");
    }

    #[test]
    #[verifies(REQ-CUR-002, case = "un nombre JSON est refusé")]
    fn amount_as_json_number_is_rejected() {
        // Un nombre JSON pour un montant est refusé au parsing (le champ est une chaîne) : un montant
        // ne peut jamais voyager en nombre flottant, même côté désérialisation.
        let from_number: Result<MoneyInput, _> =
            serde_json::from_value(serde_json::json!({ "amount": 12.34, "currency": "EUR" }));
        assert!(from_number.is_err());

        // La forme correcte (chaîne) se relit sans perte.
        let from_string: MoneyInput =
            serde_json::from_value(serde_json::json!({ "amount": "12.34", "currency": "EUR" }))
                .unwrap();
        assert_eq!(from_string.amount, "12.34");
    }
}
