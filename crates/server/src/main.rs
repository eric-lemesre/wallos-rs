//! Point d'entrée du serveur wallos-rs.

use std::net::SocketAddr;

use anyhow::Context;
use tracing::info;
use wallos_storage::Db;

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
    axum::serve(listener, app).await?;
    Ok(())
}
