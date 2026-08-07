#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Couche d'envoi des notifications (REQ-NOT-005 et suivants).
//!
//! Abstraction **fermée** d'un canal d'envoi ([`Channel`], dispatch par `enum` plutôt que `dyn` —
//! l'ensemble des canaux est connu, cf. Wallos : webhook, e-mail, messageries) partageant une **charge
//! utile unique** ([`ReminderNotification`], critère NOT-004 « même trait d'envoi ») : webhook
//! générique (POST JSON, REQ-NOT-005), e-mail SMTP (REQ-NOT-003), et messageries Telegram, Discord,
//! Gotify, Pushover (REQ-NOT-004 — même message texte localisé, seul le transport diffère).
//!
//! La validation **anti-SSRF** d'une URL de webhook ([`webhook_url_is_safe`]) est appliquée à
//! l'**enregistrement** (REQ-NOT-005 critère #2 ; socle de REQ-SEC-005) : refus des adresses de
//! bouclage, privées, link-local, non spécifiées, et du nom `localhost`. La résolution DNS d'un nom
//! d'hôte vers une IP privée (rebinding) relève de REQ-SEC-005, non traité ici.

#![forbid(unsafe_code)]

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::Serialize;

/// Un rappel individuel à notifier (élément de la charge utile).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReminderItem {
    /// Identifiant (UUID) de l'abonnement concerné.
    pub subscription_id: String,
    /// Nom de l'abonnement.
    pub name: String,
    /// Date d'échéance déclenchant le rappel (`YYYY-MM-DD`).
    pub due_date: String,
    /// Nombre de jours d'ici l'échéance.
    pub days_until: i64,
    /// Type de rappel : `payment` (échéance) ou `trial_ending` (fin d'essai).
    pub kind: String,
}

/// Charge utile de rappel groupée pour un compte — **commune à tous les canaux** (documentée dans
/// l'OpenAPI côté serveur, REQ-NOT-005). Sérialisée telle quelle en JSON par le webhook.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReminderNotification {
    /// Date de référence du balayage (`YYYY-MM-DD`).
    pub as_of: String,
    /// Nombre de rappels du lot.
    pub reminder_count: usize,
    /// Détail des abonnements concernés.
    pub reminders: Vec<ReminderItem>,
}

impl ReminderNotification {
    /// Compose une charge utile à partir de la date de référence et des rappels.
    #[must_use]
    pub fn new(as_of: impl Into<String>, reminders: Vec<ReminderItem>) -> Self {
        Self {
            as_of: as_of.into(),
            reminder_count: reminders.len(),
            reminders,
        }
    }
}

/// Canal d'envoi (ensemble **fermé**). Toutes les variantes partagent la même charge utile
/// ([`ReminderNotification`]) et le même point d'appel (`Channel::send`) — critère REQ-NOT-004
/// « même trait d'envoi, seul l'adaptateur diffère ».
#[derive(Debug, Clone)]
pub enum Channel {
    /// Webhook générique : POST JSON vers une URL configurée (REQ-NOT-005).
    Webhook(Webhook),
    /// Canal e-mail : envoi SMTP, dans la langue du compte (REQ-NOT-003).
    Email(Email),
    /// Messagerie Telegram : message texte via l'API Bot (REQ-NOT-004).
    Telegram(Telegram),
    /// Messagerie Discord : message texte via un webhook entrant (REQ-NOT-004).
    Discord(Discord),
    /// Serveur Gotify auto-hébergé : message texte via son API (REQ-NOT-004).
    Gotify(Gotify),
    /// Service Pushover : message texte via son API (REQ-NOT-004).
    Pushover(Pushover),
}

impl Channel {
    /// Envoie la charge utile sur ce canal (best-effort ; l'appelant journalise l'échec sans
    /// interrompre les autres canaux, esprit REQ-NOT-003).
    ///
    /// # Errors
    /// Erreur réseau, délai dépassé, ou statut non favorable (HTTP non 2xx / échec SMTP).
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        match self {
            Self::Webhook(w) => w.send(notification).await,
            Self::Email(e) => e.send(notification).await,
            Self::Telegram(t) => t.send(notification).await,
            Self::Discord(d) => d.send(notification).await,
            Self::Gotify(g) => g.send(notification).await,
            Self::Pushover(p) => p.send(notification).await,
        }
    }

    /// Étiquette stable du type de canal (pour les journaux).
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Webhook(_) => "webhook",
            Self::Email(_) => "email",
            Self::Telegram(_) => "telegram",
            Self::Discord(_) => "discord",
            Self::Gotify(_) => "gotify",
            Self::Pushover(_) => "pushover",
        }
    }
}

