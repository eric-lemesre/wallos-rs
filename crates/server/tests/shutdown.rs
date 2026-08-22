//! Tests d'intégration de l'arrêt propre sur signal d'extinction (REQ-OPS-006).
//!
//! Le signal est simulé par un canal oneshot : la mécanique de drainage, de délai de grâce et de
//! fermeture de la base est exercée sur un vrai socket, sans dépendre d'un signal POSIX ni d'une
//! base joignable (pool paresseux jamais connecté).

use std::time::Duration;

use axum::Router;
use axum::routing::get;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wallos_req_macros::verifies;
use wallos_server::shutdown::serve_with_graceful_shutdown;
use wallos_storage::Db;

/// Pool paresseux : aucune connexion n'est ouverte, mais `close()` reste observable.
fn lazy_db() -> Db {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .expect("pool paresseux");
    Db::from_pool(pool)
}

/// Émet un GET brut et rend la réponse complète (statut + corps).
async fn http_get(addr: std::net::SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connexion");
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await
        .expect("écriture requête");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .expect("lecture réponse");
    response
}

fn slow_app(delay: Duration) -> Router {
    Router::new().route(
        "/slow",
        get(move || async move {
            tokio::time::sleep(delay).await;
            "terminé"
        }),
    )
}

#[tokio::test]
#[verifies(REQ-OPS-006, case = "les requêtes en vol s'achèvent après le signal")]
async fn in_flight_request_completes_after_signal() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let db = lazy_db();
    let pool = db.pool().clone();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let server = tokio::spawn(serve_with_graceful_shutdown(
        listener,
        slow_app(Duration::from_millis(300)),
        db,
        async move {
            let _ = rx.await;
        },
        Duration::from_secs(5),
    ));

    let client = tokio::spawn(async move { http_get(addr, "/slow").await });
    // La requête est en vol quand le signal part.
    tokio::time::sleep(Duration::from_millis(80)).await;
    tx.send(()).expect("signal");

    let response = client.await.expect("client");
    assert!(
        response.contains("200"),
        "réponse servie malgré le signal : {response}"
    );
    assert!(response.contains("terminé"), "corps complet : {response}");

    server.await.expect("join").expect("arrêt propre");
    assert!(
        pool.is_closed(),
        "les connexions à la base sont refermées explicitement"
    );
}

#[tokio::test]
#[verifies(REQ-OPS-006, case = "délai de grâce dépassé : sortie en succès malgré une requête en vol")]
async fn grace_period_exceeded_still_exits_ok() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let db = lazy_db();
    let pool = db.pool().clone();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let server = tokio::spawn(serve_with_graceful_shutdown(
        listener,
        slow_app(Duration::from_secs(30)),
        db,
        async move {
            let _ = rx.await;
        },
        Duration::from_millis(150),
    ));

    // Une requête qui excédera largement le délai de grâce.
    let client = tokio::spawn(async move { http_get(addr, "/slow").await });
    tokio::time::sleep(Duration::from_millis(80)).await;
    tx.send(()).expect("signal");

    // Le serveur rend la main en succès bien avant la fin de la requête de 30 s.
    let result = tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .expect("le serveur quitte malgré la requête en vol")
        .expect("join");
    assert!(
        result.is_ok(),
        "code de sortie de succès attendu : {result:?}"
    );
    assert!(pool.is_closed(), "base refermée même en sortie forcée");
    client.abort();
}
