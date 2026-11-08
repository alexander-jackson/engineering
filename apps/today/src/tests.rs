use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::header::{AsHeaderName, CONTENT_TYPE, LOCATION};
use axum::http::{Method, Request, StatusCode};
use axum::response::Response;
use chrono::{Days, Utc};
use foundation_templating::TemplateEngine;
use http_body_util::BodyExt;
use serde_test::{Token, assert_ser_tokens};
use sqlx::PgPool;
use tower::ServiceExt;

use crate::clock;
use crate::persistence::Content;
use crate::server::IndexCache;
use crate::templates::IndexContext;
use crate::uid::ItemUid;

const FORM_MIME_TYPE: &str = "application/x-www-form-urlencoded";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn build_router(pool: PgPool) -> Result<Router> {
    let template_engine = TemplateEngine::new()?;
    let index_cache = IndexCache::new(1);

    let router = crate::server::build_router(template_engine, pool, index_cache);

    Ok(router)
}

async fn read_full_body(response: Response) -> Result<String> {
    let body = response.into_body().collect().await?.to_bytes();
    let message = String::from_utf8(body.to_vec())?;

    Ok(message)
}

fn get_response_header<'a, K: AsHeaderName>(response: &'a Response, header: K) -> Option<&'a str> {
    response.headers().get(header).and_then(|h| h.to_str().ok())
}

#[sqlx::test]
async fn invalid_requests_get_404s(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;
    let request = Request::builder()
        .uri("/unknown-path")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;
    let status = response.status();

    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

#[sqlx::test]
async fn can_add_items(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("content=Task"))?;

    let response = router.clone().oneshot(request).await?;

    // Get redirected to the index page
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(get_response_header(&response, LOCATION), Some("/"));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        get_response_header(&response, CONTENT_TYPE),
        Some("text/html")
    );

    let body = read_full_body(response).await?;

    assert!(body.contains("Task"));

    Ok(())
}

#[sqlx::test]
async fn checked_items_are_rendered_as_checked(pool: PgPool) -> Result<()> {
    let router = build_router(pool.clone())?;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("content=Task"))?;

    router.clone().oneshot(request).await?;

    let today = Utc::now().date_naive();
    let items = crate::persistence::select_items(&pool, today).await?;
    let uid = items[0].item_uid.to_string();

    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/update/{uid}"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"state":"Checked"}"#))?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;
    let body = read_full_body(response).await?;

    // The item should be rendered with its checkbox ticked
    let start = body
        .find(&format!("id=\"{uid}\""))
        .expect("the item should be rendered");
    let rest = &body[start..];
    let checkbox = &rest[..rest.find("/>").expect("the checkbox should close")];

    assert!(
        checkbox.contains("checked"),
        "the completed item was not rendered as checked"
    );

    Ok(())
}

#[sqlx::test]
async fn completed_items_are_collapsed_below_open_items(pool: PgPool) -> Result<()> {
    let router = build_router(pool.clone())?;

    for content in ["content=Done", "content=Open"] {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/add")
            .header(CONTENT_TYPE, FORM_MIME_TYPE)
            .body(Body::from(content))?;

        router.clone().oneshot(request).await?;
    }

    let today = Utc::now().date_naive();
    let items = crate::persistence::select_items(&pool, today).await?;
    let done = items.iter().find(|item| item.source == "Done").unwrap();

    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/update/{}", done.item_uid))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"state":"Checked"}"#))?;

    router.clone().oneshot(request).await?;

    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;
    let body = read_full_body(response).await?;

    let details = body
        .find("<details")
        .expect("completed items should be grouped");
    let open = body
        .find("Open</span")
        .expect("the open item should be rendered");
    let done = body
        .find("Done</span")
        .expect("the completed item should be rendered");

    assert!(
        open < details,
        "the open item was inside the completed group"
    );
    assert!(
        details < done,
        "the completed item was outside the completed group"
    );
    assert!(body.contains("Completed (1)"));

    Ok(())
}

async fn get_index(router: Router, uri: &str) -> Result<Response> {
    let request = Request::builder().uri(uri).body(Body::empty())?;

    Ok(router.oneshot(request).await?)
}

