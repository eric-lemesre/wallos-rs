#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Couche d'envoi des notifications (REQ-NOT-005 et suivants).
//!
//! Abstraction **fermée** d'un canal d'envoi ([`Channel`], dispatch par `enum` plutôt que `dyn` —
//! l'ensemble des canaux est connu, cf. Wallos : webhook, e-mail, messageries) partageant une **charge
//! utile unique** ([`ReminderNotification`], critère NOT-004 « même trait d'envoi »). La première
//! implémentation est le **webhook générique** (POST JSON, REQ-NOT-005) ; l'e-mail (NOT-003) et les
//! messageries (NOT-004) s'y ajouteront comme variantes.
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

/// Canal d'envoi (ensemble **fermé**). Le webhook est la première variante ; e-mail/messageries
/// s'ajouteront ici sans changer le point d'appel (`Channel::send`).
#[derive(Debug, Clone)]
pub enum Channel {
    /// Webhook générique : POST JSON vers une URL configurée (REQ-NOT-005).
    Webhook(Webhook),
    /// Canal e-mail : envoi SMTP, dans la langue du compte (REQ-NOT-003).
    Email(Email),
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
        }
    }

    /// Étiquette stable du type de canal (pour les journaux).
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Webhook(_) => "webhook",
            Self::Email(_) => "email",
        }
    }
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
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client.post(&self.url).json(notification).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("statut HTTP inattendu: {}", response.status());
        }
        Ok(())
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
