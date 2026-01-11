use std::net::SocketAddrV4;

use axum::Router;
use axum::routing::get;
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use jsonwebtoken::DecodingKey;
use tokio::net::TcpListener;

use opentracker::endpoints::{self, AppState};

mod config;

use crate::config::Configuration;

#[tokio::main]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;

    let state = AppState {
        pool,
        decoding_key: DecodingKey::from_secret(config.authorisation.jwt_key.as_bytes()),
    };

    let app = Router::new()
        .route("/health", get(endpoints::health))
        .nest("/api", endpoints::router(state));

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(&addr).await?;

    let server = Server::new(app, listener);
    server.run().await?;

    Ok(())
}
