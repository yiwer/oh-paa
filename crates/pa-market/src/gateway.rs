use chrono::{DateTime, Utc};
use pa_core::{AppError, DebugEvent, Timeframe};
use pa_instrument::InstrumentMarketDataContext;
use tokio::sync::broadcast;

use crate::provider::{
    HistoricalKlineQuery, ProviderRouter, RoutedKlines, RoutedTick,
};

pub struct MarketGateway {
    router: ProviderRouter,
    debug_tx: Option<broadcast::Sender<DebugEvent>>,
}

impl MarketGateway {
    pub fn new(router: ProviderRouter) -> Self {
        Self {
            router,
            debug_tx: None,
        }
    }

    pub fn with_debug_tx(mut self, tx: broadcast::Sender<DebugEvent>) -> Self {
        self.debug_tx = Some(tx);
        self
    }

    pub fn router(&self) -> &ProviderRouter {
        &self.router
    }

    fn emit(&self, event: DebugEvent) {
        if let Some(tx) = &self.debug_tx {
            let _ = tx.send(event);
        }
    }

    pub async fn fetch_klines(
        &self,
        ctx: &InstrumentMarketDataContext,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<RoutedKlines, AppError> {
        let primary = ctx.policy.kline_primary.as_str();
        let primary_symbol = ctx.binding_for_provider(primary)?.provider_symbol.clone();

        let start = tokio::time::Instant::now();

        let routed = match ctx.policy.kline_fallback.as_deref() {
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
                    .await?
            }
            None => {
                let klines = self
                    .router
                    .fetch_klines_from(primary, &primary_symbol, timeframe, limit)
                    .await?;
                RoutedKlines {
                    provider_name: primary.to_string(),
                    klines,
                }
            }
        };

        let latency_ms = start.elapsed().as_millis() as u64;
        let last_open_time = routed.klines.last().map(|k| k.open_time).unwrap_or_default();

        self.emit(DebugEvent::KlineIngested {
            instrument_id: ctx.instrument.id,
            timeframe: timeframe.to_string(),
            open_time: last_open_time,
            provider: routed.provider_name.clone(),
            latency_ms,
        });

        if routed.provider_name != primary {
            self.emit(DebugEvent::ProviderFallback {
                instrument_id: ctx.instrument.id,
                primary_provider: primary.to_string(),
                fallback_provider: routed.provider_name.clone(),
                error: "primary returned empty or failed".to_string(),
            });
        }

        Ok(routed)
    }

    /// Window queries are primary-only. `policy.kline_fallback` is intentionally ignored.
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

        let start = tokio::time::Instant::now();

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

        let routed = RoutedKlines {
            provider_name: primary.to_string(),
            klines,
        };

        let latency_ms = start.elapsed().as_millis() as u64;
        let last_open_time = routed.klines.last().map(|k| k.open_time).unwrap_or_default();

        self.emit(DebugEvent::KlineIngested {
            instrument_id: ctx.instrument.id,
            timeframe: timeframe.to_string(),
            open_time: last_open_time,
            provider: routed.provider_name.clone(),
            latency_ms,
        });

        Ok(routed)
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
