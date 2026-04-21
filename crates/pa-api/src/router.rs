use axum::{Router, routing::get};

use crate::{admin, analysis, market, user};

#[derive(Debug, Clone)]
pub struct AppState {
    pub server_addr: String,
}

impl AppState {
    pub fn new(server_addr: impl Into<String>) -> Self {
        Self {
            server_addr: server_addr.into(),
        }
    }
}

pub fn app_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .nest("/admin", admin::routes())
        .nest("/market", market::routes())
        .nest("/analysis", analysis::routes())
        .nest("/user", user::routes())
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}
