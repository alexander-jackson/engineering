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
    addr: Ipv4Addr,
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

    let addr = SocketAddrV4::new(config.addr, config.port);
    let listener = TcpListener::bind(addr).await?;

    let server = Server::new(router(), listener);

    ShutdownCoordinator::new().with_task(server).run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serves_hello_world() -> Result<()> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move { axum::serve(listener, router()).await });

        let body = reqwest::get(format!("http://{addr}/"))
            .await?
            .text()
            .await?;

        assert_eq!(body, "Hello, World!");

        Ok(())
    }
}
