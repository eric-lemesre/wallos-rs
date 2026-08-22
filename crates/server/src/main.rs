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

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let db = Db::connect(&database_url).await?;
    db.migrate().await?;

    if std::env::var("ENCRYPTION_KEY")
        .ok()
        .and_then(|raw| wallos_core::secrets::derive_key(&raw))
        .is_none()
    {
        // REQ-SEC-004 : sans clé, la création d'un canal à secrets est refusée (422) — signaler
        // clairement à l'opérateur plutôt que d'échouer silencieusement plus tard.
        tracing::warn!(
            "ENCRYPTION_KEY absente : chiffrement au repos désactivé, les canaux à secrets seront refusés"
        );
    }
    let app = wallos_server::app_with_db(db.clone());
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
