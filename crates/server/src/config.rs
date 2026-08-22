//! Référence et validation de la configuration d'exécution (REQ-OPS-004).
//!
//! Le serveur lit des variables décisives pour la sécurité sans qu'aucune ne soit décrite hors du
//! code ; une configuration incomplète se manifestait tard, par un refus opaque en pleine
//! utilisation. Ce module porte la **référence** (chaque variable : rôle, caractère, défaut —
//! projetée dans `docs/configuration.md`, dont la synchronisation est testée) et la **validation
//! au démarrage** : les manques bloquants arrêtent le serveur en nommant la variable et l'attendu,
//! les manques tolérables produisent un avertissement énonçant la conséquence fonctionnelle.
//! Une valeur reçue n'est **jamais** restituée : elle pourrait fuiter dans un journal collecté.

use wallos_core::requirement;

use crate::listen;

/// Description d'une variable d'environnement lue par le serveur.
#[derive(Debug)]
pub struct VarSpec {
    /// Nom exact de la variable.
    pub name: &'static str,
    /// Rôle fonctionnel, tel que projeté dans la référence.
    pub role: &'static str,
    /// `true` si le serveur refuse de démarrer sans elle.
    pub required: bool,
    /// Valeur par défaut documentée (`None` pour une variable sans défaut).
    pub default: Option<&'static str>,
    /// `true` si la valeur est un secret : jamais journalisée, jamais restituée.
    pub secret: bool,
    /// Conséquence fonctionnelle si la variable est absente (variables tolérables).
    pub absence_consequence: Option<&'static str>,
    /// Contrainte de forme vérifiée au démarrage.
    pub expects: Expects,
}

/// Contrainte de forme d'une valeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expects {
    /// Chaîne libre non vide.
    NonEmpty,
    /// Adresse d'écoute `ip:port` (validée par [`listen::resolve_listen_addr`]).
    ListenAddr,
    /// Entier strictement positif.
    PositiveInt,
    /// Booléen `true` ou `false`.
    Bool,
}

/// Référence de configuration : chaque variable lue par le serveur y figure.
/// La porte `every_env_var_read_in_code_is_documented` échoue si une lecture lui échappe.
pub const CONFIG_REFERENCE: &[VarSpec] = &[
    VarSpec {
        name: "DATABASE_URL",
        role: "URL de connexion PostgreSQL (utilisateur, hôte, base)",
        required: true,
        default: None,
        secret: true,
        absence_consequence: None,
        expects: Expects::NonEmpty,
    },
    VarSpec {
        name: "LISTEN_ADDR",
        role: "Adresse et port d'écoute du serveur HTTP (REQ-OPS-002)",
        required: false,
        default: Some("127.0.0.1:3000"),
        secret: false,
        absence_consequence: None,
        expects: Expects::ListenAddr,
    },
    VarSpec {
        name: "WEBUI_DIR",
        role: "Répertoire de l'interface web compilée servie par le serveur (REQ-OPS-003)",
        required: false,
        default: None,
        secret: false,
        absence_consequence: Some("interface web non servie : API seule"),
        expects: Expects::NonEmpty,
    },
    VarSpec {
        name: "ENCRYPTION_KEY",
        role: "Clé de chiffrement au repos des secrets de canaux (REQ-SEC-004)",
        required: false,
        default: None,
        secret: true,
        absence_consequence: Some(
            "chiffrement au repos désactivé : la création d'un canal à secrets sera refusée (422)",
        ),
        expects: Expects::NonEmpty,
    },
    VarSpec {
        name: "CRON_TOKEN",
        role: "Secret d'opérateur autorisant le déclenchement du cron de rappels (REQ-NOT-001)",
        required: false,
        default: None,
        secret: true,
        absence_consequence: Some(
            "cron de rappels désactivé : aucune notification planifiée ne partira",
        ),
        expects: Expects::NonEmpty,
    },
    VarSpec {
        name: "SESSION_COOKIE_SECURE",
        role: "Attribut Secure du cookie de session (REQ-AUT-004) — `false` réservé aux tests locaux en HTTP",
        required: false,
        default: Some("true"),
        secret: false,
        absence_consequence: None,
        expects: Expects::Bool,
    },
    VarSpec {
        name: "SESSION_IDLE_TTL_MINUTES",
        role: "Durée d'inactivité (minutes) au-delà de laquelle une session est rejetée (REQ-AUT-004)",
        required: false,
        default: Some("30"),
        secret: false,
        absence_consequence: None,
        expects: Expects::PositiveInt,
    },
    VarSpec {
        name: "AUTH_RATELIMIT_MAX_ATTEMPTS",
        role: "Tentatives échouées, par compte ou par IP, avant limitation de l'authentification (REQ-AUT-008)",
        required: false,
        default: Some("5"),
        secret: false,
        absence_consequence: None,
        expects: Expects::PositiveInt,
    },
    VarSpec {
        name: "AUTH_RATELIMIT_WINDOW_SECONDS",
        role: "Largeur (secondes) de la fenêtre glissante de comptage des tentatives (REQ-AUT-008)",
        required: false,
        default: Some("900"),
        secret: false,
        absence_consequence: None,
        expects: Expects::PositiveInt,
    },
    VarSpec {
        name: "TOMBSTONE_RETENTION_DAYS",
        role: "Rétention (jours) des pierres tombales de synchronisation (REQ-SYN-004, ADR 0013)",
        required: false,
        default: Some("30"),
        secret: false,
        absence_consequence: None,
        expects: Expects::PositiveInt,
    },
];

