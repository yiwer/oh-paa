use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use pa_core::{AppError, Timeframe};
use pa_market::{
    InMemoryCanonicalKlineRepository, MarketDataProvider, ProviderKline, ProviderRouter,
    ProviderTick, backfill_canonical_klines,
};
use uuid::Uuid;

struct StubProvider {
    name: &'static str,
    klines: Result<Vec<ProviderKline>, AppError>,
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
        match &self.klines {
            Ok(klines) => Ok(klines.clone()),
            Err(_) => Err(AppError::Provider {
                message: format!("{} failed", self.name),
                source: None,
            }),
        }
    }

    async fn fetch_latest_tick(&self, _provider_symbol: &str) -> Result<ProviderTick, AppError> {
        unimplemented!("tick fetching is outside this backfill test")
    }

    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

#[tokio::test]
async fn repeated_backfill_upserts_canonical_rows_by_instrument_timeframe_and_open_time() {
    let instrument_id = Uuid::new_v4();
    let klines = vec![ProviderKline::fixture(), ProviderKline::fixture()];
    let repository = InMemoryCanonicalKlineRepository::default();

    let mut router = ProviderRouter::default();
    router.insert(Arc::new(StubProvider {
        name: "primary",
        klines: Ok(klines),
    }));
    router.insert(Arc::new(StubProvider {
        name: "fallback",
        klines: Ok(Vec::new()),
    }));

    backfill_canonical_klines(
        &router,
        &repository,
        instrument_id,
        "000001.SZ",
        Timeframe::M15,
        100,
        "primary",
        "fallback",
    )
    .await
    .expect("first backfill should succeed");
    backfill_canonical_klines(
        &router,
        &repository,
        instrument_id,
        "000001.SZ",
        Timeframe::M15,
        100,
        "primary",
        "fallback",
    )
    .await
    .expect("repeat backfill should still succeed");

    let rows = repository.rows();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].instrument_id, instrument_id);
    assert_eq!(rows[0].timeframe, Timeframe::M15);
    assert_eq!(rows[0].open_time, ProviderKline::fixture().open_time);
}

#[tokio::test]
async fn backfill_skips_bars_whose_close_time_is_still_in_the_future() {
    let instrument_id = Uuid::new_v4();
    let mut future_bar = ProviderKline::fixture();
    future_bar.close_time = Utc::now() + Duration::minutes(15);

    let repository = InMemoryCanonicalKlineRepository::default();
    let mut router = ProviderRouter::default();
    router.insert(Arc::new(StubProvider {
        name: "primary",
        klines: Ok(vec![future_bar]),
    }));
    router.insert(Arc::new(StubProvider {
        name: "fallback",
        klines: Ok(Vec::new()),
    }));

    backfill_canonical_klines(
        &router,
        &repository,
        instrument_id,
        "000001.SZ",
        Timeframe::M15,
        100,
        "primary",
        "fallback",
    )
    .await
    .expect("future bars should be skipped without failing backfill");

    assert!(repository.rows().is_empty());
}

#[tokio::test]
async fn backfill_persists_fallback_provider_name_when_primary_returns_empty() {
    let instrument_id = Uuid::new_v4();
    let repository = InMemoryCanonicalKlineRepository::default();
    let mut router = ProviderRouter::default();

    router.insert(Arc::new(StubProvider {
        name: "primary",
        klines: Ok(Vec::new()),
    }));
    router.insert(Arc::new(StubProvider {
        name: "fallback",
        klines: Ok(vec![ProviderKline::fixture()]),
    }));

    backfill_canonical_klines(
        &router,
        &repository,
        instrument_id,
        "000001.SZ",
        Timeframe::M15,
        100,
        "primary",
        "fallback",
    )
    .await
    .expect("fallback backfill should succeed");

    let rows = repository.rows();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source_provider, "fallback");
}