/// Client HTTP **durci** partagé par les canaux sortants : délai borné (10 s) et suivi de
/// redirection **désactivé** (anti-SSRF, même politique que le webhook NOT-005 — une `3xx`
/// est un échec).
///
/// # Errors
/// Échec de construction du client TLS.
fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

/// Statut HTTP non favorable renvoyé par la cible d'un canal. Erreur **typée** (downcastable depuis
/// `anyhow`) pour qu'un appelant — l'envoi de test REQ-NOT-006 — produise un diagnostic exploitable
/// (le code de statut) sans jamais refléter l'URL ni le corps de la réponse (secrets possibles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnexpectedStatus(pub u16);

impl fmt::Display for UnexpectedStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "statut HTTP inattendu: {}", self.0)
    }
}

impl std::error::Error for UnexpectedStatus {}

/// Vérifie qu'une réponse HTTP est un succès `2xx`, sinon [`UnexpectedStatus`] (jamais le corps —
/// il pourrait refléter des éléments de configuration).
fn ensure_success(response: &reqwest::Response) -> anyhow::Result<()> {
    if !response.status().is_success() {
        return Err(UnexpectedStatus(response.status().as_u16()).into());
    }
    Ok(())
}

/// Webhook générique (REQ-NOT-005) : POST de la charge utile JSON vers l'URL configurée.
#[derive(Debug, Clone)]
pub struct Webhook {
    url: String,
}

impl Webhook {
    /// Construit un webhook sur une URL. La sûreté de l'URL est vérifiée en amont
    /// ([`webhook_url_is_safe`]) à l'enregistrement — jamais ici.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// POST la charge utile JSON, délai borné (10 s). Échec si le statut n'est pas 2xx.
    ///
    /// **Anti-SSRF (REQ-NOT-005 / socle REQ-SEC-005)** : le suivi de redirection est **désactivé**.
    /// La garde `webhook_url_is_safe` ne valide que l'URL **initiale** à l'enregistrement ; sans cette
    /// politique, une redirection `3xx` pourrait pointer vers une adresse interne (bouclage, métadonnées
    /// d'instance) et reqwest la suivrait. Une réponse de redirection est donc traitée comme un échec.
    ///
    /// # Errors
    /// Construction du client, erreur réseau/délai, ou statut HTTP non 2xx (redirection incluse).
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        let response = http_client()?
            .post(&self.url)
            .json(notification)
            .send()
            .await?;
        ensure_success(&response)
    }
}

/// Configuration SMTP d'un canal e-mail (REQ-NOT-003). `Debug` **redacte** identifiant et mot de passe
/// (jamais journalisés, esprit du critère « échec journalisé sans exposer les identifiants »).
#[derive(Clone)]
pub struct EmailConfig {
    /// Hôte du serveur SMTP (nom d'hôte).
    pub host: String,
    /// Port SMTP (587 STARTTLS, 465 TLS implicite, 25 clair).
    pub port: u16,
    /// Identifiant d'authentification SMTP.
    pub username: String,
    /// Mot de passe / jeton SMTP.
    pub password: String,
    /// Adresse d'expéditeur (`From`).
    pub from: String,
    /// Utiliser STARTTLS (sinon TLS implicite).
    pub starttls: bool,
}

impl fmt::Debug for EmailConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmailConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &"<redacted>")
            .field("password", &"<redacted>")
            .field("from", &self.from)
            .field("starttls", &self.starttls)
            .finish()
    }
}

/// Canal e-mail (REQ-NOT-003) : connexion SMTP + **destinataire** et **langue** du compte (résolus au
/// moment de l'envoi, jamais figés dans la configuration du canal).
#[derive(Debug, Clone)]
pub struct Email {
    config: EmailConfig,
    recipient: String,
    language: String,
}

