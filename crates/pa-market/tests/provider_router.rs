use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use pa_core::{AppError, Timeframe};
use pa_market::{MarketDataProvider, ProviderKline, ProviderTick, provider::ProviderRouter};

struct StubProvider {
    name: &'static str,
    klines: Result<Vec<ProviderKline>, AppError>,
    tick: Result<ProviderTick, AppError>,
    calls: Arc<AtomicUsize>,
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
        self.calls.fetch_add(1, Ordering::SeqCst);

        match &self.klines {
            Ok(klines) => Ok(klines.clone()),
            Err(_) => Err(AppError::Provider {
                message: format!("{} failed", self.name),
                source: None,
            }),
        }
    }

    async fn fetch_latest_tick(&self, _provider_symbol: &str) -> Result<ProviderTick, AppError> {
        self.tick_calls.fetch_add(1, Ordering::SeqCst);

        match &self.tick {
            Ok(tick) => Ok(tick.clone()),
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

#[tokio::test]
async fn fetch_klines_uses_fallback_when_primary_fails() {
    let primary_calls = Arc::new(AtomicUsize::new(0));
    let fallback_calls = Arc::new(AtomicUsize::new(0));
    let primary_tick_calls = Arc::new(AtomicUsize::new(0));
    let fallback_tick_calls = Arc::new(AtomicUsize::new(0));
    let expected = vec![ProviderKline::fixture()];

    let mut router = ProviderRouter::default();
    router.insert(Arc::new(StubProvider {
        name: "primary",
        klines: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        calls: Arc::clone(&primary_calls),
        tick: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        tick_calls: Arc::clone(&primary_tick_calls),
    }));
    router.insert(Arc::new(StubProvider {
        name: "fallback",
        klines: Ok(expected.clone()),
        calls: Arc::clone(&fallback_calls),
        tick: Ok(tick("2024-01-02T09:30:00Z", "10.1")),
        tick_calls: Arc::clone(&fallback_tick_calls),
    }));

    let actual = router
        .fetch_klines_with_fallback(
            "primary",
            "fallback",
            "000001.SZ",
            "0.000001",
            Timeframe::M15,
            100,
        )
        .await
        .expect("fallback should satisfy request");

    assert_eq!(actual, expected);
    assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn fetch_klines_returns_validation_error_when_primary_is_missing() {
    let fallback_calls = Arc::new(AtomicUsize::new(0));
    let fallback_tick_calls = Arc::new(AtomicUsize::new(0));
    let expected = vec![ProviderKline::fixture()];

    let mut router = ProviderRouter::default();
    router.insert(Arc::new(StubProvider {
        name: "fallback",
        klines: Ok(expected),
        calls: Arc::clone(&fallback_calls),
        tick: Ok(tick("2024-01-02T09:30:00Z", "10.1")),
        tick_calls: Arc::clone(&fallback_tick_calls),
    }));

    let error = router
        .fetch_klines_with_fallback(
            "primary",
            "fallback",
            "000001.SZ",
            "0.000001",
            Timeframe::M15,
            100,
        )
        .await
        .expect_err("missing primary should return a validation error");

    match error {
        AppError::Validation { message, .. } => {
            assert!(message.contains("provider `primary` is not registered"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }

    assert_eq!(fallback_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn fetch_klines_surfaces_validation_error_when_fallback_is_missing() {
    let primary_calls = Arc::new(AtomicUsize::new(0));
    let primary_tick_calls = Arc::new(AtomicUsize::new(0));

    let mut router = ProviderRouter::default();
    router.insert(Arc::new(StubProvider {
        name: "primary",
        klines: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        calls: Arc::clone(&primary_calls),
        tick: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        tick_calls: Arc::clone(&primary_tick_calls),
    }));

    let error = router
        .fetch_klines_with_fallback(
            "primary",
            "fallback",
            "000001.SZ",
            "0.000001",
            Timeframe::M15,
            100,
        )
        .await
        .expect_err("missing fallback should surface validation error");

    match error {
        AppError::Validation { message, .. } => {
            assert!(message.contains("provider `fallback` is not registered"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }

    assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn fetch_latest_tick_uses_fallback_when_primary_fails() {
    let primary_calls = Arc::new(AtomicUsize::new(0));
    let fallback_calls = Arc::new(AtomicUsize::new(0));
    let primary_tick_calls = Arc::new(AtomicUsize::new(0));
    let fallback_tick_calls = Arc::new(AtomicUsize::new(0));

    let mut router = ProviderRouter::default();
    router.insert(Arc::new(StubProvider {
        name: "primary",
        klines: Ok(vec![ProviderKline::fixture()]),
        tick: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        calls: Arc::clone(&primary_calls),
        tick_calls: Arc::clone(&primary_tick_calls),
    }));
    router.insert(Arc::new(StubProvider {
        name: "fallback",
        klines: Ok(vec![ProviderKline::fixture()]),
        tick: Ok(tick("2024-01-02T09:30:00Z", "10.2")),
        calls: Arc::clone(&fallback_calls),
        tick_calls: Arc::clone(&fallback_tick_calls),
    }));

    let actual = router
        .fetch_latest_tick_with_fallback(
            "primary",
            "fallback",
            "AAA",
            "BBB",
        )
        .await
        .expect("fallback tick provider should satisfy request");

    assert_eq!(actual.price, "10.2".parse().expect("decimal should parse"));
    assert_eq!(primary_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fallback_calls.load(Ordering::SeqCst), 0);
    assert_eq!(primary_tick_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fallback_tick_calls.load(Ordering::SeqCst), 1);
}

fn tick(tick_time: &str, price: &str) -> ProviderTick {
    ProviderTick {
        price: price.parse().expect("price should parse"),
        size: None,
        tick_time: chrono::DateTime::parse_from_rfc3339(tick_time)
            .expect("tick timestamp should parse")
            .with_timezone(&chrono::Utc),
    }
}
