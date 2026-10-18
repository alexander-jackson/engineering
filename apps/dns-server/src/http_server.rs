use std::collections::HashSet;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Form, Path, State};
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
use crate::classifier::{DecisionStore, Prompt, Threshold};
use crate::persistence::DomainEventType;
use crate::templates::{IndexContext, QuestionsContext};
use crate::uid::QuestionUid;

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
    questions: Arc<dyn DecisionStore>,
    template_engine: TemplateEngine,
}

pub fn build<B: BlocklistBackend + Clone + 'static>(
    manager: BlocklistManager<B>,
    questions: Arc<dyn DecisionStore>,
    template_engine: TemplateEngine,
    listener: TcpListener,
) -> Server {
    Server::new(build_router(manager, questions, template_engine), listener)
}

pub fn build_router<B: BlocklistBackend + Clone + 'static>(
    manager: BlocklistManager<B>,
    questions: Arc<dyn DecisionStore>,
    template_engine: TemplateEngine,
) -> Router {
    let state = ApplicationState {
        manager,
        questions,
        template_engine,
    };

    Router::new()
        .route("/", get(index))
        .route("/block", post(block_domain))
        .route("/questions", get(list_questions).post(add_question))
        .route("/questions/{uid}/retire", post(retire_question))
        .route("/questions/{uid}/replace", post(replace_question))
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

async fn list_questions<B: BlocklistBackend + Clone + 'static>(
    State(state): State<ApplicationState<B>>,
) -> ServerResult<RenderedTemplate> {
    let context = QuestionsContext::new(state.questions.questions().await?);

    Ok(state
        .template_engine
        .render_serialized("questions.tera.html", &context)?)
}

#[derive(Deserialize)]
struct QuestionForm {
    prompt: Prompt,
    threshold: Threshold,
}

async fn add_question<B: BlocklistBackend + Clone + 'static>(
    State(state): State<ApplicationState<B>>,
    Form(form): Form<QuestionForm>,
) -> ServerResult<Response> {
    let uid = state
        .questions
        .add_question(&form.prompt, form.threshold)
        .await?;
    tracing::info!(%uid, threshold = %form.threshold, "added classifier question");

    Ok(redirect("/questions"))
}

