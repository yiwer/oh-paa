use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};

use crate::router::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(root))
}

async fn root() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "market routes pending")
}
