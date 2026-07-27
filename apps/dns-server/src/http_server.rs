use std::collections::HashSet;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use foundation_http_server::Server;
use serde::Deserialize;
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::persistence::DomainEventType;

#[derive(Clone)]
struct ApplicationState {
    pool: PgPool,
}

pub fn build(pool: PgPool, listener: TcpListener) -> Server {
    let state = ApplicationState { pool };

    let router = Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/api/v1/blocklist", get(get_blocked_domains).put(add_blocked_domain))
        .with_state(state);

    Server::new(router, listener)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn get_blocked_domains(
    State(state): State<ApplicationState>,
) -> Result<Json<HashSet<String>>, StatusCode> {
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let blocked_domains = crate::persistence::select_blocked_domains(&mut tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(blocked_domains))
}

#[derive(Deserialize)]
struct BlockedDomainPayload {
    domain: String,
}

async fn add_blocked_domain(
    State(state): State<ApplicationState>,
    Json(payload): Json<BlockedDomainPayload>,
) -> Result<(), StatusCode> {
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let domain_uid = crate::persistence::insert_domain(&mut tx, &payload.domain)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    crate::persistence::insert_domain_event(&mut tx, domain_uid, DomainEventType::Blocked)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}
