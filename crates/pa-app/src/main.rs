use std::{path::Path, sync::Arc, time::Duration};

use anyhow::Result;
use pa_api::{AppState, MarketRuntime, app_router};
use pa_app::build_worker_executor_from_config;
use pa_instrument::InstrumentRepository;
use pa_market::{
    MarketGateway, PgCanonicalKlineRepository, ProviderRouter,
    provider::providers::{EastMoneyProvider, TwelveDataProvider},
};
use pa_orchestrator::{
    Executor, OpenAiCompatibleClient, OrchestrationRepository, PgOrchestrationRepository,
    run_single_task,
};
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
    let orchestration_repository: Arc<dyn OrchestrationRepository> =
        Arc::new(PgOrchestrationRepository::new(pool.clone()));
    orchestration_repository
        .recover_stale_running_tasks(
            chrono::Utc::now() - chrono::Duration::minutes(5),
            "worker_recovered_on_startup",
            "startup recovery returned stale running task to retry_waiting",
        )
        .await?;
    let instrument_repository = InstrumentRepository::new(pool.clone());
    let canonical_kline_repository = Arc::new(PgCanonicalKlineRepository::new(pool.clone()));
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(EastMoneyProvider::new(&config.eastmoney_base_url)));
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        &config.twelvedata_base_url,
        &config.twelvedata_api_key,
    )));
    let market_gateway = Arc::new(MarketGateway::new(provider_router));
    let market_runtime = Arc::new(MarketRuntime::new(
        instrument_repository,
        canonical_kline_repository,
        market_gateway,
    ));
    let worker_executor = build_worker_executor_from_config(&config)?;
    let state = AppState::with_dependencies(
        config.server_addr.clone(),
        Arc::clone(&orchestration_repository),
        Some(market_runtime),
    );
    let worker_repository = Arc::clone(&orchestration_repository);
    let app = app_router(state);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    tracing::info!(address = %bind_addr, "pa-app listening");
    tracing::info!("market runtime configured with PostgreSQL + provider router");
    tracing::info!("phase2 analysis worker started with OpenAI-compatible llm transport");

    tokio::spawn(async move {
        run_analysis_worker(worker_repository, worker_executor).await;
    });

    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_analysis_worker(
    repository: Arc<dyn pa_orchestrator::OrchestrationRepository>,
    executor: Executor<OpenAiCompatibleClient>,
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
