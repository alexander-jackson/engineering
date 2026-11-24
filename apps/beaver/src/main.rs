use std::net::{Ipv4Addr, SocketAddrV4};

use axum::Router;
use axum::routing::get;
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use foundation_shutdown::ShutdownCoordinator;
use serde::Deserialize;
use tokio::net::TcpListener;

#[derive(Deserialize)]
struct Configuration {
    server: ServerConfiguration,
}

#[derive(Deserialize)]
struct ServerConfiguration {
    host: Ipv4Addr,
    port: u16,
}

async fn hello() -> &'static str {
    "Hello, World!"
}

fn router() -> Router {
    Router::new().route("/", get(hello))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    let server = Server::new(router(), listener);

    ShutdownCoordinator::new().with_task(server).run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use color_eyre::eyre::Result;

    use crate::router;

    #[tokio::test]
    async fn serves_hello_world() -> Result<()> {
        let request = Request::get("/").body(Body::empty())?;
        let response = router().oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await?.to_bytes();
        assert_eq!(&body[..], b"Hello, World!");

        Ok(())
    }
}
