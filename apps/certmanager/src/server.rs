use axum::Router;
use axum::body::Body;
use axum::extract::{Form, Query, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::Response;
use axum::routing::{get, post};
use color_eyre::Report;
use foundation_http_server::Server;
use foundation_templating::{RenderedTemplate, TemplateEngine};
use serde::Deserialize;
use sqlx::PgPool;
use sqlx::types::chrono::Utc;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::error::ServerResult;
use crate::persistence::DomainStatus;
use crate::renewal::Renewer;
use crate::templates::IndexContext;
use crate::uid::DomainUid;

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
        .route("/retire", post(retire_domain))
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
        .map_err(Report::from)?;
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
    let mut tx = pool.begin().await.map_err(Report::from)?;

    let existing = crate::persistence::select_domain_by_name(&mut tx, &domain)
        .await
        .map_err(Report::from)?;

    let domain_uid = match existing {
        Some(record) if record.status == DomainStatus::Active => {
            return redirect_with_error(&format!("{domain} is already registered"));
        }
        Some(record) => {
            crate::persistence::resurrect_domain(&mut tx, record.domain_uid)
                .await
                .map_err(Report::from)?;
            record.domain_uid
        }
        None => crate::persistence::insert_domain(&mut tx, &domain)
            .await
            .map_err(Report::from)?,
    };

    match renewer.renew(&domain).await {
        Ok(expires_at) => {
            let certificate_uid =
                crate::persistence::insert_certificate(&mut tx, domain_uid, Utc::now(), expires_at)
                    .await
                    .map_err(Report::from)?;
            tx.commit().await.map_err(Report::from)?;
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

#[derive(Debug, Deserialize)]
struct RetireDomainForm {
    domain_uid: Uuid,
}

#[tracing::instrument(skip(renewer, pool))]
async fn retire_domain(
    State(ApplicationState { renewer, pool, .. }): State<ApplicationState>,
    Form(RetireDomainForm { domain_uid }): Form<RetireDomainForm>,
) -> ServerResult<Response> {
    let domain_uid = DomainUid::from(domain_uid);
    let mut tx = pool.begin().await.map_err(Report::from)?;

    let Some(record) = crate::persistence::select_domain_by_uid(&mut tx, domain_uid)
        .await
        .map_err(Report::from)?
    else {
        return redirect_with_error("Domain not found");
    };

    crate::persistence::retire_domain(&mut tx, domain_uid)
        .await
        .map_err(Report::from)?;

    // Commit first so renewals stop even if the S3 deletion fails
    tx.commit().await.map_err(Report::from)?;

    match renewer.delete_certificate(&record.name).await {
        Ok(()) => {
            tracing::info!(domain = %record.name, %domain_uid, "Domain retired and certificate deleted");
            redirect("/")
        }
        Err(e) => {
            tracing::warn!(domain = %record.name, error = %e, "Domain retired but failed to delete certificate");
            redirect_with_error(&format!(
                "{} was retired but its certificate could not be deleted: {e}",
                record.name
            ))
        }
    }
}

fn redirect(path: &str) -> ServerResult<Response> {
    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, path)
        .body(Body::empty())
        .map_err(Report::from)?;

    Ok(res)
}

fn redirect_with_error(message: &str) -> ServerResult<Response> {
    let encoded = urlencoding::encode(message);
    let location = format!("/?error={}", encoded);

    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, location)
        .body(Body::empty())
        .map_err(Report::from)?;

    Ok(res)
}
