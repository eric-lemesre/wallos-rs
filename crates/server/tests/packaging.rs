//! Tests de garde de la recette d'empaquetage (REQ-OPS-010).
//!
//! La construction effective des paquets vit dans `packaging/` (recette nfpm, unité systemd,
//! configuration par défaut, scripts) et s'exécute en CI de release. Ces tests empêchent la
//! recette de dériver des critères d'acceptation : ils lisent les fichiers committés et vérifient
//! les invariants qui, cassés, produiraient un paquet installable mais non conforme.

use std::path::PathBuf;

use wallos_req_macros::verifies;

fn packaging_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packaging")
}

fn read(name: &str) -> String {
    let path = packaging_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} illisible : {e}", path.display()))
}

#[test]
#[verifies(REQ-OPS-010, case = "la recette pose binaire, interface, unité et configuration aux emplacements standards")]
fn recipe_ships_binary_ui_unit_and_config() {
    let recipe = read("nfpm.yaml");
    for expected in [
        "dst: /usr/bin/wallos-server",
        "dst: /usr/share/wallos-server/webui",
        "dst: /usr/lib/systemd/system/wallos-server.service",
        "dst: /etc/wallos-server/wallos-server.env",
    ] {
        assert!(recipe.contains(expected), "la recette déclare {expected}");
    }
}

#[test]
#[verifies(REQ-OPS-010, case = "la configuration porteuse de secrets n'est lisible que par le compte de service")]
fn config_is_group_readable_only() {
    let recipe = read("nfpm.yaml");
    assert!(
        recipe.contains("mode: 0o640") || recipe.contains("mode: 0640"),
        "config en 0640 (root:wallos-server), jamais lisible du monde"
    );
    assert!(
        recipe.contains("group: wallos-server"),
        "le groupe du fichier de configuration est le compte de service"
    );
}

#[test]
#[verifies(REQ-OPS-010, case = "aucun secret prédéfini dans la configuration livrée")]
fn default_config_carries_no_secret() {
    let config = read("config/wallos-server.env");
    for line in config.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let var = line.split('=').next().unwrap_or_default();
        assert!(
            !["ENCRYPTION_KEY", "CRON_TOKEN", "DATABASE_URL"].contains(&var),
            "aucune valeur de secret prédéfinie : `{line}` doit rester en commentaire"
        );
    }
}

#[test]
#[verifies(REQ-OPS-010, case = "la configuration de l'exploitant survit aux mises à jour")]
fn config_is_preserved_on_upgrade() {
    let recipe = read("nfpm.yaml");
    assert!(
        recipe.contains("config|noreplace"),
        "type config|noreplace : la modification de l'exploitant est préservée"
    );
}

#[test]
#[verifies(REQ-OPS-010, case = "le service s'exécute sous un compte système dédié, non privilégié")]
fn unit_runs_under_dedicated_system_account() {
    let unit = read("systemd/wallos-server.service");
    assert!(unit.contains("User=wallos-server"), "compte dédié");
    assert!(unit.contains("Group=wallos-server"), "groupe dédié");
    assert!(unit.contains("NoNewPrivileges=true"), "non privilégié");
    assert!(
        unit.contains("EnvironmentFile=/etc/wallos-server/wallos-server.env"),
        "la configuration vient de /etc"
    );
    assert!(
        unit.contains("WEBUI_DIR=/usr/share/wallos-server/webui"),
        "l'interface livrée est servie"
    );
    let postinstall = read("scripts/postinstall.sh");
    assert!(
        postinstall.contains("--system") && postinstall.contains("--shell /usr/sbin/nologin"),
        "compte système sans session interactive"
    );
}

#[test]
#[verifies(REQ-OPS-010, case = "mise à jour : le service redémarre sur la nouvelle version")]
fn upgrade_restarts_the_service() {
    let postinstall = read("scripts/postinstall.sh");
    assert!(
        postinstall.contains("try-restart"),
        "try-restart : redémarre si actif, n'impose pas un démarrage"
    );
}

#[test]
#[verifies(REQ-OPS-010, case = "désinstallation ordinaire : configuration conservée, purge explicite")]
fn uninstall_preserves_config_unless_purged() {
    let postremove = read("scripts/postremove.sh");
    assert!(
        postremove.contains("purge"),
        "la suppression de la configuration n'a lieu que sur purge explicite"
    );
}

#[test]
#[verifies(REQ-OPS-010, case = "le serveur de base de données n'est ni imposé ni installé d'office")]
fn database_is_not_a_package_dependency() {
    let recipe = read("nfpm.yaml");
    assert!(
        !recipe.contains("postgresql"),
        "PostgreSQL ne figure pas dans les dépendances : la connexion est décrite par la configuration"
    );
}

#[test]
#[verifies(REQ-OPS-010, case = "deb et rpm sont produits pour la même version, depuis la même recette")]
fn release_builds_both_formats_same_version() {
    let recipe = read("nfpm.yaml");
    assert!(
        recipe.contains("${WALLOS_VERSION}"),
        "la version est injectée une seule fois pour les deux formats"
    );
    let workflow =
        std::fs::read_to_string(packaging_dir().join("../.github/workflows/release.yml"))
            .expect("workflow de release");
    assert!(
        workflow.contains("--packager deb") && workflow.contains("--packager rpm"),
        "le workflow construit les deux formats"
    );
}
