use axum::Router;
use axum::body::Body;
use axum::http::header::{AsHeaderName, CONTENT_TYPE, LOCATION};
use axum::http::{Method, Request, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

use crate::templates::TemplateEngine;

const FORM_MIME_TYPE: &str = "application/x-www-form-urlencoded";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn build_router(pool: PgPool) -> Result<Router> {
    let template_engine = TemplateEngine::new()?;
    let router = crate::server::build_router(template_engine, pool);

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
async fn index_returns_200(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;
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

    Ok(())
}

#[sqlx::test]
async fn can_add_locker(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("locker_number=42&bag_type=PeakDesign30L"))?;

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

    let body = read_full_body(response).await?;

    // Check that the locker appears in the response
    assert!(body.contains("#42"));
    assert!(body.contains("Peak Design 30L"));

    Ok(())
}

#[sqlx::test]
async fn can_remove_locker(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    // First add a locker
    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("locker_number=100&bag_type=StubbleAndCo20L"))?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::FOUND);

    // Now remove it
    let request = Request::builder()
        .method(Method::POST)
        .uri("/remove/100")
        .body(Body::empty())?;

    let response = router.clone().oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(get_response_header(&response, LOCATION), Some("/"));

    // Verify it's gone
    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;
    let body = read_full_body(response).await?;

    // Should show empty state
    assert!(body.contains("No bags checked in"));

    Ok(())
}

#[sqlx::test]
async fn duplicate_locker_number_fails(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    // Add first locker
    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("locker_number=50&bag_type=PeakDesign30L"))?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::FOUND);

    // Try to add same locker number again
    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("locker_number=50&bag_type=StubbleAndCo20L"))?;

    let response = router.clone().oneshot(request).await?;

    // Should redirect with error message about occupied locker
    assert_eq!(response.status(), StatusCode::FOUND);
    let location = get_response_header(&response, LOCATION);
    assert!(location.is_some());
    let location_str = location.unwrap();
    assert!(location_str.starts_with("/?error="));
    assert!(location_str.contains("50")); // Locker number should be in error message

    Ok(())
}

#[sqlx::test]
async fn duplicate_bag_type_fails(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    // Add first locker with Peak Design bag
    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("locker_number=100&bag_type=PeakDesign30L"))?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::FOUND);

    // Try to add same bag type to a different locker
    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("locker_number=200&bag_type=PeakDesign30L"))?;

    let response = router.oneshot(request).await?;

    // Should redirect with error message about bag already checked in
    assert_eq!(response.status(), StatusCode::FOUND);
    let location = get_response_header(&response, LOCATION);
    assert!(location.is_some());
    let location_str = location.unwrap();
    assert!(location_str.starts_with("/?error="));
    assert!(location_str.contains("Peak") || location_str.contains("already")); // Bag name or "already" should be in error

    Ok(())
}

#[sqlx::test]
async fn cannot_checkout_unoccupied_locker(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    // Try to check out a locker that was never checked in
    let request = Request::builder()
        .method(Method::POST)
        .uri("/remove/999")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;

    // Should redirect with error message
    assert_eq!(response.status(), StatusCode::FOUND);
    let location = get_response_header(&response, LOCATION);
    assert!(location.is_some());
    let location_str = location.unwrap();
    assert!(location_str.starts_with("/?error="));
    assert!(location_str.contains("not%20occupied") || location_str.contains("999"));

    Ok(())
}

#[sqlx::test]
async fn can_check_in_again_after_checkout(pool: PgPool) -> Result<()> {
    let router = build_router(pool)?;

    // Check in a bag
    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("locker_number=75&bag_type=PeakDesign30L"))?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::FOUND);

    // Check it out
    let request = Request::builder()
        .method(Method::POST)
        .uri("/remove/75")
        .body(Body::empty())?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::FOUND);

    // Check in the same bag again (should work now)
    let request = Request::builder()
        .method(Method::POST)
        .uri("/add")
        .header(CONTENT_TYPE, FORM_MIME_TYPE)
        .body(Body::from("locker_number=80&bag_type=PeakDesign30L"))?;

    let response = router.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::FOUND);

    // Verify it's checked in at the new locker
    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())?;

    let response = router.oneshot(request).await?;
    let body = read_full_body(response).await?;

    assert!(body.contains("#80"));
    assert!(body.contains("Peak Design 30L"));
    assert!(!body.contains("#75")); // Old locker should not appear

    Ok(())
}
