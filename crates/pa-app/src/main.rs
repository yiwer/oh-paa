use anyhow::Result;
use pa_api::{AppState, app_router};
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
    let state = AppState::new(config.server_addr);
    let app = app_router(state);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    tracing::info!(address = %bind_addr, "pa-app listening");

    axum::serve(listener, app).await?;

    Ok(())
}