impl Email {
    /// Construit un canal e-mail pour un destinataire et une langue de compte donnés.
    #[must_use]
    pub fn new(
        config: EmailConfig,
        recipient: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            config,
            recipient: recipient.into(),
            language: language.into(),
        }
    }

    /// Compose et envoie l'e-mail de rappel via SMTP.
    ///
    /// # Errors
    /// Adresse invalide (`from`/`to`), configuration SMTP illisible, ou échec de connexion/envoi. Le
    /// message d'erreur ne contient jamais le mot de passe (voir [`EmailConfig`] `Debug`).
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        let message = compose_email(
            &self.language,
            &self.recipient,
            &self.config.from,
            notification,
        )?;
        let creds = Credentials::new(self.config.username.clone(), self.config.password.clone());
        let builder = if self.config.starttls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.host)?
        };
        let mailer = builder
            .port(self.config.port)
            .credentials(creds)
            .timeout(Some(Duration::from_secs(10)))
            .build();
        mailer.send(message).await?;
        Ok(())
    }
}

/// Messagerie Telegram (REQ-NOT-004) : envoi du message texte localisé via l'API Bot
/// (`POST /bot{token}/sendMessage`). `Debug` **redacte** le jeton du bot.
#[derive(Clone)]
pub struct Telegram {
    bot_token: String,
    chat_id: String,
    language: String,
    api_base: String,
}

impl fmt::Debug for Telegram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Telegram")
            .field("bot_token", &"<redacted>")
            .field("chat_id", &self.chat_id)
            .field("language", &self.language)
            .field("api_base", &self.api_base)
            .finish()
    }
}

impl Telegram {
    /// Construit un canal Telegram pour un bot, une conversation et une langue de compte donnés.
    #[must_use]
    pub fn new(
        bot_token: impl Into<String>,
        chat_id: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            bot_token: bot_token.into(),
            chat_id: chat_id.into(),
            language: language.into(),
            api_base: "https://api.telegram.org".to_string(),
        }
    }

    /// Remplace la base de l'API (tests uniquement — l'API publique de Telegram est fixe et ce
    /// champ n'est **pas** exposé à l'enregistrement d'un canal).
    #[must_use]
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    /// POST le message texte localisé à l'API Bot (`{chat_id, text}`, oracle legacy).
    ///
    /// # Errors
    /// Construction du client, erreur réseau/délai, ou statut HTTP non 2xx.
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        let url = format!("{}/bot{}/sendMessage", self.api_base, self.bot_token);
        let body = serde_json::json!({
            "chat_id": self.chat_id,
            "text": message_text(&self.language, notification),
        });
        let response = http_client()?.post(url).json(&body).send().await?;
        ensure_success(&response)
    }
}

/// Messagerie Discord (REQ-NOT-004) : envoi du message texte localisé à un webhook entrant
/// (`{content, username?, avatar_url?}`, oracle legacy). L'URL est validée anti-SSRF à
/// l'enregistrement ([`webhook_url_is_safe`]), comme le webhook générique. `Debug` **redacte**
/// l'URL : un webhook Discord embarque son jeton dans le chemin (revue NOT-004 F1).
#[derive(Clone)]
pub struct Discord {
    url: String,
    username: Option<String>,
    avatar_url: Option<String>,
    language: String,
}

impl fmt::Debug for Discord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Discord")
            .field("url", &"<redacted>")
            .field("username", &self.username)
            .field("avatar_url", &self.avatar_url)
            .field("language", &self.language)
            .finish()
    }
}

impl Discord {
    /// Construit un canal Discord (nom et avatar du bot optionnels, repris du legacy).
    #[must_use]
    pub fn new(
        url: impl Into<String>,
        username: Option<String>,
        avatar_url: Option<String>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            username,
            avatar_url,
            language: language.into(),
        }
    }

    /// POST le message texte localisé au webhook Discord.
    ///
    /// # Errors
    /// Construction du client, erreur réseau/délai, ou statut HTTP non 2xx.
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        // `allowed_mentions` vide : un nom d'abonnement contenant `@everyone`/`@here` ne doit
        // jamais déclencher de mention massive sur le serveur cible (revue NOT-004 F5).
        let mut body = serde_json::json!({
            "content": message_text(&self.language, notification),
            "allowed_mentions": { "parse": [] },
        });
        if let Some(username) = &self.username {
            body["username"] = serde_json::Value::String(username.clone());
        }
        if let Some(avatar_url) = &self.avatar_url {
            body["avatar_url"] = serde_json::Value::String(avatar_url.clone());
        }
        let response = http_client()?.post(&self.url).json(&body).send().await?;
        ensure_success(&response)
    }
}

