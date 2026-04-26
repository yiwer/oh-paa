use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use pa_instrument::InstrumentMarketDataContext;

use crate::provider::{
    HistoricalKlineQuery, ProviderRouter, RoutedKlines, RoutedTick,
};

pub struct MarketGateway {
    router: ProviderRouter,
}

impl MarketGateway {
    pub fn new(router: ProviderRouter) -> Self {
        Self { router }
    }

    pub fn router(&self) -> &ProviderRouter {
        &self.router
    }

    pub async fn fetch_klines(
        &self,
        ctx: &InstrumentMarketDataContext,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<RoutedKlines, AppError> {
        let primary = ctx.policy.kline_primary.as_str();
        let primary_symbol = ctx.binding_for_provider(primary)?.provider_symbol.clone();

        match ctx.policy.kline_fallback.as_deref() {
            Some(fallback) => {
                let fallback_symbol =
                    ctx.binding_for_provider(fallback)?.provider_symbol.clone();
                self.router
                    .fetch_klines_with_fallback_source(
                        primary,
                        fallback,
                        &primary_symbol,
                        &fallback_symbol,
                        timeframe,
                        limit,
                    )
                    .await
            }
            None => {
                let klines = self
                    .router
                    .fetch_klines_from(primary, &primary_symbol, timeframe, limit)
                    .await?;
                Ok(RoutedKlines {
                    provider_name: primary.to_string(),
                    klines,
                })
            }
        }
    }

    pub async fn fetch_klines_window(
        &self,
        ctx: &InstrumentMarketDataContext,
        timeframe: Timeframe,
        start_open_time: Option<DateTime<Utc>>,
        end_close_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<RoutedKlines, AppError> {
        let primary = ctx.policy.kline_primary.as_str();
        let primary_symbol = ctx.binding_for_provider(primary)?.provider_symbol.clone();

        let klines = self
            .router
            .fetch_klines_window_from(
                primary,
                HistoricalKlineQuery {
                    provider_symbol: primary_symbol,
                    timeframe,
                    start_open_time,
                    end_close_time,
                    limit,
                },
            )
            .await?;
        Ok(RoutedKlines {
            provider_name: primary.to_string(),
            klines,
        })
    }

    pub async fn fetch_latest_tick(
        &self,
        ctx: &InstrumentMarketDataContext,
    ) -> Result<RoutedTick, AppError> {
        let primary = ctx.policy.tick_primary.as_str();
        let primary_symbol = ctx.binding_for_provider(primary)?.provider_symbol.clone();

        match ctx.policy.tick_fallback.as_deref() {
            Some(fallback) => {
                let fallback_symbol =
                    ctx.binding_for_provider(fallback)?.provider_symbol.clone();
                self.router
                    .fetch_latest_tick_with_fallback_source(
                        primary,
                        fallback,
                        &primary_symbol,
                        &fallback_symbol,
                    )
                    .await
            }
            None => {
                let tick = self
                    .router
                    .fetch_latest_tick_from(primary, &primary_symbol)
                    .await?;
                Ok(RoutedTick {
                    provider_name: primary.to_string(),
                    tick,
                })
            }
        }
    }
}
