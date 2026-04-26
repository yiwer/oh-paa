use std::sync::Arc;

use axum::{Router, routing::get};
use pa_instrument::InstrumentRepository;
use pa_market::{CanonicalKlineRepository, MarketGateway};
use pa_orchestrator::{InMemoryOrchestrationRepository, OrchestrationRepository};

use crate::{admin, analysis, market, user};

#[derive(Clone)]
pub struct MarketRuntime {
    pub instrument_repository: InstrumentRepository,
    pub canonical_kline_repository: Arc<dyn CanonicalKlineRepository>,
    pub market_gateway: Arc<MarketGateway>,
}

impl MarketRuntime {
    pub fn new(
        instrument_repository: InstrumentRepository,
        canonical_kline_repository: Arc<dyn CanonicalKlineRepository>,
        market_gateway: Arc<MarketGateway>,
    ) -> Self {
        Self {
            instrument_repository,
            canonical_kline_repository,
            market_gateway,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub server_addr: String,
    pub orchestration_repository: Arc<dyn OrchestrationRepository>,
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
        orchestration_repository: Arc<dyn OrchestrationRepository>,
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
