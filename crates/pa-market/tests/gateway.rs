use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use pa_core::{AppError, Timeframe};
use pa_instrument::InstrumentMarketDataContext;
use pa_market::{MarketDataProvider, MarketGateway, ProviderKline, ProviderRouter, ProviderTick};

struct StubProvider {
    name: &'static str,
    klines: Result<Vec<ProviderKline>, AppError>,
    tick: Result<ProviderTick, AppError>,
    kline_calls: Arc<AtomicUsize>,
    tick_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl MarketDataProvider for StubProvider {
    fn name(&self) -> &'static str {
        self.name
    }
    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        self.kline_calls.fetch_add(1, Ordering::SeqCst);
        match &self.klines {
            Ok(k) => Ok(k.clone()),
            Err(_) => Err(AppError::Provider {
                message: format!("{} kline failed", self.name),
                source: None,
            }),
        }
    }
    async fn fetch_latest_tick(&self, _provider_symbol: &str) -> Result<ProviderTick, AppError> {
        self.tick_calls.fetch_add(1, Ordering::SeqCst);
        match &self.tick {
            Ok(t) => Ok(t.clone()),
            Err(_) => Err(AppError::Provider {
                message: format!("{} tick failed", self.name),
                source: None,
            }),
        }
    }
    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

fn ok_klines_provider(name: &'static str, klines: Vec<ProviderKline>) -> Arc<StubProvider> {
    Arc::new(StubProvider {
        name,
        klines: Ok(klines),
        tick: Err(AppError::Provider {
            message: "tick not exercised".into(),
            source: None,
        }),
        kline_calls: Arc::new(AtomicUsize::new(0)),
        tick_calls: Arc::new(AtomicUsize::new(0)),
    })
}

fn err_klines_provider(name: &'static str) -> Arc<StubProvider> {
    Arc::new(StubProvider {
        name,
        klines: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        tick: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        kline_calls: Arc::new(AtomicUsize::new(0)),
        tick_calls: Arc::new(AtomicUsize::new(0)),
    })
}

#[tokio::test]
async fn fetch_klines_returns_primary_provider_when_primary_succeeds() {
    let primary = ok_klines_provider("primary", vec![ProviderKline::fixture()]);
    let fallback = ok_klines_provider("fallback", vec![ProviderKline::fixture()]);

    let mut router = ProviderRouter::default();
    router.insert(primary.clone());
    router.insert(fallback.clone());
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        Some("fallback"),
        "primary",
        Some("fallback"),
    );

    let routed = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect("primary should satisfy");

    assert_eq!(routed.provider_name, "primary");
    assert_eq!(routed.klines.len(), 1);
}
