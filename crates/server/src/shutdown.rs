//! Arrêt propre sur signal d'extinction (REQ-OPS-006).
//!
//! Sous orchestrateur, une extinction commence par un signal poli puis se termine par une mise à
//! mort. À la réception du signal : plus aucune nouvelle connexion, les requêtes en vol
//! s'achèvent ; passé le délai de grâce, le serveur quitte malgré tout — en succès, après l'avoir
//! signalé — pour ne pas attendre la mise à mort. Les connexions à la base sont refermées
//! explicitement dans tous les cas.

use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use tokio::net::TcpListener;
use tracing::{info, warn};
use wallos_core::requirement;
use wallos_storage::Db;

/// Délai de grâce par défaut accordé aux requêtes en vol après le signal.
pub const DEFAULT_GRACE: Duration = Duration::from_secs(10);

/// Sert `app` jusqu'à la résolution de `signal`, puis draine et referme la base.
///
/// Comportement après `signal` : le port n'accepte plus de connexion, les requêtes en cours
/// disposent de `grace` pour s'achever ; au-delà, le serveur quitte en succès après l'avoir
/// journalisé. La base est refermée explicitement avant de rendre la main.
///
/// # Errors
/// Une erreur d'E/S du serveur HTTP (le dépassement du délai de grâce n'en est pas une).
#[requirement(REQ-OPS-006)]
pub async fn serve_with_graceful_shutdown(
    listener: TcpListener,
    app: Router,
    db: Db,
    signal: impl Future<Output = ()> + Send + 'static,
    grace: Duration,
) -> std::io::Result<()> {
    let (drained_tx, drained_rx) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        signal.await;
        info!(
            "signal d'extinction reçu : plus de nouvelles connexions, drainage des requêtes en vol"
        );
        let _ = drained_tx.send(());
    });

    let result = tokio::select! {
        res = server => res,
        () = async {
            // Le délai de grâce ne court qu'à partir du signal.
            let _ = drained_rx.await;
            tokio::time::sleep(grace).await;
        } => {
            warn!(
                "délai de grâce écoulé ({} s) : arrêt malgré des requêtes encore en vol",
                grace.as_secs_f32()
            );
            Ok(())
        }
    };

    db.close().await;
    info!("connexions à la base refermées, arrêt terminé");
    result
}
