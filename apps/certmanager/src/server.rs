use axum::Router;
use axum::body::Body;
use axum::extract::{Form, Query, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::Response;
use axum::routing::{get, post};
use foundation_http_server::Server;
use foundation_templating::{RenderedTemplate, TemplateEngine};
use serde::Deserialize;
use sqlx::PgPool;
use sqlx::types::chrono::Utc;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::error::ServerResult;
use crate::renewal::Renewer;
use crate::templates::IndexContext;

#[derive(Clone)]
struct ApplicationState {
    template_engine: TemplateEngine,
    renewer: Renewer,
    pool: PgPool,
}

pub fn build(
    template_engine: TemplateEngine,
    renewer: Renewer,
    pool: PgPool,
    listener: TcpListener,
) -> Server {
    let state = ApplicationState {
        template_engine,
        renewer,
        pool,
    };

    let router = Router::new()
        .route("/", get(index))
        .route("/register", post(register_domain))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state);

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
    let domains = crate::persistence::select_latest_expiry_per_domain(&pool)
        .await
        .map_err(color_eyre::Report::from)?;
    let context = IndexContext::new(domains, query.error);
    let rendered = template_engine.render_serialized("index.tera.html", &context)?;

    Ok(rendered)
}

#[derive(Debug, Deserialize)]
struct RegisterDomainForm {
    domain: String,
}

#[tracing::instrument(skip(renewer, pool))]
async fn register_domain(
    State(ApplicationState { renewer, pool, .. }): State<ApplicationState>,
    Form(RegisterDomainForm { domain }): Form<RegisterDomainForm>,
) -> ServerResult<Response> {
    let mut tx = pool.begin().await.map_err(color_eyre::Report::from)?;

    let domain_uid = crate::persistence::insert_domain(&mut tx, &domain)
        .await
        .map_err(color_eyre::Report::from)?;

    match renewer.renew(&domain).await {
        Ok(expires_at) => {
            let certificate_uid =
                crate::persistence::insert_certificate(&mut tx, domain_uid, Utc::now(), expires_at)
                    .await
                    .map_err(color_eyre::Report::from)?;
            tx.commit().await.map_err(color_eyre::Report::from)?;
            tracing::info!(%domain, %domain_uid, %certificate_uid, "Domain registered and certificate issued");
            redirect("/")
        }
        Err(e) => {
            let error_msg = e.to_string();
            tracing::warn!(%domain, error = %error_msg, "Failed to issue certificate");
            redirect_with_error(&error_msg)
        }
    }
}

fn redirect(path: &str) -> ServerResult<Response> {
    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, path)
        .body(Body::empty())
        .map_err(color_eyre::Report::from)?;

    Ok(res)
}

fn redirect_with_error(message: &str) -> ServerResult<Response> {
    let encoded = urlencoding::encode(message);
    let location = format!("/?error={}", encoded);

    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, location)
        .body(Body::empty())
        .map_err(color_eyre::Report::from)?;

    Ok(res)
}
