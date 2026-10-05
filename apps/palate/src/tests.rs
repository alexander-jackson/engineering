use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::Utc;
use foundation_templating::TemplateEngine;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

use crate::openrouter::testing::ScriptedModel;
use crate::persistence::Role;
use crate::uid::ConversationUid;

fn router(pool: PgPool, model: ScriptedModel) -> Router {
    crate::server::build_router(TemplateEngine::new().unwrap(), pool, model)
}

async fn body_string(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();

    String::from_utf8(bytes.to_vec()).unwrap()
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

fn form(path: &str, body: &str) -> Request<Body> {
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[sqlx::test]
async fn index_lists_conversations(pool: PgPool) {
    let uid = ConversationUid::new();
    crate::persistence::create_conversation(&pool, uid, &[], Utc::now())
        .await
        .unwrap();

    let router = router(pool, ScriptedModel::failing());
    let response = router.oneshot(get("/")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_string(response).await.contains(&uid.to_string()));
}

#[sqlx::test]
async fn conversation_renders_messages(pool: PgPool) {
    let uid = ConversationUid::new();
    crate::persistence::create_conversation(
        &pool,
        uid,
        &[(Role::User, "hello"), (Role::Assistant, "**bold** idea")],
        Utc::now(),
    )
    .await
    .unwrap();

    let router = router(pool, ScriptedModel::failing());
    let response = router
        .oneshot(get(&format!("/conversations/{uid}")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    assert!(body.contains("<strong>bold</strong> idea"));
    assert!(body.contains(&format!("/conversations/{uid}/messages")));
}

#[sqlx::test]
async fn unknown_conversations_are_not_found(pool: PgPool) {
    let router = router(pool, ScriptedModel::failing());
    let response = router
        .oneshot(get(&format!("/conversations/{}", ConversationUid::new())))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn follow_ups_are_stored_with_the_reply(pool: PgPool) {
    let uid = ConversationUid::new();
    crate::persistence::create_conversation(
        &pool,
        uid,
        &[(Role::System, "sys"), (Role::User, "first")],
        Utc::now(),
    )
    .await
    .unwrap();

    let model = ScriptedModel::replying("reply");
    let router = router(pool.clone(), model.clone());
    let path = format!("/conversations/{uid}/messages");
    let response = router
        .oneshot(form(&path, "content=more+vegetarian+please"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(
        response.headers()[header::LOCATION],
        format!("/conversations/{uid}").as_str()
    );

    let conversation = crate::persistence::select_conversation(&pool, uid)
        .await
        .unwrap()
        .unwrap();

    let stored: Vec<_> = conversation
        .messages
        .iter()
        .map(|m| (m.role, m.content.as_str()))
        .collect();

    assert_eq!(
        stored,
        vec![
            (Role::System, "sys"),
            (Role::User, "first"),
            (Role::User, "more vegetarian please"),
            (Role::Assistant, "reply"),
        ]
    );

    // The whole history is replayed to the model
    assert_eq!(
        model.requests(),
        vec![vec![
            (Role::System, "sys".to_owned()),
            (Role::User, "first".to_owned()),
            (Role::User, "more vegetarian please".to_owned()),
        ]]
    );
}

#[sqlx::test]
async fn blank_follow_ups_do_not_call_the_model(pool: PgPool) {
    let uid = ConversationUid::new();
    crate::persistence::create_conversation(&pool, uid, &[(Role::User, "first")], Utc::now())
        .await
        .unwrap();

    let model = ScriptedModel::replying("reply");
    let router = router(pool, model.clone());
    let path = format!("/conversations/{uid}/messages");
    let response = router.oneshot(form(&path, "content=+++")).await.unwrap();

    assert_eq!(response.status(), StatusCode::FOUND);
    assert!(model.requests().is_empty());
}
