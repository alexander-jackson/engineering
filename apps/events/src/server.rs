use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::Response;
use axum::routing::{get, post};
use chrono::{Local, Utc};
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use foundation_templating::{RenderedTemplate, TemplateEngine};
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::error::ServerResult;
use crate::persistence::EventType;
use crate::templates::{HistoryContext, IndexContext};

#[derive(Clone)]
struct ApplicationState {
    template_engine: TemplateEngine,
    pool: PgPool,
}

pub fn build(template_engine: TemplateEngine, pool: PgPool, listener: TcpListener) -> Server {
    let state = ApplicationState {
        template_engine,
        pool,
    };

    let router = Router::new()
        .route("/", get(index))
        .route("/history", get(history))
        .route("/insert", post(insert))
        .route("/remove", post(remove))
        .with_state(state);

    Server::new(router, listener)
}

#[tracing::instrument(skip(template_engine, pool))]
async fn index(
    State(ApplicationState {
        template_engine,
        pool,
        ..
    }): State<ApplicationState>,
) -> ServerResult<RenderedTemplate> {
    let today_start = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();

    let stats = crate::persistence::get_daily_stats(&pool, today_start, Utc::now()).await?;
    let context = IndexContext::from(stats);
    let rendered = template_engine.render_serialized("index.tera.html", &context)?;

    Ok(rendered)
}

#[tracing::instrument(skip(template_engine, pool))]
async fn history(
    State(ApplicationState {
        template_engine,
        pool,
        ..
    }): State<ApplicationState>,
) -> ServerResult<RenderedTemplate> {
    let days = crate::persistence::get_history(&pool, Utc::now()).await?;
    let context = HistoryContext::from(days);
    let rendered = template_engine.render_serialized("history.tera.html", &context)?;

    Ok(rendered)
}

#[tracing::instrument(skip(pool))]
async fn insert(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
) -> ServerResult<Response> {
    crate::persistence::record_event(&pool, EventType::Inserted, Utc::now()).await?;
    tracing::info!("recorded insert event");
    Ok(redirect("/")?)
}

#[tracing::instrument(skip(pool))]
async fn remove(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
) -> ServerResult<Response> {
    crate::persistence::record_event(&pool, EventType::Removed, Utc::now()).await?;
    tracing::info!("recorded remove event");
    Ok(redirect("/")?)
}

fn redirect(path: &'static str) -> Result<Response> {
    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, path)
        .body(Body::empty())?;

    Ok(res)
}
