//! Point d'entrée du serveur wallos-rs.

use anyhow::Context;
use tracing::info;
use wallos_core::requirement;
use wallos_storage::Db;

/// Démarre le service : connexion + migrations, puis sert l'API (dont la santé, REQ-OPS-001).
#[requirement(REQ-OPS-001)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    // REQ-OPS-004 : configuration validée avant de servir la moindre requête — les manques
    // bloquants arrêtent ici en nommant la variable, les manques tolérables sont journalisés
    // avec leur conséquence fonctionnelle.
    let report = wallos_server::config::validate_startup(&|name| std::env::var(name).ok())
        .context("configuration invalide")?;
    for warning in &report.warnings {
        tracing::warn!("{warning}");
    }
    if let Ok(raw) = std::env::var("ENCRYPTION_KEY")
        && !raw.is_empty()
        && wallos_core::secrets::derive_key(&raw).is_none()
    {
        // REQ-SEC-004 : clé présente mais inutilisable — la création d'un canal à secrets sera
        // refusée (422). Seul le nom de la variable est cité, jamais sa valeur.
        tracing::warn!(
            "ENCRYPTION_KEY présente mais inutilisable : les canaux à secrets seront refusés"
        );
    }

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let db = Db::connect(&database_url).await?;
    db.migrate().await?;
    // REQ-OPS-003 : l'interface compilée est servie si WEBUI_DIR la désigne ; son absence est
    // signalée et l'API reste seule servie.
    let raw_webui = std::env::var(wallos_server::webui::WEBUI_DIR_VAR).ok();
    let ui = wallos_server::webui::detect(raw_webui.as_deref());
    match &ui {
        wallos_server::webui::WebUi::Enabled(dir) => {
            info!("interface web servie depuis {}", dir.display());
        }
        wallos_server::webui::WebUi::Disabled { reason } => tracing::warn!("{reason}"),
    }
    let app = wallos_server::app_with_db_webui(db.clone(), &ui);
    // REQ-OPS-002 : écoute configurable par LISTEN_ADDR, arrêt immédiat si la valeur est invalide.
    let raw_listen = std::env::var(wallos_server::listen::LISTEN_ADDR_VAR).ok();
    let addr = wallos_server::listen::resolve_listen_addr(raw_listen.as_deref())?;
    info!("wallos-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // REQ-OPS-006 : drainage des requêtes en vol sur SIGTERM/SIGINT, délai de grâce, fermeture
    // explicite de la base. L'adresse du pair reste exposée aux handlers (IP source pour la
    // limitation du taux d'authentification, REQ-AUT-008) : le service est construit avec
    // `into_make_service_with_connect_info` dans `serve_with_graceful_shutdown`.
    wallos_server::shutdown::serve_with_graceful_shutdown(
        listener,
        app,
        db,
        shutdown_signal(),
        wallos_server::shutdown::DEFAULT_GRACE,
    )
    .await?;
    Ok(())
}

/// Résout à la réception de SIGTERM (orchestrateur, systemd) ou SIGINT (Ctrl-C).
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!(
                        "impossible d'écouter SIGTERM : {e} — seul Ctrl-C arrêtera proprement"
                    );
                    let _ = ctrl_c.await;
                    return;
                }
            };
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
