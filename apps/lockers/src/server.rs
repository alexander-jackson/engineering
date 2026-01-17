use axum::Router;
use axum::body::Body;
use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::Response;
use axum::routing::{get, post};
use chrono::Utc;
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use serde::Deserialize;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::error::ServerResult;
use crate::persistence::BagType;
use crate::templates::{IndexContext, RenderedTemplate, TemplateEngine};

#[derive(Clone)]
struct ApplicationState {
    template_engine: TemplateEngine,
    pool: PgPool,
}

pub fn build_router(template_engine: TemplateEngine, pool: PgPool) -> Router {
    let state = ApplicationState {
        template_engine,
        pool,
    };

    Router::new()
        .route("/", get(index))
        .route("/add", post(add_locker))
        .route("/remove/{locker_number}", post(remove_locker))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state)
}

pub fn build(template_engine: TemplateEngine, pool: PgPool, listener: TcpListener) -> Server {
    let router = build_router(template_engine, pool);
    Server::new(router, listener)
}

#[derive(Debug, Deserialize)]
struct IndexQuery {
    error: Option<String>,
}

#[tracing::instrument(skip(template_engine, pool))]
async fn index(
    State(ApplicationState {
        template_engine,
        pool,
        ..
    }): State<ApplicationState>,
    Query(query): Query<IndexQuery>,
) -> ServerResult<RenderedTemplate> {
    let lockers = crate::persistence::select_all_lockers(&pool).await?;
    let context = IndexContext::new(lockers, query.error);
    let rendered = template_engine.render_serialized("index.tera.html", &context)?;

    Ok(rendered)
}

#[derive(Debug, Deserialize)]
struct AddLockerForm {
    locker_number: i16,
    bag_type: String,
}

#[tracing::instrument(skip(pool))]
async fn add_locker(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
    Form(AddLockerForm {
        locker_number,
        bag_type,
    }): Form<AddLockerForm>,
) -> ServerResult<Response> {
    let event_uid = Uuid::new_v4();
    let now = Utc::now().naive_local();
    let bag_type = BagType::from(bag_type);

    match crate::persistence::insert_check_in_event(&pool, event_uid, locker_number, bag_type, now)
        .await
    {
        Ok(_) => {
            tracing::info!(%event_uid, locker_number, ?bag_type, "checked in bag");
            Ok(redirect("/")?)
        }
        Err(e) => {
            let error_msg = e.to_string();
            tracing::warn!(locker_number, ?bag_type, error = %error_msg, "failed to check in bag");
            Ok(redirect_with_error(&error_msg)?)
        }
    }
}

#[tracing::instrument(skip(pool))]
async fn remove_locker(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
    Path(locker_number): Path<i16>,
) -> ServerResult<Response> {
    let event_uid = Uuid::new_v4();
    let now = Utc::now().naive_local();

    match crate::persistence::insert_check_out_event(&pool, event_uid, locker_number, now).await {
        Ok(_) => {
            tracing::info!(%event_uid, locker_number, "checked out bag");
            Ok(redirect("/")?)
        }
        Err(e) => {
            let error_msg = e.to_string();
            tracing::warn!(locker_number, error = %error_msg, "failed to check out bag");
            Ok(redirect_with_error(&error_msg)?)
        }
    }
}

fn redirect(path: &'static str) -> Result<Response> {
    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, path)
        .body(Body::empty())?;

    Ok(res)
}

fn redirect_with_error(message: &str) -> Result<Response> {
    use urlencoding::encode;
    let encoded_message = encode(message);
    let location = format!("/?error={}", encoded_message);

    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, location)
        .body(Body::empty())?;

    Ok(res)
}
