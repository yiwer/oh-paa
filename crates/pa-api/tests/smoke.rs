use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use pa_api::{AppState, app_router};
use tower::ServiceExt;

#[tokio::test]
async fn healthz_and_grouped_routes_are_wired() {
    let app = app_router(AppState::new("127.0.0.1:0"));

    let healthz = request(&app, "/healthz").await;
    assert_eq!(healthz.status(), StatusCode::OK);
    assert_eq!(response_text(healthz).await, "ok");

    assert_placeholder(&app, "/admin", "admin routes pending").await;
    assert_placeholder(&app, "/market", "market routes pending").await;
    assert_placeholder(&app, "/analysis", "analysis routes pending").await;
    assert_placeholder(&app, "/user", "user routes pending").await;
}

async fn assert_placeholder(app: &axum::Router, uri: &str, expected_body: &str) {
    let response = request(app, uri).await;

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(response_text(response).await, expected_body);
}

async fn request(app: &axum::Router, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
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
