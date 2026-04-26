use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
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

    async fn fetch_klines_window(
        &self,
        query: HistoricalKlineQuery,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let HistoricalKlineQuery {
            provider_symbol,
            timeframe,
            start_open_time,
            end_close_time,
            limit,
        } = query;

        if start_open_time.is_none()
            && end_close_time.is_none()
            && let Some(limit) = limit
        {
            return self.fetch_klines(&provider_symbol, timeframe, limit).await;
        }

        Err(AppError::Validation {
            message: format!(
                "provider `{}` does not support historical window fetch",
                self.name()
            ),
            source: None,
        })
    }

    async fn healthcheck(&self) -> Result<(), AppError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalKlineQuery {
    pub provider_symbol: String,
    pub timeframe: Timeframe,
    pub start_open_time: Option<DateTime<Utc>>,
    pub end_close_time: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

pub type ProviderMap = HashMap<&'static str, Arc<dyn MarketDataProvider>>;

#[derive(Debug, Clone, PartialEq)]
pub struct RoutedKlines {
    pub provider_name: String,
    pub klines: Vec<ProviderKline>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutedTick {
    pub provider_name: String,
    pub tick: ProviderTick,
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
        primary_provider_symbol: &str,
        fallback_provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let routed = self
            .fetch_klines_with_fallback_source(
                primary,
                fallback,
                primary_provider_symbol,
                fallback_provider_symbol,
                timeframe,
                limit,
            )
            .await?;

        Ok(routed.klines)
    }

    pub async fn fetch_klines_with_fallback_source(
        &self,
        primary: &str,
        fallback: &str,
        primary_provider_symbol: &str,
        fallback_provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<RoutedKlines, AppError> {
        let primary_result = self
            .fetch_klines_from(primary, primary_provider_symbol, timeframe, limit)
            .await;

        match primary_result {
            Ok(klines) if !klines.is_empty() => Ok(RoutedKlines {
                provider_name: primary.to_string(),
                klines,
            }),
            Ok(_) => self
                .fetch_klines_from(fallback, fallback_provider_symbol, timeframe, limit)
                .await
                .map(|klines| RoutedKlines {
                    provider_name: fallback.to_string(),
                    klines,
                }),
            Err(AppError::Validation { .. }) => {
                Err(primary_result.expect_err("primary result is validation error"))
            }
            Err(_) => self
                .fetch_klines_from(fallback, fallback_provider_symbol, timeframe, limit)
                .await
                .map(|klines| RoutedKlines {
                    provider_name: fallback.to_string(),
                    klines,
                }),
        }
    }

    pub async fn fetch_latest_tick_with_fallback(
        &self,
        primary: &str,
        fallback: &str,
        primary_provider_symbol: &str,
        fallback_provider_symbol: &str,
    ) -> Result<ProviderTick, AppError> {
        let routed = self
            .fetch_latest_tick_with_fallback_source(
                primary,
                fallback,
                primary_provider_symbol,
                fallback_provider_symbol,
            )
            .await?;

        Ok(routed.tick)
    }

    pub async fn fetch_latest_tick_with_fallback_source(
        &self,
        primary: &str,
        fallback: &str,
        primary_provider_symbol: &str,
        fallback_provider_symbol: &str,
    ) -> Result<RoutedTick, AppError> {
        let primary_result = self
            .fetch_latest_tick_from(primary, primary_provider_symbol)
            .await;

        match primary_result {
            Ok(tick) => Ok(RoutedTick {
                provider_name: primary.to_string(),
                tick,
            }),
            Err(AppError::Validation { .. }) => {
                Err(primary_result.expect_err("primary result is validation error"))
            }
            Err(_) => self
                .fetch_latest_tick_from(fallback, fallback_provider_symbol)
                .await
                .map(|tick| RoutedTick {
                    provider_name: fallback.to_string(),
                    tick,
                }),
        }
    }

    pub(crate) async fn fetch_klines_from(
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

    pub(crate) async fn fetch_latest_tick_from(
        &self,
        provider_name: &str,
        provider_symbol: &str,
    ) -> Result<ProviderTick, AppError> {
        let provider = self
            .provider(provider_name)
            .ok_or_else(|| AppError::Validation {
                message: format!("provider `{provider_name}` is not registered"),
                source: None,
            })?;

        provider.fetch_latest_tick(provider_symbol).await
    }

    pub async fn fetch_klines_window_from(
        &self,
        provider_name: &str,
        query: HistoricalKlineQuery,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let provider = self
            .provider(provider_name)
            .ok_or_else(|| AppError::Validation {
                message: format!("provider `{provider_name}` is not registered"),
                source: None,
            })?;

        provider.fetch_klines_window(query).await
    }
}
