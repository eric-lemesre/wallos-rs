//! Résolution de l'adresse d'écoute du serveur (REQ-OPS-002).
//!
//! Le serveur écoutait en dur sur la boucle locale : injoignable en conteneur ou derrière un
//! reverse-proxy sur une autre interface. L'adresse et le port viennent désormais de la variable
//! d'environnement [`LISTEN_ADDR_VAR`], la boucle locale port 3000 restant le défaut. Une valeur
//! syntaxiquement invalide arrête le démarrage en nommant la variable — jamais la valeur reçue,
//! qui pourrait fuiter dans un journal collecté.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use wallos_core::requirement;

/// Nom de la variable d'environnement portant l'adresse d'écoute (`ip:port`).
pub const LISTEN_ADDR_VAR: &str = "LISTEN_ADDR";

/// Écoute par défaut : boucle locale IPv4, port 3000 — le comportement historique.
const DEFAULT_LISTEN: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);

/// La valeur de [`LISTEN_ADDR_VAR`] n'est pas une adresse d'écoute.
///
/// Le message nomme la variable et l'attendu ; il ne restitue jamais la valeur reçue, qui
/// finirait sinon dans un journal collecté.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "{LISTEN_ADDR_VAR} invalide : une adresse d'écoute `ip:port` est attendue (ex. 0.0.0.0:3000)"
)]
pub struct ListenAddrError;

/// Résout l'adresse d'écoute effective depuis la valeur brute de [`LISTEN_ADDR_VAR`].
///
/// `None` (variable absente) rend le défaut ; une valeur présente doit être une adresse
/// `ip:port` valide, sans repli silencieux.
#[requirement(REQ-OPS-002)]
pub fn resolve_listen_addr(raw: Option<&str>) -> Result<SocketAddr, ListenAddrError> {
    match raw {
        None => Ok(DEFAULT_LISTEN),
        Some(value) => value.parse().map_err(|_| ListenAddrError),
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::{LISTEN_ADDR_VAR, resolve_listen_addr};

    #[test]
    #[verifies(REQ-OPS-002, case = "défaut boucle locale port 3000")]
    fn default_is_loopback_3000() {
        let addr = resolve_listen_addr(None).expect("le défaut est toujours valide");
        assert_eq!(addr.to_string(), "127.0.0.1:3000");
    }

    #[test]
    #[verifies(REQ-OPS-002, case = "adresse et port fournis par l'environnement")]
    fn env_value_overrides_default() {
        let addr = resolve_listen_addr(Some("0.0.0.0:8080")).expect("adresse valide");
        assert_eq!(addr.to_string(), "0.0.0.0:8080");
    }

    #[test]
    #[verifies(REQ-OPS-002, case = "adresse IPv6 fournie par l'environnement")]
    fn env_value_accepts_ipv6() {
        let addr = resolve_listen_addr(Some("[::1]:9000")).expect("adresse IPv6 valide");
        assert_eq!(addr.to_string(), "[::1]:9000");
    }

    #[test]
    #[verifies(REQ-OPS-002, case = "valeur invalide : arrêt en nommant la variable")]
    fn invalid_value_names_the_variable() {
        let err = resolve_listen_addr(Some("pas-une-adresse")).expect_err("valeur invalide");
        let message = err.to_string();
        assert!(
            message.contains(LISTEN_ADDR_VAR),
            "le message nomme la variable : {message}"
        );
    }

    #[test]
    #[verifies(REQ-OPS-002, case = "valeur invalide : la valeur reçue n'est pas restituée")]
    fn invalid_value_is_never_echoed() {
        let err = resolve_listen_addr(Some("secret-interne:99999")).expect_err("valeur invalide");
        let message = err.to_string();
        assert!(
            !message.contains("secret-interne"),
            "la valeur ne fuit pas : {message}"
        );
    }

    #[test]
    #[verifies(REQ-OPS-002, case = "valeur vide : invalide, pas de repli silencieux")]
    fn empty_value_is_invalid() {
        resolve_listen_addr(Some("")).expect_err("une valeur vide n'est pas une adresse");
    }

    #[test]
    #[verifies(REQ-OPS-002, case = "port seul sans adresse : invalide")]
    fn port_alone_is_invalid() {
        resolve_listen_addr(Some("8080")).expect_err("un port seul n'est pas une adresse");
    }
}
