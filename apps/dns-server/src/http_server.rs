use std::collections::HashSet;

use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use foundation_http_server::Server;
use foundation_templating::{RenderedTemplate, TemplateEngine};
use serde::Deserialize;
use tokio::net::TcpListener;

use crate::blocklist::{BlocklistBackend, BlocklistManager};
use crate::persistence::DomainEventType;
use crate::templates::IndexContext;

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
struct ApplicationState<B: BlocklistBackend> {
    manager: BlocklistManager<B>,
    template_engine: TemplateEngine,
}

pub fn build<B: BlocklistBackend + Clone + 'static>(
    manager: BlocklistManager<B>,
    template_engine: TemplateEngine,
    listener: TcpListener,
) -> Server {
    Server::new(build_router(manager, template_engine), listener)
}

pub fn build_router<B: BlocklistBackend + Clone + 'static>(
    manager: BlocklistManager<B>,
    template_engine: TemplateEngine,
) -> Router {
    let state = ApplicationState {
        manager,
        template_engine,
    };

    Router::new()
        .route("/", get(index))
        .route("/block", post(block_domain))
        .route("/health", get(health_check))
        .route(
            "/api/v1/blocklist",
            get(get_blocked_domains).put(add_blocked_domain),
        )
        .with_state(state)
}

async fn health_check() -> &'static str {
    "OK"
}

/// Normalises user input into the form stored in the database, or `None` if it is not usable.
fn normalize_domain(input: &str) -> Option<String> {
    let domain = input.trim().trim_end_matches('.').to_lowercase();

    if domain.is_empty() || domain.contains(char::is_whitespace) {
        return None;
    }

    Some(domain)
}

async fn index<B: BlocklistBackend + Clone + 'static>(
    State(state): State<ApplicationState<B>>,
) -> ServerResult<RenderedTemplate> {
    let domains = state.manager.read().await?;
    let context = IndexContext::new(domains);

    Ok(state
        .template_engine
        .render_serialized("index.tera.html", &context)?)
}

#[derive(Deserialize)]
struct BlockDomainForm {
    domain: String,
}

async fn block_domain<B: BlocklistBackend + Clone + 'static>(
    State(state): State<ApplicationState<B>>,
    Form(form): Form<BlockDomainForm>,
) -> ServerResult<Response> {
    let Some(domain) = normalize_domain(&form.domain) else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    state
        .manager
        .update(&domain, DomainEventType::Blocked)
        .await?;

    Ok(redirect("/"))
}

fn redirect(path: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, path)
        .body(Body::empty())
        .expect("static redirect response is always valid")
}

async fn get_blocked_domains<B: BlocklistBackend + Clone + 'static>(
    State(state): State<ApplicationState<B>>,
) -> ServerResult<Json<HashSet<String>>> {
    let blocked_domains = state.manager.read().await?;

    Ok(Json(blocked_domains))
}

#[derive(Deserialize)]
struct BlockedDomainPayload {
    domain: String,
}

async fn add_blocked_domain<B: BlocklistBackend + Clone + 'static>(
    State(state): State<ApplicationState<B>>,
    Json(payload): Json<BlockedDomainPayload>,
) -> ServerResult<Response> {
    let Some(domain) = normalize_domain(&payload.domain) else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    state
        .manager
        .update(&domain, DomainEventType::Blocked)
        .await?;

    Ok(StatusCode::OK.into_response())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    use axum::body::Body;
    use axum::http::header::{CONTENT_TYPE, LOCATION};
    use axum::http::{Method, Request, StatusCode};
    use axum::response::Response;
    use color_eyre::eyre::Result;
    use foundation_templating::TemplateEngine;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::blocklist::{BlocklistBackend, BlocklistManager};
    use crate::http_server::{build_router, normalize_domain};
    use crate::persistence::DomainEventType;

    #[derive(Clone, Default)]
    struct SharedBackend {
        domains: Arc<Mutex<HashSet<String>>>,
    }

    #[async_trait::async_trait]
    impl BlocklistBackend for SharedBackend {
        async fn read(&self) -> Result<HashSet<String>> {
            Ok(self.domains.lock().unwrap().clone())
        }

        async fn update(&self, domain: &str, _: DomainEventType) -> Result<()> {
            self.domains.lock().unwrap().insert(domain.to_string());
            Ok(())
        }
    }

    async fn router_with(domains: &[&str]) -> Result<(axum::Router, SharedBackend)> {
        let backend = SharedBackend::default();
        backend
            .domains
            .lock()
            .unwrap()
            .extend(domains.iter().map(|d| d.to_string()));

        let manager = BlocklistManager::new(backend.clone(), HashSet::new()).await?;
        let router = build_router(manager, TemplateEngine::new()?);

        Ok((router, backend))
    }

    async fn body_of(response: Response) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn form(body: &'static str) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri("/block")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap()
    }

    #[test]
    fn normalizes_domains() {
        assert_eq!(
            normalize_domain(" Example.COM. "),
            Some("example.com".into())
        );
        assert_eq!(normalize_domain(""), None);
        assert_eq!(normalize_domain("   "), None);
        assert_eq!(normalize_domain("a b.com"), None);
    }

    #[tokio::test]
    async fn index_lists_domains_in_sorted_order() -> Result<()> {
        let (router, _) = router_with(&["zeta.com", "alpha.com"]).await?;

        let response = router
            .oneshot(Request::get("/").body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let body = body_of(response).await;
        let alpha = body.find("alpha.com").expect("alpha.com is listed");
        let zeta = body.find("zeta.com").expect("zeta.com is listed");
        assert!(alpha < zeta);

        Ok(())
    }

    #[tokio::test]
    async fn index_shows_empty_state() -> Result<()> {
        let (router, _) = router_with(&[]).await?;

        let response = router
            .oneshot(Request::get("/").body(Body::empty())?)
            .await?;

        assert!(body_of(response).await.contains("No domains blocked"));

        Ok(())
    }

    #[tokio::test]
    async fn blocking_a_domain_normalizes_and_redirects() -> Result<()> {
        let (router, backend) = router_with(&[]).await?;

        let response = router.oneshot(form("domain=Example.COM.")).await?;

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(response.headers().get(LOCATION).unwrap(), "/");
        assert!(backend.domains.lock().unwrap().contains("example.com"));

        Ok(())
    }

    #[tokio::test]
    async fn blocking_an_empty_domain_is_rejected() -> Result<()> {
        let (router, backend) = router_with(&[]).await?;

        let response = router.oneshot(form("domain=+")).await?;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(backend.domains.lock().unwrap().is_empty());

        Ok(())
    }
}
