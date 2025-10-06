use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Form, Router};
use chrono::Utc;
use color_eyre::eyre::Result;
use humantime::format_duration;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::templates::{RenderedTemplate, TemplateEngine};

#[derive(Clone)]
struct ApplicationState {
    pool: PgPool,
    template_engine: TemplateEngine,
}

pub fn build(pool: PgPool) -> Result<Router> {
    let template_engine = TemplateEngine::new()?;
    let state = ApplicationState {
        pool,
        template_engine,
    };

    let router = Router::new()
        .route("/", get(index))
        .route("/add-origin", get(add_origin_template).post(add_origin))
        .route("/origin/:origin_uid", get(origin_detail))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state);

    Ok(router)
}

#[derive(Serialize)]
struct IndexOrigin {
    origin_uid: Uuid,
    uri: String,
    status: u16,
    latency_millis: u64,
    queried: String,
}

#[derive(Serialize)]
struct OriginFailure {
    origin_uid: Uuid,
    uri: String,
    failure_reason: String,
    queried: String,
}

#[derive(Serialize)]
struct IndexContext {
    origins: Vec<IndexOrigin>,
    failing_origins: Vec<OriginFailure>,
}

async fn index(
    State(ApplicationState {
        pool,
        template_engine,
    }): State<ApplicationState>,
) -> RenderedTemplate {
    let origins = crate::persistence::fetch_origins_with_most_recent_success_metrics(&pool)
        .await
        .expect("failed to fetch origins")
        .into_iter()
        .map(|origin| {
            let delta = (Utc::now() - origin.queried_at).abs();
            let duration = Duration::from_millis(delta.num_milliseconds() as u64);

            IndexOrigin {
                origin_uid: origin.origin_uid,
                uri: origin.uri,
                status: origin.status as u16,
                latency_millis: origin.latency_millis as u64,
                queried: format_duration(duration).to_string(),
            }
        })
        .collect();

    let failing_origins = crate::persistence::fetch_origins_with_most_recent_failure_metrics(&pool)
        .await
        .expect("failed to fetch failing origins")
        .into_iter()
        .map(|origin| {
            let delta = (Utc::now() - origin.queried_at).abs();
            let duration = Duration::from_millis(delta.num_milliseconds() as u64);

            OriginFailure {
                origin_uid: origin.origin_uid,
                uri: origin.uri,
                failure_reason: origin.failure_reason,
                queried: format_duration(duration).to_string(),
            }
        })
        .collect();

    let context = IndexContext {
        origins,
        failing_origins,
    };

    template_engine
        .render_serialized("index.tera.html", &context)
        .expect("failed to render template")
}

async fn add_origin_template(
    State(ApplicationState {
        template_engine, ..
    }): State<ApplicationState>,
) -> RenderedTemplate {
    template_engine
        .render_contextless("add-origin.tera.html")
        .expect("failed to render template")
}

#[derive(Deserialize)]
struct OriginCreationRequest {
    uri: String,
}

async fn add_origin(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
    Form(OriginCreationRequest { uri }): Form<OriginCreationRequest>,
) -> Redirect {
    let origin_uid = Uuid::new_v4();

    crate::persistence::insert_origin(&pool, origin_uid, &uri)
        .await
        .expect("failed to insert origin");

    Redirect::to("/")
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum OriginHealthStatus {
    Healthy { last_failure: Option<String> },
    Down { last_success: Option<String> },
    Unknown,
}

#[derive(Serialize)]
struct OriginDetailContext {
    origin_uid: Uuid,
    uri: String,
    queries: Vec<QueryInfo>,
    total_queries: usize,
    success_rate: f64,
    health: OriginHealthStatus,
}

#[derive(Serialize)]
struct QueryInfo {
    status: Option<u16>,
    latency_millis: Option<u64>,
    failure_reason: Option<String>,
    queried: String,
    is_success: bool,
}

async fn origin_detail(
    Path(origin_uid): Path<Uuid>,
    State(ApplicationState {
        pool,
        template_engine,
    }): State<ApplicationState>,
) -> Response {
    // Fetch origin details
    let origin = match crate::persistence::fetch_origin_by_uid(&pool, origin_uid).await {
        Ok(Some(origin)) => origin,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Origin not found").into_response();
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch origin").into_response();
        }
    };

    // Fetch recent queries (both successful and failed)
    let queries = crate::persistence::fetch_recent_queries(&pool, origin_uid, 5)
        .await
        .expect("failed to fetch recent queries")
        .into_iter()
        .map(|query| {
            let delta = (Utc::now() - query.queried_at).abs();
            let duration = Duration::from_millis(delta.num_milliseconds() as u64);

            QueryInfo {
                status: query.status.map(|s| s as u16),
                latency_millis: query.latency_millis.map(|l| l as u64),
                failure_reason: query.failure_reason,
                queried: format_duration(duration).to_string(),
                is_success: query.is_success,
            }
        })
        .collect::<Vec<_>>();

    let total_queries = queries.len();
    let successful_count = queries.iter().filter(|q| q.is_success).count();
    let success_rate = if total_queries > 0 {
        (successful_count as f64 / total_queries as f64) * 100.0
    } else {
        0.0
    };

    // Determine health status based on most recent query
    let health = match queries.first().map(|q| q.is_success) {
        Some(true) => {
            // Currently healthy, fetch when it last failed
            let last_failure = crate::persistence::fetch_most_recent_failure(&pool, origin_uid)
                .await
                .expect("failed to fetch most recent failure")
                .map(|ts| {
                    let delta = (Utc::now() - ts).abs();
                    let duration = Duration::from_millis(delta.num_milliseconds() as u64);
                    format_duration(duration).to_string()
                });
            OriginHealthStatus::Healthy { last_failure }
        }
        Some(false) => {
            // Currently down, fetch when it last succeeded
            let last_success = crate::persistence::fetch_most_recent_success(&pool, origin_uid)
                .await
                .expect("failed to fetch most recent success")
                .map(|ts| {
                    let delta = (Utc::now() - ts).abs();
                    let duration = Duration::from_millis(delta.num_milliseconds() as u64);
                    format_duration(duration).to_string()
                });
            OriginHealthStatus::Down { last_success }
        }
        None => OriginHealthStatus::Unknown,
    };

    let context = OriginDetailContext {
        origin_uid: origin.origin_uid,
        uri: origin.uri,
        queries,
        total_queries,
        success_rate,
        health,
    };

    template_engine
        .render_serialized("origin.tera.html", &context)
        .expect("failed to render template")
        .into_response()
}
