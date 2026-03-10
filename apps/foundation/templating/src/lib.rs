use axum::body::Body;
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use color_eyre::eyre::Result;
use serde::Serialize;
use tera::{Context, Tera};

#[derive(Clone)]
pub struct TemplateEngine {
    inner: Tera,
}

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        let inner = Tera::new("templates/**.tera.html")?;

        Ok(Self { inner })
    }

    fn render(&self, template: &str, context: &Context) -> Result<RenderedTemplate> {
        let rendered = self.inner.render(template, context)?;

        Ok(RenderedTemplate { inner: rendered })
    }

    pub fn render_serialized<C: Serialize>(
        &self,
        template: &str,
        context: &C,
    ) -> Result<RenderedTemplate> {
        let context = Context::from_serialize(context)?;

        self.render(template, &context)
    }

    pub fn render_contextless(&self, template: &str) -> Result<RenderedTemplate> {
        self.render(template, &Context::default())
    }
}

pub struct RenderedTemplate {
    inner: String,
}

impl IntoResponse for RenderedTemplate {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/html")
            .header(CACHE_CONTROL, "no-store")
            .body(Body::from(self.inner))
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    use super::RenderedTemplate;

    async fn body_string(template: RenderedTemplate) -> String {
        let response = template.into_response();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn rendered_template_returns_200() {
        let template = RenderedTemplate {
            inner: String::new(),
        };
        let response = template.into_response();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn rendered_template_sets_content_type_html() {
        let template = RenderedTemplate {
            inner: String::new(),
        };
        let response = template.into_response();
        assert_eq!(response.headers()["content-type"], "text/html");
    }

    #[tokio::test]
    async fn rendered_template_sets_cache_control_no_store() {
        let template = RenderedTemplate {
            inner: String::new(),
        };
        let response = template.into_response();
        assert_eq!(response.headers()["cache-control"], "no-store");
    }

    #[tokio::test]
    async fn rendered_template_body_contains_rendered_content() {
        let template = RenderedTemplate {
            inner: "<h1>Hello</h1>".to_owned(),
        };
        assert_eq!(body_string(template).await, "<h1>Hello</h1>");
    }
}
