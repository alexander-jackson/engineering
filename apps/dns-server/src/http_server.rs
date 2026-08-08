use std::collections::HashSet;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use foundation_http_server::Server;
use serde::Deserialize;
use tokio::net::TcpListener;

use crate::blocklist::BlocklistManager;
use crate::persistence::DomainEventType;

struct ErrorDetails {
    report: color_eyre::Report,
}

impl From<color_eyre::Report> for ErrorDetails {
    fn from(report: color_eyre::Report) -> Self {
        Self { report }
    }
}

impl IntoResponse for ErrorDetails {
    fn into_response(self) -> axum::response::Response {
        tracing::error!(error = %self.report, "internal server error");

        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

type ServerResult<T> = Result<T, ErrorDetails>;

#[derive(Clone)]
struct ApplicationState {
    manager: BlocklistManager,
}

pub fn build(manager: BlocklistManager, listener: TcpListener) -> Server {
    let state = ApplicationState { manager };

    let router = Router::new()
        .route("/health", axum::routing::get(health_check))
        .route(
            "/api/v1/blocklist",
            get(get_blocked_domains).put(add_blocked_domain),
        )
        .with_state(state);

    Server::new(router, listener)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn get_blocked_domains(
    State(state): State<ApplicationState>,
) -> ServerResult<Json<HashSet<String>>> {
    let blocked_domains = state.manager.read().await?;

    Ok(Json(blocked_domains))
}

#[derive(Deserialize)]
struct BlockedDomainPayload {
    domain: String,
}

async fn add_blocked_domain(
    State(state): State<ApplicationState>,
    Json(payload): Json<BlockedDomainPayload>,
) -> ServerResult<()> {
    state
        .manager
        .update(&payload.domain, DomainEventType::Blocked)
        .await?;

    Ok(())
}