/// Serveur Gotify auto-hébergé (REQ-NOT-004) : envoi du message texte localisé à
/// `POST {url}/message` (`{message, priority}`, oracle legacy). Le jeton d'application passe par
/// l'en-tête `X-Gotify-Key` (jamais dans l'URL — il fuiterait dans les journaux d'accès).
/// `Debug` **redacte** le jeton. L'URL du serveur est validée anti-SSRF à l'enregistrement.
#[derive(Clone)]
pub struct Gotify {
    url: String,
    token: String,
    language: String,
}

impl fmt::Debug for Gotify {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gotify")
            .field("url", &self.url)
            .field("token", &"<redacted>")
            .field("language", &self.language)
            .finish()
    }
}

impl Gotify {
    /// Construit un canal Gotify pour un serveur, un jeton d'application et une langue donnés.
    #[must_use]
    pub fn new(
        url: impl Into<String>,
        token: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            token: token.into(),
            language: language.into(),
        }
    }

    /// POST le message texte localisé au serveur Gotify (priorité 5, valeur legacy).
    ///
    /// # Errors
    /// Construction du client, erreur réseau/délai, ou statut HTTP non 2xx.
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        let url = format!("{}/message", self.url.trim_end_matches('/'));
        let body = serde_json::json!({
            "message": message_text(&self.language, notification),
            "priority": 5,
        });
        let response = http_client()?
            .post(url)
            .header("X-Gotify-Key", &self.token)
            .json(&body)
            .send()
            .await?;
        ensure_success(&response)
    }
}

/// Service Pushover (REQ-NOT-004) : envoi du message texte localisé à
/// `POST /1/messages.json` (formulaire `token`/`user`/`message`, oracle legacy).
/// `Debug` **redacte** le jeton d'application et la clé utilisateur.
#[derive(Clone)]
pub struct Pushover {
    user_key: String,
    token: String,
    language: String,
    api_base: String,
}

impl fmt::Debug for Pushover {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pushover")
            .field("user_key", &"<redacted>")
            .field("token", &"<redacted>")
            .field("language", &self.language)
            .field("api_base", &self.api_base)
            .finish()
    }
}

impl Pushover {
    /// Construit un canal Pushover pour une clé utilisateur, un jeton d'application et une langue.
    #[must_use]
    pub fn new(
        user_key: impl Into<String>,
        token: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            user_key: user_key.into(),
            token: token.into(),
            language: language.into(),
            api_base: "https://api.pushover.net".to_string(),
        }
    }

    /// Remplace la base de l'API (tests uniquement — l'API publique de Pushover est fixe et ce
    /// champ n'est **pas** exposé à l'enregistrement d'un canal).
    #[must_use]
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    /// POST le message texte localisé à l'API Pushover (formulaire URL-encodé, oracle legacy).
    ///
    /// # Errors
    /// Construction du client, erreur réseau/délai, ou statut HTTP non 2xx.
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        let url = format!("{}/1/messages.json", self.api_base);
        let form = [
            ("token", self.token.as_str()),
            ("user", self.user_key.as_str()),
            ("message", &message_text(&self.language, notification)),
        ];
        let response = http_client()?.post(url).form(&form).send().await?;
        ensure_success(&response)
    }
}

/// Vrai si la chaîne est une URL `http(s)` analysable. Validation **de forme** seulement (pas de
/// garde SSRF) : sert aux URLs transmises à un tiers sans être contactées par nos soins — l'avatar
/// de bot Discord (revue NOT-004 F8).
#[must_use]
pub fn is_http_url(raw: &str) -> bool {
    reqwest::Url::parse(raw)
        .map(|u| matches!(u.scheme(), "http" | "https"))
        .unwrap_or(false)
}

