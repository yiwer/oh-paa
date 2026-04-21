use async_trait::async_trait;
use pa_core::{AppError, Timeframe};

use crate::models::{ProviderKline, ProviderTick};

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn fetch_klines(
        &self,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError>;

    async fn fetch_latest_tick(&self, provider_symbol: &str) -> Result<ProviderTick, AppError>;

    async fn healthcheck(&self) -> Result<(), AppError>;
}