#[sqlx::test]
async fn previous_days_can_be_viewed(pool: PgPool) -> Result<()> {
    let router = build_router(pool.clone())?;
    let yesterday = clock::today() - Days::new(1);
    let created_at = yesterday.and_hms_opt(12, 0, 0).unwrap();

    crate::persistence::create_item(&pool, ItemUid::new(), "Old task", created_at).await?;

    let body = read_full_body(get_index(router.clone(), "/").await?).await?;
    assert!(!body.contains("Old task"));

    let response = get_index(router, &format!("/?date={yesterday}")).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = read_full_body(response).await?;
    assert!(body.contains(">Old task</span"));
    assert!(!body.contains("action=\"/add\""));
    assert!(body.contains("aria-label=\"Next day\""));

    Ok(())
}

#[sqlx::test]
async fn pages_cached_on_an_earlier_day_are_not_reused(pool: PgPool) -> Result<()> {
    let template_engine = TemplateEngine::new()?;
    let index_cache = IndexCache::new(8);
    let router = crate::server::build_router(template_engine, pool, index_cache.clone());

    // What the server would have cached for yesterday, back when it was still today
    let yesterday = clock::today() - Days::new(1);
    let stale = Arc::new(IndexContext::new(yesterday, yesterday, vec![]));
    index_cache.insert((yesterday, yesterday), stale).await;

    let response = get_index(router, &format!("/?date={yesterday}")).await?;
    let body = read_full_body(response).await?;

    assert!(body.contains("aria-label=\"Next day\""));
    assert!(!body.contains("action=\"/add\""));

    Ok(())
}

#[sqlx::test]
async fn today_has_no_next_link_and_keeps_the_add_form(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    let body = read_full_body(get_index(router, "/").await?).await?;

    assert!(body.contains("action=\"/add\""));
    assert!(body.contains("aria-label=\"Previous day\""));
    assert!(!body.contains("aria-label=\"Next day\""));

    Ok(())
}

#[sqlx::test]
async fn days_outside_the_window_are_not_found(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;
    let today = clock::today();

    let earliest = today - Days::new(clock::HISTORY_DAYS);
    let response = get_index(router.clone(), &format!("/?date={earliest}")).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = read_full_body(response).await?;
    assert!(!body.contains("aria-label=\"Previous day\""));

    for date in [earliest - Days::new(1), today + Days::new(1)] {
        let response = get_index(router.clone(), &format!("/?date={date}")).await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    Ok(())
}

#[sqlx::test]
async fn edited_content_is_shown_in_place_of_the_original(pool: PgPool) -> Result<()> {
    let router = build_router(pool.clone())?;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("content=Task"))?;

    router.clone().oneshot(request).await?;

    let today = Utc::now().date_naive();
    let items = crate::persistence::select_items(&pool, today).await?;
    let uid = items[0].item_uid.to_string();

    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/update/{uid}/content"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"content":"Edited task"}"#))?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;
    let body = read_full_body(response).await?;

    assert!(body.contains(">Edited task</span"));
    assert!(!body.contains(">Task</span"));

    Ok(())
}

#[sqlx::test]
async fn blank_edits_are_rejected(pool: PgPool) -> Result<()> {
    let router = build_router(pool.clone())?;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("content=Task"))?;

    router.clone().oneshot(request).await?;

    let today = Utc::now().date_naive();
    let items = crate::persistence::select_items(&pool, today).await?;
    let uid = items[0].item_uid.to_string();

    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/update/{uid}/content"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"content":"   "}"#))?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;
    let body = read_full_body(response).await?;

    assert!(body.contains(">Task</span"));

    Ok(())
}

#[sqlx::test]
async fn raw_source_is_escaped_in_the_edit_attribute(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("content=Say+%22hi%22+%3Cb%3E"))?;

    router.clone().oneshot(request).await?;

    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;
    let body = read_full_body(response).await?;

    assert!(body.contains(r#"data-source="Say &quot;hi&quot; &lt;b&gt;""#));

    Ok(())
}

#[test]
fn item_content_without_any_markdown_should_be_transparently_rendered() {
    let content = Content::from("very normal text".to_string());

    assert_ser_tokens(&content, &[Token::Str("very normal text")]);
}

#[test]
fn can_render_code_blocks_in_item_content() {
    let content = Content::from("some `code` block".to_string());

    assert_ser_tokens(&content, &[Token::Str("some <code>code</code> block")]);
}
