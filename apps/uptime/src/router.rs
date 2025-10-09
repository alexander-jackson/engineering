use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Form, Router};
use chrono::Utc;
use color_eyre::eyre::Result;
use humantime::format_duration;
use serde::{Deserialize, Serialize, Serializer};
use sqlx::types::chrono::DateTime;
use sqlx::PgPool;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::templates::{RenderedTemplate, TemplateEngine};

/// A wrapper type for timestamps that serializes as a human-readable duration relative to now
#[derive(Clone)]
struct PrettyDuration(DateTime<Utc>);

impl Serialize for PrettyDuration {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let delta = (Utc::now() - self.0).abs();
        let duration = Duration::from_millis(delta.num_milliseconds() as u64);
        let formatted = format_duration(duration).to_string();
        serializer.serialize_str(&formatted)
    }
}

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
        .route("/notifications", get(notifications))
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
    queried: PrettyDuration,
}

#[derive(Serialize)]
struct OriginFailure {
    origin_uid: Uuid,
    uri: String,
    failure_reason: String,
    queried: PrettyDuration,
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
        .map(|origin| IndexOrigin {
            origin_uid: origin.origin_uid,
            uri: origin.uri,
            status: origin.status as u16,
            latency_millis: origin.latency_millis as u64,
            queried: PrettyDuration(origin.queried_at),
        })
        .collect();

    let failing_origins = crate::persistence::fetch_origins_with_most_recent_failure_metrics(&pool)
        .await
        .expect("failed to fetch failing origins")
        .into_iter()
        .map(|origin| OriginFailure {
            origin_uid: origin.origin_uid,
            uri: origin.uri,
            failure_reason: origin.failure_reason,
            queried: PrettyDuration(origin.queried_at),
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
    Healthy {
        last_failure: Option<PrettyDuration>,
    },
    Down {
        last_success: Option<PrettyDuration>,
    },
    Unknown,
}

#[derive(Serialize)]
struct CertificateInfo {
    expires_at: String,
    checked_at: PrettyDuration,
    days_until_expiry: i64,
    is_expiring_soon: bool,
}

#[derive(Serialize)]
struct OriginDetailContext {
    origin_uid: Uuid,
    uri: String,
    queries: Vec<QueryInfo>,
    total_queries: usize,
    success_rate: f64,
    health: OriginHealthStatus,
    certificate: Option<CertificateInfo>,
}

#[derive(Serialize)]
struct QueryInfo {
    status: Option<u16>,
    latency_millis: Option<u64>,
    failure_reason: Option<String>,
    queried: PrettyDuration,
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
        .map(|query| QueryInfo {
            status: query.status.map(|s| s as u16),
            latency_millis: query.latency_millis.map(|l| l as u64),
            failure_reason: query.failure_reason,
            queried: PrettyDuration(query.queried_at),
            is_success: query.is_success,
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
                .map(PrettyDuration);
            OriginHealthStatus::Healthy { last_failure }
        }
        Some(false) => {
            // Currently down, fetch when it last succeeded
            let last_success = crate::persistence::fetch_most_recent_success(&pool, origin_uid)
                .await
                .expect("failed to fetch most recent success")
                .map(PrettyDuration);
            OriginHealthStatus::Down { last_success }
        }
        None => OriginHealthStatus::Unknown,
    };

    // Fetch certificate information if available
    let certificate = crate::persistence::fetch_most_recent_certificate_check(&pool, origin_uid)
        .await
        .expect("failed to fetch certificate check")
        .map(|cert| {
            let days_until_expiry = (cert.expires_at - Utc::now()).num_days();
            let is_expiring_soon = days_until_expiry <= 30;

            CertificateInfo {
                expires_at: cert.expires_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                checked_at: PrettyDuration(cert.checked_at),
                days_until_expiry,
                is_expiring_soon,
            }
        });

    let context = OriginDetailContext {
        origin_uid: origin.origin_uid,
        uri: origin.uri,
        queries,
        total_queries,
        success_rate,
        health,
        certificate,
    };

    template_engine
        .render_serialized("origin.tera.html", &context)
        .expect("failed to render template")
        .into_response()
}

#[derive(Serialize)]
struct NotificationInfo {
    notification_uid: Uuid,
    origin_uid: Uuid,
    uri: String,
    notification_type: String,
    subject: String,
    message: String,
    created: PrettyDuration,
}

#[derive(Serialize)]
struct NotificationsContext {
    notifications: Vec<NotificationInfo>,
}

async fn notifications(
    State(ApplicationState {
        pool,
        template_engine,
    }): State<ApplicationState>,
) -> RenderedTemplate {
    let notifications = crate::persistence::fetch_recent_notifications(&pool, 50)
        .await
        .expect("failed to fetch notifications")
        .into_iter()
        .map(|n| NotificationInfo {
            notification_uid: n.notification_uid,
            origin_uid: n.origin_uid,
            uri: n.uri,
            notification_type: n.notification_type,
            subject: n.subject,
            message: n.message,
            created: PrettyDuration(n.created_at),
        })
        .collect();

    let context = NotificationsContext { notifications };

    template_engine
        .render_serialized("notifications.tera.html", &context)
        .expect("failed to render template")
}
