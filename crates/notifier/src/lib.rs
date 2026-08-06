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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

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
}

impl Channel {
    /// Envoie la charge utile sur ce canal (best-effort ; l'appelant journalise l'échec sans
    /// interrompre les autres canaux, esprit REQ-NOT-003).
    ///
    /// # Errors
    /// Erreur réseau, délai dépassé, ou statut HTTP non 2xx pour le webhook.
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        match self {
            Self::Webhook(w) => w.send(notification).await,
        }
    }

    /// Étiquette stable du type de canal (pour les journaux).
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Webhook(_) => "webhook",
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
    /// # Errors
    /// Construction du client, erreur réseau/délai, ou statut HTTP non 2xx.
    pub async fn send(&self, notification: &ReminderNotification) -> anyhow::Result<()> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let response = client.post(&self.url).json(notification).send().await?;
        response.error_for_status()?;
        Ok(())
    }
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
        assert!(webhook_url_is_safe("https://[2606:2800:220:1:248:1893:25c8:1946]/x"));
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
