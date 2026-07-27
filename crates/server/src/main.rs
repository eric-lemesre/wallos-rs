//! Point d'entrée du serveur wallos-rs.

use std::net::SocketAddr;

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

    let app = wallos_server::app_with_db(db);
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("wallos-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // `into_make_service_with_connect_info` expose l'adresse du pair aux handlers (IP source pour la
    // limitation du taux d'authentification, REQ-AUT-008).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
