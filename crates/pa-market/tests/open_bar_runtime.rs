use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use pa_market::{
    CanonicalKlineRepository, CanonicalKlineRow, DeriveOpenBarRequest,
    InMemoryCanonicalKlineRepository, MarketDataProvider, ProviderKline, ProviderRouter,
    ProviderTick, derive_open_bar,
};
use rust_decimal::Decimal;
use uuid::Uuid;

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp should be valid")
        .with_timezone(&Utc)
}

fn decimal(value: &str) -> Decimal {
    value.parse().expect("fixture decimal should parse")
}

fn row(
    instrument_id: Uuid,
    open_time: &str,
    open: &str,
    high: &str,
    low: &str,
    close: &str,
) -> CanonicalKlineRow {
    let open_time = utc(open_time);

    CanonicalKlineRow {
        instrument_id,
        timeframe: Timeframe::M15,
        open_time,
        close_time: open_time + chrono::Duration::minutes(15),
        open: decimal(open),
        high: decimal(high),
        low: decimal(low),
        close: decimal(close),
        volume: Some(decimal("100")),
        source_provider: "eastmoney".to_string(),
    }
}

async fn insert_rows(repository: &InMemoryCanonicalKlineRepository, rows: Vec<CanonicalKlineRow>) {
    for row in rows {
        repository
            .upsert_canonical_kline(row)
            .await
            .expect("fixture row should insert");
    }
}

struct TickOnlyProvider {
    tick: ProviderTick,
}

#[async_trait]
impl MarketDataProvider for TickOnlyProvider {
    fn name(&self) -> &'static str {
        "fallback"
    }

    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        Ok(Vec::new())
    }

    async fn fetch_latest_tick(&self, provider_symbol: &str) -> Result<ProviderTick, AppError> {
        if provider_symbol != "BBB" {
            return Err(AppError::Validation {
                message: format!("unexpected symbol: {provider_symbol}"),
                source: None,
            });
        }

        Ok(self.tick.clone())
    }

    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

struct FailingTickProvider;

#[async_trait]
impl MarketDataProvider for FailingTickProvider {
    fn name(&self) -> &'static str {
        "primary"
    }

    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        Ok(Vec::new())
    }

    async fn fetch_latest_tick(&self, _provider_symbol: &str) -> Result<ProviderTick, AppError> {
        Err(AppError::Provider {
            message: "primary tick failed".into(),
            source: None,
        })
    }

    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

#[tokio::test]
async fn cn_a_hourly_open_bar_after_lunch_uses_previous_close_as_open() {
    let instrument_id = Uuid::new_v4();
    let repository = InMemoryCanonicalKlineRepository::default();
    insert_rows(
        &repository,
        vec![
            row(instrument_id, "2024-01-02T02:30:00Z", "10.4", "10.6", "10.3", "10.5"),
            row(instrument_id, "2024-01-02T02:45:00Z", "10.5", "10.7", "10.4", "10.6"),
            row(instrument_id, "2024-01-02T03:00:00Z", "10.6", "10.8", "10.5", "10.7"),
            row(instrument_id, "2024-01-02T03:15:00Z", "10.7", "10.9", "10.6", "10.8"),
        ],
    )
    .await;

    let mut router = ProviderRouter::default();
    router.insert(Arc::new(FailingTickProvider));
    router.insert(Arc::new(TickOnlyProvider {
        tick: ProviderTick {
            price: decimal("10.95"),
            size: None,
            tick_time: utc("2024-01-02T05:05:00Z"),
        },
    }));

    let open_bar = derive_open_bar(
        &router,
        &repository,
        DeriveOpenBarRequest {
            instrument_id,
            timeframe: Timeframe::H1,
            market_code: Some("cn-a".to_string()),
            market_timezone: Some("Asia/Shanghai".to_string()),
            primary_provider: "primary",
            fallback_provider: "fallback",
            primary_provider_symbol: "AAA",
            fallback_provider_symbol: "BBB",
        },
    )
    .await
    .expect("open bar derivation should succeed")
    .expect("market should be open");

    assert_eq!(open_bar.open_time, utc("2024-01-02T05:00:00Z"));
    assert_eq!(open_bar.close_time, utc("2024-01-02T06:00:00Z"));
    assert_eq!(open_bar.open, decimal("10.8"));
    assert_eq!(open_bar.high, decimal("10.95"));
    assert_eq!(open_bar.low, decimal("10.8"));
    assert_eq!(open_bar.close, decimal("10.95"));
    assert_eq!(open_bar.child_bar_count, 0);
}

#[tokio::test]
async fn cn_a_daily_open_bar_reuses_closed_children_before_applying_latest_tick() {
    let instrument_id = Uuid::new_v4();
    let repository = InMemoryCanonicalKlineRepository::default();
    insert_rows(
        &repository,
        vec![
            row(instrument_id, "2024-01-02T01:30:00Z", "10.0", "10.2", "9.9", "10.1"),
            row(instrument_id, "2024-01-02T01:45:00Z", "10.1", "10.3", "10.0", "10.2"),
            row(instrument_id, "2024-01-02T02:00:00Z", "10.2", "10.5", "10.1", "10.4"),
            row(instrument_id, "2024-01-02T02:15:00Z", "10.4", "10.6", "10.3", "10.5"),
            row(instrument_id, "2024-01-02T05:00:00Z", "10.5", "10.8", "10.4", "10.7"),
        ],
    )
    .await;

    let mut router = ProviderRouter::default();
    router.insert(Arc::new(FailingTickProvider));
    router.insert(Arc::new(TickOnlyProvider {
        tick: ProviderTick {
            price: decimal("10.9"),
            size: None,
            tick_time: utc("2024-01-02T05:20:00Z"),
        },
    }));

    let open_bar = derive_open_bar(
        &router,
        &repository,
        DeriveOpenBarRequest {
            instrument_id,
            timeframe: Timeframe::D1,
            market_code: Some("cn-a".to_string()),
            market_timezone: Some("Asia/Shanghai".to_string()),
            primary_provider: "primary",
            fallback_provider: "fallback",
            primary_provider_symbol: "AAA",
            fallback_provider_symbol: "BBB",
        },
    )
    .await
    .expect("open bar derivation should succeed")
    .expect("market should be open");

    assert_eq!(open_bar.open_time, utc("2024-01-02T01:30:00Z"));
    assert_eq!(open_bar.close_time, utc("2024-01-02T07:00:00Z"));
    assert_eq!(open_bar.open, decimal("10.0"));
    assert_eq!(open_bar.high, decimal("10.9"));
    assert_eq!(open_bar.low, decimal("9.9"));
    assert_eq!(open_bar.close, decimal("10.9"));
    assert_eq!(open_bar.child_bar_count, 5);
}