/// Une variable obligatoire manque, ou une valeur bloquante est invalide.
/// Le message nomme la variable et l'attendu — jamais la valeur reçue.
#[derive(Debug, thiserror::Error)]
#[error("{name} : {expected}")]
pub struct ConfigError {
    name: &'static str,
    expected: String,
}

/// Bilan d'une validation réussie : avertissements à journaliser avant de servir.
#[derive(Debug, Default)]
pub struct StartupReport {
    /// Manques tolérables et valeurs facultatives ignorées, avec leur conséquence.
    pub warnings: Vec<String>,
}

/// Valide la configuration au démarrage, avant de servir la moindre requête.
///
/// `lookup` abstrait `std::env::var` pour rester testable sans environnement partagé.
///
/// # Errors
/// [`ConfigError`] si une variable obligatoire manque ou si une valeur bloquante (adresse
/// d'écoute) est invalide — le serveur ne doit pas démarrer.
#[requirement(REQ-OPS-004)]
pub fn validate_startup(
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<StartupReport, ConfigError> {
    let mut report = StartupReport::default();
    for spec in CONFIG_REFERENCE {
        let value = lookup(spec.name).filter(|v| !v.is_empty());
        match value {
            None => {
                if spec.required {
                    return Err(ConfigError {
                        name: spec.name,
                        expected: format!("variable obligatoire absente — attendu : {}", spec.role),
                    });
                }
                if let Some(consequence) = spec.absence_consequence {
                    report
                        .warnings
                        .push(format!("{} absente : {}", spec.name, consequence));
                }
            }
            Some(raw) => match spec.expects {
                Expects::NonEmpty => {}
                Expects::ListenAddr => {
                    if listen::resolve_listen_addr(Some(&raw)).is_err() {
                        return Err(ConfigError {
                            name: spec.name,
                            expected:
                                "une adresse d'écoute `ip:port` est attendue (ex. 0.0.0.0:3000)"
                                    .to_string(),
                        });
                    }
                }
                Expects::PositiveInt => {
                    if raw.parse::<i64>().ok().filter(|v| *v > 0).is_none() {
                        report.warnings.push(format!(
                            "{} inanalysable : entier strictement positif attendu — défaut {} appliqué",
                            spec.name,
                            spec.default.unwrap_or("interne")
                        ));
                    }
                }
                Expects::Bool => {
                    if raw != "true" && raw != "false" {
                        report.warnings.push(format!(
                            "{} inanalysable : `true` ou `false` attendu — défaut {} appliqué",
                            spec.name,
                            spec.default.unwrap_or("interne")
                        ));
                    }
                }
            },
        }
    }
    Ok(report)
}

/// Projette la référence en Markdown — le contenu exact de `docs/configuration.md`.
#[requirement(REQ-OPS-004)]
#[must_use]
pub fn reference_markdown() -> String {
    let mut out = String::from(
        "# Référence de configuration du serveur\n\n\
         > Généré depuis `crates/server/src/config.rs` (REQ-OPS-004) — ne pas éditer à la main.\n\
         > La synchronisation est vérifiée par test ; toute variable lue dans le code doit figurer ici.\n\n\
         | Variable | Rôle | Caractère | Défaut | Secret |\n\
         |----------|------|-----------|--------|--------|\n",
    );
    for spec in CONFIG_REFERENCE {
        let caractere = if spec.required {
            "Obligatoire"
        } else {
            "Facultative"
        };
        let defaut = spec.default.map_or("—".to_string(), |d| format!("`{d}`"));
        let secret = if spec.secret { "oui" } else { "non" };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            spec.name, spec.role, caractere, defaut, secret
        ));
    }
    out.push_str(
        "\nUne variable marquée **Secret** n'est jamais journalisée ni restituée dans une erreur :\n\
         seul son nom est cité. Une variable facultative absente dont l'absence a une conséquence\n\
         fonctionnelle est signalée par un avertissement au démarrage.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wallos_req_macros::verifies;

    use super::{CONFIG_REFERENCE, reference_markdown, validate_startup};

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "variable obligatoire absente : arrêt en nommant la variable")]
    fn missing_required_var_stops_startup() {
        let err = validate_startup(&env_of(&[])).expect_err("DATABASE_URL manque");
        let message = err.to_string();
        assert!(
            message.contains("DATABASE_URL"),
            "nomme la variable : {message}"
        );
        assert!(
            message.contains("URL de connexion PostgreSQL"),
            "énonce l'attendu : {message}"
        );
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "variable obligatoire vide : traitée comme absente")]
    fn empty_required_var_stops_startup() {
        let err = validate_startup(&env_of(&[("DATABASE_URL", "")])).expect_err("valeur vide");
        assert!(err.to_string().contains("DATABASE_URL"));
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "écoute invalide : arrêt sans restituer la valeur")]
    fn invalid_listen_addr_stops_startup_without_echo() {
        let err = validate_startup(&env_of(&[
            ("DATABASE_URL", "postgres://u:p@h/db"),
            ("LISTEN_ADDR", "valeur-confidentielle"),
        ]))
        .expect_err("LISTEN_ADDR invalide");
        let message = err.to_string();
        assert!(
            message.contains("LISTEN_ADDR"),
            "nomme la variable : {message}"
        );
        assert!(
            !message.contains("valeur-confidentielle"),
            "ne restitue jamais la valeur : {message}"
        );
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "configuration tolérable : avertissement avec la conséquence")]
    fn tolerable_gaps_warn_with_consequence() {
        let report = validate_startup(&env_of(&[("DATABASE_URL", "postgres://u:p@h/db")]))
            .expect("configuration minimale valide");
        let all = report.warnings.join("\n");
        assert!(
            all.contains("ENCRYPTION_KEY") && all.contains("refus"),
            "conséquence fonctionnelle énoncée pour la clé absente : {all}"
        );
        assert!(
            all.contains("CRON_TOKEN") && all.contains("désactivé"),
            "conséquence énoncée pour le cron absent : {all}"
        );
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "valeur facultative inanalysable : avertissement nommant la variable")]
    fn unparseable_optional_value_warns_without_echo() {
        let report = validate_startup(&env_of(&[
            ("DATABASE_URL", "postgres://u:p@h/db"),
            ("SESSION_IDLE_TTL_MINUTES", "trois-cents"),
        ]))
        .expect("le serveur poursuit sur le défaut documenté");
        let all = report.warnings.join("\n");
        assert!(
            all.contains("SESSION_IDLE_TTL_MINUTES"),
            "nomme la variable : {all}"
        );
        assert!(
            !all.contains("trois-cents"),
            "ne restitue pas la valeur : {all}"
        );
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "les secrets sont marqués et leur valeur n'apparaît nulle part")]
    fn secrets_are_flagged_in_reference() {
        for name in ["DATABASE_URL", "ENCRYPTION_KEY", "CRON_TOKEN"] {
            let spec = CONFIG_REFERENCE
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("{name} figure dans la référence"));
            assert!(spec.secret, "{name} est marquée secrète");
        }
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "référence : chaque variable a un rôle, un caractère et un défaut")]
    fn reference_documents_every_var() {
        let markdown = reference_markdown();
        for spec in CONFIG_REFERENCE {
            assert!(markdown.contains(spec.name), "{} documentée", spec.name);
            assert!(!spec.role.is_empty(), "{} a un rôle", spec.name);
        }
        assert!(
            markdown.contains("Obligatoire"),
            "caractère obligatoire/facultatif visible"
        );
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "porte : toute variable lue dans le code est décrite")]
    fn every_env_var_read_in_code_is_documented() {
        let crates_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("répertoire crates/");
        let mut read_names = std::collections::BTreeSet::new();
        for krate in ["server", "storage", "notifier"] {
            let src = crates_dir.join(krate).join("src");
            collect_env_var_literals(&src, &mut read_names);
        }
        let documented: std::collections::BTreeSet<&str> =
            CONFIG_REFERENCE.iter().map(|s| s.name).collect();
        let undocumented: Vec<_> = read_names
            .iter()
            .filter(|n| !documented.contains(n.as_str()))
            .collect();
        assert!(
            undocumented.is_empty(),
            "variables lues dans le code mais absentes de la référence : {undocumented:?}"
        );
        // La seule lecture non littérale (constante LISTEN_ADDR_VAR) est couverte.
        assert!(documented.contains("LISTEN_ADDR"));
    }

    fn collect_env_var_literals(
        dir: &std::path::Path,
        out: &mut std::collections::BTreeSet<String>,
    ) {
        let entries = std::fs::read_dir(dir).expect("lecture du répertoire source");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_env_var_literals(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let content = std::fs::read_to_string(&path).expect("lecture source");
                let mut rest = content.as_str();
                while let Some(idx) = rest.find("env::var(\"") {
                    rest = &rest[idx + "env::var(\"".len()..];
                    if let Some(end) = rest.find('"') {
                        out.insert(rest[..end].to_string());
                        rest = &rest[end..];
                    }
                }
            }
        }
    }

    #[test]
    #[verifies(REQ-OPS-004, case = "la référence committée est synchrone avec le code")]
    fn committed_reference_is_in_sync() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/configuration.md");
        let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "docs/configuration.md illisible ({e}) — générer :\n\n{}",
                reference_markdown()
            )
        });
        assert_eq!(
            committed,
            reference_markdown(),
            "docs/configuration.md a dérivé — contenu attendu :\n\n{}",
            reference_markdown()
        );
    }
}
