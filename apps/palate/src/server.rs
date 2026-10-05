use axum::Router;
use axum::body::Body;
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::Response;
use axum::routing::{get, post};
use chrono::Utc;
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use foundation_templating::{RenderedTemplate, TemplateEngine};
use serde::Deserialize;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::error::{ServerError, ServerResult};
use crate::openrouter::OpenRouterClient;
use crate::persistence::Role;
use crate::templates::{ConversationContext, IndexContext};
use crate::uid::ConversationUid;

#[derive(Clone)]
struct ApplicationState {
    template_engine: TemplateEngine,
    pool: PgPool,
    openrouter: OpenRouterClient,
}

pub fn build_router(
    template_engine: TemplateEngine,
    pool: PgPool,
    openrouter: OpenRouterClient,
) -> Router {
    let state = ApplicationState {
        template_engine,
        pool,
        openrouter,
    };

    Router::new()
        .route("/", get(index))
        .route("/conversations/{conversation_uid}", get(conversation))
        .route(
            "/conversations/{conversation_uid}/messages",
            post(add_message),
        )
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state)
}

pub fn build(
    template_engine: TemplateEngine,
    pool: PgPool,
    openrouter: OpenRouterClient,
    listener: TcpListener,
) -> Server {
    let router = build_router(template_engine, pool, openrouter);
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
    let conversations = crate::persistence::select_conversations(&pool).await?;
    let context = IndexContext::from(conversations);

    Ok(template_engine.render_serialized("index.tera.html", &context)?)
}

#[tracing::instrument(skip(template_engine, pool))]
async fn conversation(
    State(ApplicationState {
        template_engine,
        pool,
        ..
    }): State<ApplicationState>,
    Path(conversation_uid): Path<ConversationUid>,
) -> ServerResult<RenderedTemplate> {
    let conversation = crate::persistence::select_conversation(&pool, conversation_uid)
        .await?
        .ok_or(ServerError::NotFound)?;

    let context = ConversationContext::from(conversation);

    Ok(template_engine.render_serialized("conversation.tera.html", &context)?)
}

#[derive(Debug, Deserialize)]
struct AddMessageForm {
    content: String,
}

/// Stores the message, waits for the model's reply and redirects back to the conversation.
#[tracing::instrument(skip(pool, openrouter, content))]
async fn add_message(
    State(ApplicationState {
        pool, openrouter, ..
    }): State<ApplicationState>,
    Path(conversation_uid): Path<ConversationUid>,
    Form(AddMessageForm { content }): Form<AddMessageForm>,
) -> ServerResult<Response> {
    let content = content.trim();

    if !content.is_empty() {
        if crate::persistence::select_conversation(&pool, conversation_uid)
            .await?
            .is_none()
        {
            return Err(ServerError::NotFound);
        }

        crate::persistence::insert_message(
            &pool,
            conversation_uid,
            Role::User,
            content,
            Utc::now(),
        )
        .await?;

        crate::conversation::generate_reply(&pool, &openrouter, conversation_uid).await?;

        tracing::info!(%conversation_uid, "replied to a follow-up message");
    }

    Ok(redirect(&format!("/conversations/{conversation_uid}"))?)
}

fn redirect(path: &str) -> Result<Response> {
    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, path)
        .body(Body::empty())?;

    Ok(res)
}
