use std::sync::Arc;

use axum::{Router, routing::get};
use pa_instrument::InstrumentRepository;
use pa_market::{CanonicalKlineRepository, ProviderRouter};
use pa_orchestrator::InMemoryOrchestrationRepository;

use crate::{admin, analysis, market, user};

#[derive(Clone)]
pub struct MarketRuntime {
    pub instrument_repository: InstrumentRepository,
    pub canonical_kline_repository: Arc<dyn CanonicalKlineRepository>,
    pub provider_router: Arc<ProviderRouter>,
}

impl MarketRuntime {
    pub fn new(
        instrument_repository: InstrumentRepository,
        canonical_kline_repository: Arc<dyn CanonicalKlineRepository>,
        provider_router: Arc<ProviderRouter>,
    ) -> Self {
        Self {
            instrument_repository,
            canonical_kline_repository,
            provider_router,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub server_addr: String,
    pub orchestration_repository: Arc<InMemoryOrchestrationRepository>,
    pub market_runtime: Option<Arc<MarketRuntime>>,
}

impl AppState {
    pub fn new(server_addr: impl Into<String>) -> Self {
        Self::with_dependencies(
            server_addr,
            Arc::new(InMemoryOrchestrationRepository::default()),
            None,
        )
    }

    pub fn with_dependencies(
        server_addr: impl Into<String>,
        orchestration_repository: Arc<InMemoryOrchestrationRepository>,
        market_runtime: Option<Arc<MarketRuntime>>,
    ) -> Self {
        Self {
            server_addr: server_addr.into(),
            orchestration_repository,
            market_runtime,
        }
    }

    pub fn with_market_runtime(
        server_addr: impl Into<String>,
        market_runtime: Arc<MarketRuntime>,
    ) -> Self {
        Self::with_dependencies(
            server_addr,
            Arc::new(InMemoryOrchestrationRepository::default()),
            Some(market_runtime),
        )
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