/// Vrai si le jeton a le format d'un jeton de bot Telegram (`<id numérique>:<suffixe [A-Za-z0-9_-]>`).
/// Validé à l'enregistrement (REQ-NOT-004) : le jeton est interpolé dans le **chemin** de l'URL de
/// l'API Bot — un caractère hors format (`/`, `?`, `#`, espace) altérerait la requête émise
/// (revue NOT-004 F2).
#[must_use]
pub fn telegram_bot_token_is_valid(token: &str) -> bool {
    let Some((id, suffix)) = token.split_once(':') else {
        return false;
    };
    !id.is_empty()
        && id.bytes().all(|b| b.is_ascii_digit())
        && !suffix.is_empty()
        && suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Classe l'échec d'un envoi en **code de diagnostic stable** + statut HTTP éventuel
/// (REQ-NOT-006 : « diagnostic exploitable en cas d'échec »). Ne reflète **jamais** le texte brut
/// de l'erreur : il peut contenir l'URL cible (donc un jeton pour Telegram) ou des détails SMTP.
///
/// Codes : `http-status` (statut non 2xx, avec le code), `timeout`, `connection-failed`,
/// `smtp-failed`, `send-failed` (défaut).
#[must_use]
pub fn diagnose_send_error(err: &anyhow::Error) -> (&'static str, Option<u16>) {
    if let Some(UnexpectedStatus(status)) = err.downcast_ref::<UnexpectedStatus>() {
        return ("http-status", Some(*status));
    }
    if let Some(e) = err.downcast_ref::<reqwest::Error>() {
        if e.is_timeout() {
            return ("timeout", None);
        }
        if e.is_connect() {
            return ("connection-failed", None);
        }
        return ("send-failed", None);
    }
    if err
        .downcast_ref::<lettre::transport::smtp::Error>()
        .is_some()
        || err.downcast_ref::<lettre::error::Error>().is_some()
    {
        return ("smtp-failed", None);
    }
    ("send-failed", None)
}

/// Message texte localisé d'un lot de rappels, **commun aux canaux de messagerie** (REQ-NOT-004 :
/// les adaptateurs ne diffèrent que par le transport, jamais par le contenu). Réutilise les
/// gabarits de l'e-mail ([`email_content`]) : corps seul, le sujet n'a pas d'équivalent en
/// messagerie. Fonction **pure** ; langue inconnue → repli anglais (REQ-I18N-004).
#[must_use]
pub fn message_text(language: &str, notification: &ReminderNotification) -> String {
    email_content(language, notification).1
}

/// Contenu textuel (sujet, corps) d'un e-mail de rappel **dans la langue du compte** (REQ-NOT-003),
/// avec le détail des abonnements concernés (nom, échéance, jours restants ; ligne distincte pour une
/// fin d'essai). Fonction **pure** (testable sans SMTP) ; `language` non reconnue → repli anglais
/// (cohérent avec REQ-I18N-004).
#[must_use]
pub fn email_content(language: &str, notification: &ReminderNotification) -> (String, String) {
    let french = language.eq_ignore_ascii_case("fr");
    let subject = if french {
        "Rappel : échéances d'abonnement à venir"
    } else {
        "Reminder: upcoming subscription payments"
    };
    let intro = if french {
        "Vous avez des échéances d'abonnement à venir :"
    } else {
        "You have upcoming subscription payments:"
    };
    let mut body = String::new();
    body.push_str(intro);
    body.push_str("\n\n");
    for item in &notification.reminders {
        let is_trial = item.kind == "trial_ending";
        let line = match (french, is_trial) {
            (true, true) => format!(
                "- {} : fin de la période d'essai le {} (dans {} jour(s))\n",
                item.name, item.due_date, item.days_until
            ),
            (true, false) => format!(
                "- {} : échéance le {} (dans {} jour(s))\n",
                item.name, item.due_date, item.days_until
            ),
            (false, true) => format!(
                "- {}: free trial ends on {} (in {} day(s))\n",
                item.name, item.due_date, item.days_until
            ),
            (false, false) => format!(
                "- {}: due on {} (in {} day(s))\n",
                item.name, item.due_date, item.days_until
            ),
        };
        body.push_str(&line);
    }
    (subject.to_string(), body)
}

/// Vrai si `addr` est une adresse e-mail analysable (boîte aux lettres SMTP valide, REQ-NOT-003).
/// Permet au serveur de valider une adresse d'expéditeur sans dépendre directement de `lettre`.
#[must_use]
pub fn is_valid_email_address(addr: &str) -> bool {
    addr.parse::<lettre::message::Mailbox>().is_ok()
}

/// Compose le message e-mail de rappel (REQ-NOT-003) : localise via [`email_content`] puis construit le
/// message SMTP (adresses `from`/`to`, corps texte).
///
/// # Errors
/// Adresse `from` ou `to` non analysable, ou corps invalide.
pub fn compose_email(
    language: &str,
    recipient: &str,
    from: &str,
    notification: &ReminderNotification,
) -> anyhow::Result<Message> {
    let (subject, body) = email_content(language, notification);
    let message = Message::builder()
        .from(from.parse()?)
        .to(recipient.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)?;
    Ok(message)
}

/// Vrai si l'URL de webhook est **sûre** à enregistrer (prévention SSRF, REQ-NOT-005 critère #2) :
/// schéma `http`/`https`, et hôte **public** — ni bouclage, ni privé (RFC 1918 / ULA), ni link-local,
/// ni non spécifié, ni CGNAT, ni le nom `localhost`. Un nom d'hôte DNS non réservé est accepté (sa
/// résolution vers une IP interne relève de REQ-SEC-005).
#[must_use]
pub fn webhook_url_is_safe(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    // Normalise une éventuelle IPv6 entre crochets selon la version de l'analyseur.
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return false;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip_is_public(ip);
    }
    // Nom d'hôte non-IP, non réservé : considéré public à l'enregistrement.
    true
}

