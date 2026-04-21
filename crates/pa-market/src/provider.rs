use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use pa_core::{AppError, Timeframe};

use crate::models::{ProviderKline, ProviderTick};

#[path = "providers/mod.rs"]
pub mod providers;

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

pub type ProviderMap = HashMap<&'static str, Arc<dyn MarketDataProvider>>;

#[derive(Debug, Clone, PartialEq)]
pub struct RoutedKlines {
    pub provider_name: String,
    pub klines: Vec<ProviderKline>,
}

#[derive(Default)]
pub struct ProviderRouter {
    providers: ProviderMap,
}

impl ProviderRouter {
    pub fn new(providers: ProviderMap) -> Self {
        Self { providers }
    }

    pub fn insert(
        &mut self,
        provider: Arc<dyn MarketDataProvider>,
    ) -> Option<Arc<dyn MarketDataProvider>> {
        self.providers.insert(provider.name(), provider)
    }

    pub fn provider(&self, name: &str) -> Option<Arc<dyn MarketDataProvider>> {
        self.providers.get(name).map(Arc::clone)
    }

    pub async fn fetch_klines_with_fallback(
        &self,
        primary: &str,
        fallback: &str,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let routed = self
            .fetch_klines_with_fallback_source(primary, fallback, provider_symbol, timeframe, limit)
            .await?;

        Ok(routed.klines)
    }

    pub async fn fetch_klines_with_fallback_source(
        &self,
        primary: &str,
        fallback: &str,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<RoutedKlines, AppError> {
        let primary_result = self
            .fetch_klines_from(primary, provider_symbol, timeframe, limit)
            .await;

        match primary_result {
            Ok(klines) if !klines.is_empty() => Ok(RoutedKlines {
                provider_name: primary.to_string(),
                klines,
            }),
            Ok(_) => self
                .fetch_klines_from(fallback, provider_symbol, timeframe, limit)
                .await
                .map(|klines| RoutedKlines {
                    provider_name: fallback.to_string(),
                    klines,
                }),
            Err(AppError::Validation { .. }) => {
                Err(primary_result.expect_err("primary result is validation error"))
            }
            Err(_) => self
                .fetch_klines_from(fallback, provider_symbol, timeframe, limit)
                .await
                .map(|klines| RoutedKlines {
                    provider_name: fallback.to_string(),
                    klines,
                }),
        }
    }

    async fn fetch_klines_from(
        &self,
        provider_name: &str,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let provider = self
            .provider(provider_name)
            .ok_or_else(|| AppError::Validation {
                message: format!("provider `{provider_name}` is not registered"),
                source: None,
            })?;

        provider
            .fetch_klines(provider_symbol, timeframe, limit)
            .await
    }
}
