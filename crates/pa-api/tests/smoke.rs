use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use pa_api::{AppState, app_router};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn healthz_and_phase2_analysis_routes_are_wired() {
    let app = app_router(AppState::fixture());

    let healthz = request(&app, "/healthz").await;
    assert_eq!(healthz.status(), StatusCode::OK);
    assert_eq!(response_text(healthz).await, "ok");

    assert_placeholder(&app, "/admin", "admin routes pending").await;
    assert_placeholder(&app, "/market", "market routes pending").await;

    let shared_bar = request_json(
        &app,
        Method::POST,
        "/analysis/shared/bar",
        r#"{
            "instrument_id":"00000000-0000-0000-0000-000000000001",
            "timeframe":"15m",
            "bar_state":"closed",
            "bar_open_time":"2026-04-21T01:45:00Z",
            "bar_close_time":"2026-04-21T02:00:00Z",
            "canonical_bar_json":{"open":1,"close":2},
            "structure_context_json":{"trend":"up"}
        }"#,
    )
    .await;
    assert_eq!(shared_bar.status(), StatusCode::ACCEPTED);
    let shared_bar_json = response_json(shared_bar).await;
    let task_id = shared_bar_json["task_id"]
        .as_str()
        .expect("shared bar task_id should exist")
        .to_string();
    assert_eq!(shared_bar_json["status"], "pending");
    assert_eq!(shared_bar_json["dedupe_hit"], false);

    let task = request(&app, &format!("/analysis/tasks/{task_id}")).await;
    assert_eq!(task.status(), StatusCode::OK);
    let task_json = response_json(task).await;
    assert_eq!(task_json["task_id"], task_id);
    assert_eq!(task_json["task_type"], "shared_bar_analysis");

    let manual_user = request_json(
        &app,
        Method::POST,
        "/user/analysis/manual",
        r#"{
            "user_id":"00000000-0000-0000-0000-000000000101",
            "instrument_id":"00000000-0000-0000-0000-000000000001",
            "timeframe":"15m",
            "bar_state":"closed",
            "bar_open_time":"2026-04-21T01:45:00Z",
            "bar_close_time":"2026-04-21T02:00:00Z",
            "trading_date":"2026-04-21",
            "positions_json":[],
            "subscriptions_json":[],
            "shared_bar_analysis_json":{"bullish_case":{},"bearish_case":{}},
            "shared_daily_context_json":{"decision_tree_nodes":{}}
        }"#,
    )
    .await;
    assert_eq!(manual_user.status(), StatusCode::ACCEPTED);
    let manual_user_json = response_json(manual_user).await;
    assert_eq!(manual_user_json["status"], "pending");
    assert_eq!(manual_user_json["dedupe_hit"], false);
}

async fn assert_placeholder(app: &axum::Router, uri: &str, expected_body: &str) {
    let response = request(app, uri).await;

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(response_text(response).await, expected_body);
}

async fn request(app: &axum::Router, uri: &str) -> axum::response::Response {
    request_with_body(app, Method::GET, uri, Body::empty()).await
}

async fn request_json(
    app: &axum::Router,
    method: Method,
    uri: &str,
    body: &str,
) -> axum::response::Response {
    request_with_body(app, method, uri, Body::from(body.to_string())).await
}

async fn request_with_body(
    app: &axum::Router,
    method: Method,
    uri: &str,
    body: Body,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(body)
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

async fn response_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");

    String::from_utf8(bytes.to_vec()).expect("body should be utf-8")
}

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_str(&response_text(response).await).expect("body should be valid json")
}