async fn retire_question<B: BlocklistBackend + Clone + 'static>(
    State(state): State<ApplicationState<B>>,
    Path(uid): Path<QuestionUid>,
) -> ServerResult<Response> {
    if !state.questions.retire_question(uid).await? {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    tracing::info!(%uid, "retired classifier question");

    Ok(redirect("/questions"))
}

/// Questions are immutable, so changing one marks it as redundant and adds a new question in its
/// place.
async fn replace_question<B: BlocklistBackend + Clone + 'static>(
    State(state): State<ApplicationState<B>>,
    Path(uid): Path<QuestionUid>,
    Form(form): Form<QuestionForm>,
) -> ServerResult<Response> {
    let Some(replacement) = state
        .questions
        .replace_question(uid, &form.prompt, form.threshold)
        .await?
    else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    tracing::info!(%uid, %replacement, threshold = %form.threshold, "replaced classifier question");

    Ok(redirect("/questions"))
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
    use crate::classifier::{DecisionStore, Prompt, Question, Threshold};
    use crate::http_server::{build_router, normalize_domain};
    use crate::persistence::DomainEventType;
    use crate::uid::QuestionUid;

    #[derive(Default)]
    struct MemoryQuestions {
        questions: Mutex<Vec<Question>>,
    }

    #[async_trait::async_trait]
    impl DecisionStore for MemoryQuestions {
        async fn questions(&self) -> Result<Vec<Question>> {
            Ok(self.questions.lock().unwrap().clone())
        }

        async fn decisions(&self) -> Result<std::collections::HashMap<String, bool>> {
            Ok(Default::default())
        }

        async fn record(&self, _: &str, _: bool, _: &[(QuestionUid, f64)]) -> Result<()> {
            Ok(())
        }

        async fn add_question(&self, prompt: &Prompt, threshold: Threshold) -> Result<QuestionUid> {
            let uid = QuestionUid::new();
            self.questions.lock().unwrap().push(Question {
                uid,
                prompt: prompt.clone(),
                threshold,
            });

            Ok(uid)
        }

        async fn retire_question(&self, uid: QuestionUid) -> Result<bool> {
            let mut questions = self.questions.lock().unwrap();
            let before = questions.len();
            questions.retain(|q| q.uid != uid);

            Ok(questions.len() != before)
        }

        async fn replace_question(
            &self,
            uid: QuestionUid,
            prompt: &Prompt,
            threshold: Threshold,
        ) -> Result<Option<QuestionUid>> {
            if !self.retire_question(uid).await? {
                return Ok(None);
            }

            self.add_question(prompt, threshold).await.map(Some)
        }
    }

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
        let router = build_router(
            manager,
            Arc::new(MemoryQuestions::default()),
            TemplateEngine::new()?,
        );

        Ok((router, backend))
    }

    async fn router_with_questions() -> Result<(axum::Router, Arc<MemoryQuestions>)> {
        let backend = SharedBackend::default();
        let manager = BlocklistManager::new(backend, HashSet::new()).await?;
        let questions = Arc::new(MemoryQuestions::default());
        let router = build_router(manager, questions.clone(), TemplateEngine::new()?);

        Ok((router, questions))
    }

    async fn add(questions: &MemoryQuestions, prompt: &str, threshold: f64) -> Result<QuestionUid> {
        questions
            .add_question(&Prompt::try_from(prompt)?, Threshold::try_from(threshold)?)
            .await
    }

    fn post_form(uri: &str, body: &'static str) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap()
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

    #[tokio::test]
    async fn questions_page_shows_empty_state() -> Result<()> {
        let (router, _) = router_with_questions().await?;

        let response = router
            .oneshot(Request::get("/questions").body(Body::empty())?)
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_of(response).await.contains("No active questions"));

        Ok(())
    }

    #[tokio::test]
    async fn adding_a_question_stores_it_and_lists_it() -> Result<()> {
        let (router, questions) = router_with_questions().await?;

        let response = router
            .clone()
            .oneshot(post_form(
                "/questions",
                "prompt=+Is+this+a+proxy%3F+&threshold=0.8",
            ))
            .await?;

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(response.headers().get(LOCATION).unwrap(), "/questions");

        let stored = questions.questions.lock().unwrap().clone();
        assert_eq!(stored.len(), 1);
        assert_eq!(&*stored[0].prompt, "Is this a proxy?");
        assert_eq!(*stored[0].threshold, 0.8);

        let response = router
            .oneshot(Request::get("/questions").body(Body::empty())?)
            .await?;
        assert!(body_of(response).await.contains("Is this a proxy?"));

        Ok(())
    }

    #[tokio::test]
    async fn invalid_questions_are_rejected() -> Result<()> {
        let (router, questions) = router_with_questions().await?;

        for body in [
            "prompt=+&threshold=0.5",
            "prompt=ok&threshold=1.5",
            "prompt=ok&threshold=-0.1",
            "prompt=ok&threshold=NaN",
            "prompt=ok&threshold=abc",
        ] {
            let response = router
                .clone()
                .oneshot(post_form("/questions", body))
                .await?;
            assert_eq!(
                response.status(),
                StatusCode::UNPROCESSABLE_ENTITY,
                "{body}"
            );
        }

        assert!(questions.questions.lock().unwrap().is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn retiring_a_question_removes_it() -> Result<()> {
        let (router, questions) = router_with_questions().await?;
        let uid = add(&questions, "q", 0.5).await?;

        let response = router
            .clone()
            .oneshot(post_form(&format!("/questions/{uid}/retire"), ""))
            .await?;
        assert_eq!(response.status(), StatusCode::FOUND);
        assert!(questions.questions.lock().unwrap().is_empty());

        let again = router
            .clone()
            .oneshot(post_form(&format!("/questions/{uid}/retire"), ""))
            .await?;
        assert_eq!(again.status(), StatusCode::NOT_FOUND);

        let invalid = router
            .oneshot(post_form("/questions/not-a-uid/retire", ""))
            .await?;
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

        Ok(())
    }

    #[tokio::test]
    async fn replacing_a_question_swaps_its_definition() -> Result<()> {
        let (router, questions) = router_with_questions().await?;
        let uid = add(&questions, "old", 0.5).await?;

        let response = router
            .clone()
            .oneshot(post_form(
                &format!("/questions/{uid}/replace"),
                "prompt=new&threshold=0.9",
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::FOUND);

        let stored = questions.questions.lock().unwrap().clone();
        assert_eq!(stored.len(), 1);
        assert_ne!(stored[0].uid, uid);
        assert_eq!(&*stored[0].prompt, "new");
        assert_eq!(*stored[0].threshold, 0.9);

        let missing = router
            .oneshot(post_form(
                &format!("/questions/{}/replace", QuestionUid::new()),
                "prompt=new&threshold=0.9",
            ))
            .await?;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        Ok(())
    }
}
