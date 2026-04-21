use std::sync::Arc;

use axum::{Router, routing::get};
use pa_orchestrator::InMemoryOrchestrationRepository;

use crate::{admin, analysis, market, user};

#[derive(Debug, Clone)]
pub struct AppState {
    pub server_addr: String,
    pub orchestration_repository: Arc<InMemoryOrchestrationRepository>,
}

impl AppState {
    pub fn new(server_addr: impl Into<String>) -> Self {
        Self::with_repository(
            server_addr,
            Arc::new(InMemoryOrchestrationRepository::default()),
        )
    }

    pub fn with_repository(
        server_addr: impl Into<String>,
        orchestration_repository: Arc<InMemoryOrchestrationRepository>,
    ) -> Self {
        Self {
            server_addr: server_addr.into(),
            orchestration_repository,
        }
    }

    pub fn fixture() -> Self {
        Self::new("127.0.0.1:0")
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
