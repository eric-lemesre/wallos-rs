//! Point d'entrée du serveur wallos-rs.

use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let app = wallos_server::app();
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("wallos-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
