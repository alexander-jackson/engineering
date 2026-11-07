use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{Form, Json, Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use chrono::{NaiveDate, Utc};
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use foundation_templating::TemplateEngine;
use moka::future::Cache;
use serde::Deserialize;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::clock;
use crate::error::ServerResult;
use crate::persistence::ItemState;
use crate::templates::IndexContext;
use crate::uid::ItemUid;

// Keyed by the day being viewed and what today was when it was built, as the context
// embeds today-relative state (navigation links, the add form). Without the second
// part a page cached on its own day would be reused as stale once the day rolled over.
// Any change drops every entry.
pub type IndexCache = Cache<(NaiveDate, NaiveDate), Arc<IndexContext>>;

#[derive(Clone)]
struct ApplicationState {
    template_engine: TemplateEngine,
    pool: PgPool,
    index_cache: IndexCache,
}

pub fn build_router(
    template_engine: TemplateEngine,
    pool: PgPool,
    index_cache: IndexCache,
) -> Router {
    let state = ApplicationState {
        template_engine,
        pool,
        index_cache,
    };

    Router::new()
        .route("/", get(templated))
        .route("/add", post(add_item))
        .route("/update/{item_uid}", patch(update_item))
        .route("/update/{item_uid}/content", patch(edit_item))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state)
}

pub fn build(
    template_engine: TemplateEngine,
    pool: PgPool,
    index_cache: IndexCache,
    listener: TcpListener,
) -> Server {
    let router = build_router(template_engine, pool, index_cache);
    Server::new(router, listener)
}

#[derive(Debug, Deserialize)]
struct IndexQuery {
    date: Option<NaiveDate>,
}

#[tracing::instrument(skip(template_engine, pool, index_cache))]
async fn templated(
    State(ApplicationState {
        template_engine,
        pool,
        index_cache,
        ..
    }): State<ApplicationState>,
    Query(IndexQuery { date }): Query<IndexQuery>,
) -> ServerResult<Response> {
    let today = clock::today();
    let date = date.unwrap_or(today);

    // Only the last week, and nothing in the future, can be viewed
    if date > today || date < clock::earliest_viewable(today) {
        return Ok(status(StatusCode::NOT_FOUND)?);
    }

    let context = match index_cache.get(&(date, today)).await {
        Some(ctx) => ctx,
        None => {
            let items = crate::persistence::select_items(&pool, date).await?;
            let context = Arc::new(IndexContext::new(date, today, items));

            index_cache
                .insert((date, today), Arc::clone(&context))
                .await;

            context
        }
    };

    let rendered = template_engine.render_serialized("index.tera.html", &context)?;

    Ok(rendered.into_response())
}

#[derive(Debug, Deserialize)]
struct AddItemForm {
    content: String,
}

#[tracing::instrument(skip(pool, index_cache, content))]
async fn add_item(
    State(ApplicationState {
        pool, index_cache, ..
    }): State<ApplicationState>,
    Form(AddItemForm { content }): Form<AddItemForm>,
) -> ServerResult<Response> {
    let item_uid = ItemUid::new();
    let now = Utc::now().naive_local();

    crate::persistence::create_item(&pool, item_uid, &content, now).await?;
    index_cache.invalidate_all();

    tracing::info!(%item_uid, "added an item");

    Ok(redirect("/")?)
}

#[derive(Debug, Deserialize)]
struct UpdateItemRequest {
    state: ItemState,
}

#[tracing::instrument(skip(pool, index_cache))]
async fn update_item(
    State(ApplicationState {
        pool, index_cache, ..
    }): State<ApplicationState>,
    Path(item_uid): Path<ItemUid>,
    Json(request): Json<UpdateItemRequest>,
) -> ServerResult<Response> {
    crate::persistence::update_item(&pool, item_uid, request.state).await?;
    index_cache.invalidate_all();

    tracing::info!(%item_uid, ?request, "updated an item");

    Ok(success()?)
}

#[derive(Debug, Deserialize)]
struct EditItemRequest {
    content: String,
}

#[tracing::instrument(skip(pool, index_cache, request))]
async fn edit_item(
    State(ApplicationState {
        pool, index_cache, ..
    }): State<ApplicationState>,
    Path(item_uid): Path<ItemUid>,
    Json(request): Json<EditItemRequest>,
) -> ServerResult<Response> {
    if request.content.trim().is_empty() {
        return Ok(status(StatusCode::BAD_REQUEST)?);
    }

    crate::persistence::edit_item(&pool, item_uid, &request.content).await?;
    index_cache.invalidate_all();

    tracing::info!(%item_uid, "edited an item");

    Ok(success()?)
}

fn success() -> Result<Response> {
    status(StatusCode::OK)
}

fn status(status: StatusCode) -> Result<Response> {
    let res = Response::builder().status(status).body(Body::empty())?;

    Ok(res)
}

fn redirect(path: &'static str) -> Result<Response> {
    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, path)
        .body(Body::empty())?;

    Ok(res)
}
