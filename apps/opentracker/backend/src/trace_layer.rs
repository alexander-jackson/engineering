use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::header::{AUTHORIZATION, HOST, USER_AGENT};
use axum::http::{Request, Response};
use tower::{Layer, Service};
use tracing::instrument::{Instrument, Instrumented};

use crate::auth::Claims;

#[derive(Clone, Default)]
pub struct TraceLayer;

impl<S> Layer<S> for TraceLayer
where
    S: Service<Request<Body>> + Clone + Send + 'static,
{
    type Service = TraceLayerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceLayerService { inner }
    }
}

#[derive(Clone)]
pub struct TraceLayerService<S> {
    inner: S,
}

impl<S, B> Service<Request<Body>> for TraceLayerService<S>
where
    S: Service<Request<Body>, Response = Response<B>> + Clone + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = RequestFuture<Instrumented<S::Future>, B, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let user_agent = req
            .headers()
            .get(USER_AGENT)
            .and_then(|s| s.to_str().ok())
            .unwrap_or("");

        let host = req
            .headers()
            .get(HOST)
            .and_then(|s| s.to_str().ok())
            .unwrap_or("");

        let id = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| Claims::from_authorization_header(Some(v)).ok())
            .map(|claims| claims.id);

        let span = tracing::info_span!(
            "http request",
            method = %req.method(),
            url = %req.uri(),
            status_code = tracing::field::Empty,
            user_agent = &user_agent,
            host = &host,
            user_id = ?id,
        );

        let fut = {
            let _guard = span.enter();
            self.inner.call(req)
        };

        RequestFuture {
            inner: fut.instrument(span.clone()),
            span,
        }
    }
}

pin_project_lite::pin_project! {
    pub struct RequestFuture<F, B, E>
    where
        F: Future<Output = Result<Response<B>, E>>,
    {
        #[pin]
        inner: F,
        span: tracing::Span,
    }
}

impl<F, B, E> Future for RequestFuture<F, B, E>
where
    F: Future<Output = Result<Response<B>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let res = futures::ready!(this.inner.poll(cx));

        if let Ok(ref res) = res {
            this.span.record("status_code", res.status().as_u16());
        }

        res.into()
    }
}