/// Vrai si l'IP est routable publiquement (aucune plage interne/réservée).
fn ip_is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_is_public(v4),
        IpAddr::V6(v6) => v6_is_public(v6),
    }
}

/// Plages IPv4 refusées : bouclage, privées, link-local, non spécifiée, broadcast, documentation,
/// « ce réseau » `0.0.0.0/8`, et CGNAT `100.64.0.0/10`.
fn v4_is_public(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    let is_cgnat = o[0] == 100 && (0x40..=0x7f).contains(&o[1]);
    let is_this_network = o[0] == 0;
    !(ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_documentation()
        || is_cgnat
        || is_this_network)
}

/// Plages IPv6 refusées : bouclage, non spécifiée, unique-local `fc00::/7`, link-local `fe80::/10`,
/// et IPv4 mappée pointant vers une IPv4 interne (`::ffff:a.b.c.d`).
fn v6_is_public(ip: Ipv6Addr) -> bool {
    let seg0 = ip.segments()[0];
    let is_unique_local = (seg0 & 0xfe00) == 0xfc00;
    let is_link_local = (seg0 & 0xffc0) == 0xfe80;
    if let Some(v4) = ip.to_ipv4_mapped() {
        return v4_is_public(v4);
    }
    !(ip.is_loopback() || ip.is_unspecified() || is_unique_local || is_link_local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_urls_are_accepted() {
        assert!(webhook_url_is_safe("https://hooks.example.com/abc"));
        assert!(webhook_url_is_safe("http://93.184.216.34/notify")); // IPv4 publique (example.com)
        assert!(webhook_url_is_safe(
            "https://[2606:2800:220:1:248:1893:25c8:1946]/x"
        ));
    }

    #[test]
    fn loopback_and_localhost_are_rejected() {
        assert!(!webhook_url_is_safe("http://localhost/hook"));
        assert!(!webhook_url_is_safe("http://LOCALHOST:8080/hook"));
        assert!(!webhook_url_is_safe("http://sub.localhost/hook"));
        assert!(!webhook_url_is_safe("http://127.0.0.1/hook"));
        assert!(!webhook_url_is_safe("http://127.15.2.3:3000/hook"));
        assert!(!webhook_url_is_safe("http://[::1]/hook"));
    }

    #[test]
    fn private_and_reserved_ranges_are_rejected() {
        assert!(!webhook_url_is_safe("http://10.0.0.5/hook")); // RFC 1918
        assert!(!webhook_url_is_safe("http://192.168.1.1/hook"));
        assert!(!webhook_url_is_safe("http://172.16.4.4/hook"));
        assert!(!webhook_url_is_safe("http://169.254.169.254/latest")); // link-local (métadonnées cloud)
        assert!(!webhook_url_is_safe("http://100.100.0.1/hook")); // CGNAT
        assert!(!webhook_url_is_safe("http://0.0.0.0/hook"));
        assert!(!webhook_url_is_safe("http://[fc00::1]/hook")); // ULA
        assert!(!webhook_url_is_safe("http://[fe80::1]/hook")); // link-local v6
        assert!(!webhook_url_is_safe("http://[::ffff:10.0.0.1]/hook")); // v4 mappée privée
    }

    #[test]
    fn ipv6_zone_identifiers_are_rejected() {
        // Revue NOT-004 F3 : un identifiant de zone (`%eth0`) empêcherait le parse en `IpAddr` et
        // ferait passer une adresse link-local pour un nom d'hôte public. Le parseur d'URL (whatwg)
        // rejette ces hôtes ; ce test fige ce comportement contre une régression de dépendance.
        assert!(!webhook_url_is_safe("http://[fe80::1%25eth0]/hook"));
        assert!(!webhook_url_is_safe("http://[fe80::1%eth0]/hook"));
        assert!(!webhook_url_is_safe("http://fe80::1%25eth0/hook"));
    }

    #[test]
    fn non_http_schemes_and_garbage_are_rejected() {
        assert!(!webhook_url_is_safe("ftp://example.com/x"));
        assert!(!webhook_url_is_safe("file:///etc/passwd"));
        assert!(!webhook_url_is_safe("pas une url"));
        assert!(!webhook_url_is_safe(""));
    }

    fn sample_notification() -> ReminderNotification {
        ReminderNotification::new(
            "2026-08-06",
            vec![
                ReminderItem {
                    subscription_id: "s1".into(),
                    name: "Netflix".into(),
                    due_date: "2026-08-07".into(),
                    days_until: 1,
                    kind: "payment".into(),
                },
                ReminderItem {
                    subscription_id: "s2".into(),
                    name: "Figma".into(),
                    due_date: "2026-08-08".into(),
                    days_until: 2,
                    kind: "trial_ending".into(),
                },
            ],
        )
    }

    #[test]
    fn email_content_french_localizes_subject_and_body() {
        let (subject, body) = email_content("fr", &sample_notification());
        assert_eq!(subject, "Rappel : échéances d'abonnement à venir");
        assert!(body.contains("Vous avez des échéances d'abonnement à venir :"));
        assert!(body.contains("Netflix : échéance le 2026-08-07 (dans 1 jour(s))"));
        // La fin d'essai a une formulation distincte.
        assert!(body.contains("Figma : fin de la période d'essai le 2026-08-08 (dans 2 jour(s))"));
    }

    #[test]
    fn email_content_english_and_unknown_language_fallback() {
        let (subject, body) = email_content("en", &sample_notification());
        assert_eq!(subject, "Reminder: upcoming subscription payments");
        assert!(body.contains("Netflix: due on 2026-08-07 (in 1 day(s))"));
        assert!(body.contains("Figma: free trial ends on 2026-08-08 (in 2 day(s))"));
        // Langue inconnue -> repli anglais (REQ-I18N-004).
        let (unknown_subject, _) = email_content("xx", &sample_notification());
        assert_eq!(unknown_subject, "Reminder: upcoming subscription payments");
    }

    #[test]
    fn compose_email_builds_message_with_addresses() {
        let msg = compose_email(
            "en",
            "user@example.com",
            "wallos@example.com",
            &sample_notification(),
        )
        .unwrap();
        let out = String::from_utf8(msg.formatted()).unwrap();
        assert!(out.contains("To: user@example.com"));
        assert!(out.contains("From: wallos@example.com"));
    }

    #[test]
    fn compose_email_rejects_invalid_address() {
        let err = compose_email(
            "en",
            "pas-une-adresse",
            "w@example.com",
            &sample_notification(),
        );
        assert!(err.is_err());
    }

    #[test]
    fn email_config_debug_redacts_secrets() {
        let config = EmailConfig {
            host: "smtp.example.com".into(),
            port: 587,
            username: "alice".into(),
            password: "s3cr3t".into(),
            from: "wallos@example.com".into(),
            starttls: true,
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("s3cr3t"));
        assert!(!debug.contains("alice"));
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("smtp.example.com"));
    }

    #[test]
    fn message_text_is_localized_email_body() {
        let n = sample_notification();
        let fr = message_text("fr", &n);
        assert!(fr.contains("Vous avez des échéances d'abonnement à venir :"));
        assert!(fr.contains("Netflix : échéance le 2026-08-07 (dans 1 jour(s))"));
        // Langue inconnue -> repli anglais (REQ-I18N-004), identique au corps d'e-mail.
        assert_eq!(message_text("xx", &n), email_content("en", &n).1);
    }

    #[test]
    fn telegram_debug_redacts_bot_token() {
        let t = Telegram::new("123:s3cr3t-token", "42", "fr");
        let debug = format!("{t:?}");
        assert!(!debug.contains("s3cr3t-token"));
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("42")); // chat_id non secret, conservé pour le diagnostic
    }

    #[test]
    fn gotify_debug_redacts_token() {
        let g = Gotify::new("https://gotify.example.com", "app-s3cr3t", "en");
        let debug = format!("{g:?}");
        assert!(!debug.contains("app-s3cr3t"));
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("gotify.example.com"));
    }

    #[test]
    fn pushover_debug_redacts_user_key_and_token() {
        let p = Pushover::new("uk-s3cr3t", "tok-s3cr3t", "en");
        let debug = format!("{p:?}");
        assert!(!debug.contains("uk-s3cr3t"));
        assert!(!debug.contains("tok-s3cr3t"));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn discord_debug_redacts_webhook_url() {
        // Revue NOT-004 F1 : l'URL d'un webhook Discord porte son jeton dans le chemin.
        let d = Discord::new(
            "https://discord.com/api/webhooks/1/s3cr3t-token",
            Some("Wallos".into()),
            None,
            "en",
        );
        let debug = format!("{d:?}");
        assert!(!debug.contains("s3cr3t-token"));
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("Wallos"));
    }

    #[test]
    fn telegram_bot_tokens_are_validated_strictly() {
        // Revue NOT-004 F2 : le jeton est interpolé dans le chemin de l'URL de l'API Bot.
        assert!(telegram_bot_token_is_valid("123456:AAH-abc_XYZ09"));
        assert!(!telegram_bot_token_is_valid("123456"));
        assert!(!telegram_bot_token_is_valid(":abc"));
        assert!(!telegram_bot_token_is_valid("123:"));
        assert!(!telegram_bot_token_is_valid("abc:def"));
        assert!(!telegram_bot_token_is_valid("123:abc/def"));
        assert!(!telegram_bot_token_is_valid("123:abc?x=1"));
        assert!(!telegram_bot_token_is_valid("123:abc def"));
    }

    #[test]
    fn http_url_form_is_validated() {
        // Revue NOT-004 F8 : validation de forme (pas de garde SSRF — l'URL n'est pas contactée).
        assert!(is_http_url("https://cdn.example.com/avatar.png"));
        assert!(is_http_url("http://cdn.example.com/a"));
        assert!(!is_http_url("javascript:alert(1)"));
        assert!(!is_http_url("file:///etc/passwd"));
        assert!(!is_http_url("pas une url"));
    }

    #[test]
    fn channel_kinds_are_stable_labels() {
        let channels = [
            (
                Channel::Webhook(Webhook::new("https://x.example.com")),
                "webhook",
            ),
            (Channel::Telegram(Telegram::new("t", "c", "en")), "telegram"),
            (
                Channel::Discord(Discord::new("https://x.example.com", None, None, "en")),
                "discord",
            ),
            (
                Channel::Gotify(Gotify::new("https://x.example.com", "t", "en")),
                "gotify",
            ),
            (Channel::Pushover(Pushover::new("u", "t", "en")), "pushover"),
        ];
        for (channel, expected) in channels {
            assert_eq!(channel.kind(), expected);
        }
    }

    #[test]
    fn notification_counts_its_items() {
        let n = ReminderNotification::new(
            "2026-08-06",
            vec![ReminderItem {
                subscription_id: "s1".into(),
                name: "Netflix".into(),
                due_date: "2026-08-07".into(),
                days_until: 1,
                kind: "payment".into(),
            }],
        );
        assert_eq!(n.reminder_count, 1);
        // La charge utile sérialise en JSON avec les champs documentés.
        let json = serde_json::to_value(&n).unwrap();
        assert_eq!(json["reminder_count"], 1);
        assert_eq!(json["reminders"][0]["kind"], "payment");
    }
}
