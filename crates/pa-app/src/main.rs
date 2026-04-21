use std::{path::Path, sync::Arc, time::Duration};

use anyhow::Result;
use pa_api::{AppState, MarketRuntime, app_router};
use pa_instrument::InstrumentRepository;
use pa_market::{
    PgCanonicalKlineRepository, ProviderRouter,
    provider::providers::{EastMoneyProvider, TwelveDataProvider},
};
use pa_orchestrator::{Executor, FixtureLlmClient, PromptRegistry, run_single_task};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = pa_core::config::load()?;
    let bind_addr = config.server_addr.clone();
    let migration_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    sqlx::migrate::Migrator::new(migration_dir.as_path())
        .await?
        .run(&pool)
        .await?;
    let instrument_repository = InstrumentRepository::new(pool.clone());
    let canonical_kline_repository = Arc::new(PgCanonicalKlineRepository::new(pool));
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(EastMoneyProvider::new(&config.eastmoney_base_url)));
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        &config.twelvedata_base_url,
        &config.twelvedata_api_key,
    )));
    let market_runtime = Arc::new(MarketRuntime::new(
        instrument_repository,
        canonical_kline_repository,
        Arc::new(provider_router),
    ));
    let state = AppState::with_market_runtime(config.server_addr, market_runtime);
    let worker_repository = Arc::clone(&state.orchestration_repository);
    let prompt_registry = PromptRegistry::default()
        .with_spec(pa_analysis::shared_bar_analysis_v1())?
        .with_spec(pa_analysis::shared_daily_context_v1())?
        .with_spec(pa_user::user_position_advice_v1())?;
    let worker_executor = Executor::new(
        prompt_registry,
        FixtureLlmClient::with_json(fixture_worker_output_json()),
    );
    let app = app_router(state);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    tracing::info!(address = %bind_addr, "pa-app listening");
    tracing::info!("market runtime configured with PostgreSQL + provider router");
    tracing::info!("phase2 analysis worker started with fixture llm transport");

    tokio::spawn(async move {
        run_analysis_worker(worker_repository, worker_executor).await;
    });

    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_analysis_worker(
    repository: Arc<pa_orchestrator::InMemoryOrchestrationRepository>,
    executor: Executor<FixtureLlmClient>,
) {
    loop {
        match run_single_task(repository.as_ref(), &executor).await {
            Ok(true) => {}
            Ok(false) => tokio::time::sleep(Duration::from_millis(250)).await,
            Err(err) => {
                tracing::error!(error = %err, "phase2 analysis worker iteration failed");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

fn fixture_worker_output_json() -> serde_json::Value {
    json!({
        "bar_state": "closed",
        "bar_classification": {},
        "bullish_case": {},
        "bearish_case": {},
        "two_sided_summary": {},
        "nearby_levels": {},
        "signal_strength": {},
        "continuation_scenarios": {},
        "reversal_scenarios": {},
        "invalidation_levels": {},
        "execution_bias_notes": {},
        "market_background": {},
        "market_structure": {},
        "key_support_levels": {},
        "key_resistance_levels": {},
        "signal_bars": {},
        "candle_patterns": {},
        "decision_tree_nodes": {
            "trend_context": {},
            "location_context": {},
            "signal_quality": {},
            "confirmation_state": {},
            "invalidation_conditions": {}
        },
        "liquidity_context": {},
        "risk_notes": {},
        "scenario_map": {},
        "position_state": {},
        "market_read_through": {},
        "bullish_path_for_user": {},
        "bearish_path_for_user": {},
        "hold_reduce_exit_conditions": {},
        "risk_control_levels": {},
        "invalidations": {},
        "action_candidates": {}
    })
}
