use std::sync::{
    Arc, Mutex,
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

#[tokio::test]
async fn fetch_klines_falls_back_when_primary_returns_empty() {
    let primary = ok_klines_provider("primary", Vec::new());
    let fallback = ok_klines_provider("fallback", vec![ProviderKline::fixture()]);

    let mut router = ProviderRouter::default();
    router.insert(primary);
    router.insert(fallback);
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
        .expect("fallback should satisfy");

    assert_eq!(routed.provider_name, "fallback");
    assert_eq!(routed.klines.len(), 1);
}

#[tokio::test]
async fn fetch_klines_returns_validation_when_binding_missing() {
    let primary = ok_klines_provider("primary", vec![ProviderKline::fixture()]);
    let mut router = ProviderRouter::default();
    router.insert(primary);
    let gateway = MarketGateway::new(router);

    // Policy points at "ghost" but no binding for that provider exists.
    let mut ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );
    ctx.policy.kline_primary = "ghost".to_string();

    let err = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect_err("missing binding should error");

    match err {
        AppError::Validation { message, .. } => {
            assert!(message.contains("missing provider binding"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_klines_returns_validation_when_provider_not_registered() {
    let router = ProviderRouter::default(); // empty router
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );

    let err = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect_err("unregistered provider should error");

    match err {
        AppError::Validation { message, .. } => {
            assert!(message.contains("provider `primary` is not registered"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_klines_no_fallback_surfaces_primary_failure() {
    let primary = err_klines_provider("primary");
    let mut router = ProviderRouter::default();
    router.insert(primary);
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );

    let err = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect_err("primary failure with no fallback should surface");

    match err {
        AppError::Provider { message, .. } => {
            assert!(message.contains("primary kline failed"));
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}

fn sample_tick(price: &str) -> ProviderTick {
    ProviderTick {
        price: price.parse().expect("decimal parses"),
        size: None,
        tick_time: chrono::DateTime::parse_from_rfc3339("2024-01-02T09:30:00Z")
            .expect("rfc3339 parses")
            .with_timezone(&chrono::Utc),
    }
}

#[tokio::test]
async fn fetch_latest_tick_returns_primary_when_primary_succeeds() {
    let primary = Arc::new(StubProvider {
        name: "primary",
        klines: Err(AppError::Provider {
            message: "klines not exercised".into(),
            source: None,
        }),
        tick: Ok(sample_tick("10.5")),
        kline_calls: Arc::new(AtomicUsize::new(0)),
        tick_calls: Arc::new(AtomicUsize::new(0)),
    });

    let mut router = ProviderRouter::default();
    router.insert(primary);
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );

    let routed = gateway
        .fetch_latest_tick(&ctx)
        .await
        .expect("primary tick should satisfy");

    assert_eq!(routed.provider_name, "primary");
    assert_eq!(routed.tick.price, "10.5".parse().unwrap());
}

struct SymbolRecorder {
    name: &'static str,
    klines: Vec<ProviderKline>,
    tick: ProviderTick,
    last_kline_symbol: Arc<Mutex<Option<String>>>,
    last_tick_symbol: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl MarketDataProvider for SymbolRecorder {
    fn name(&self) -> &'static str {
        self.name
    }
    async fn fetch_klines(
        &self,
        provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        *self.last_kline_symbol.lock().expect("lock") = Some(provider_symbol.to_string());
        Ok(self.klines.clone())
    }
    async fn fetch_latest_tick(&self, provider_symbol: &str) -> Result<ProviderTick, AppError> {
        *self.last_tick_symbol.lock().expect("lock") = Some(provider_symbol.to_string());
        Ok(self.tick.clone())
    }
    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

#[tokio::test]
async fn gateway_forwards_binding_derived_provider_symbol() {
    let kline_record = Arc::new(Mutex::new(None));
    let tick_record = Arc::new(Mutex::new(None));

    let recorder = Arc::new(SymbolRecorder {
        name: "primary",
        klines: vec![ProviderKline::fixture()],
        tick: sample_tick("12.34"),
        last_kline_symbol: Arc::clone(&kline_record),
        last_tick_symbol: Arc::clone(&tick_record),
    });

    let mut router = ProviderRouter::default();
    router.insert(recorder);
    let gateway = MarketGateway::new(router);

    let mut ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );
    // Pin the binding to a known value so we can assert it was forwarded.
    ctx.bindings
        .iter_mut()
        .find(|b| b.provider == "primary")
        .expect("primary binding present")
        .provider_symbol = "EXPECTED-SYMBOL".to_string();

    gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect("kline fetch should succeed");
    gateway
        .fetch_latest_tick(&ctx)
        .await
        .expect("tick fetch should succeed");

    assert_eq!(
        kline_record.lock().expect("lock").as_deref(),
        Some("EXPECTED-SYMBOL"),
    );
    assert_eq!(
        tick_record.lock().expect("lock").as_deref(),
        Some("EXPECTED-SYMBOL"),
    );
}
