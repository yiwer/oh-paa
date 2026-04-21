use pa_core::AppError;
use pa_market::models::ProviderKline;
use pa_market::normalize::normalize_kline;
use pa_market::provider::MarketDataProvider;
use rust_decimal::Decimal;

#[test]
fn rejects_kline_when_high_is_below_close() {
    let mut bar = ProviderKline::fixture();
    bar.high = Decimal::new(104, 0);
    bar.close = Decimal::new(105, 0);

    let error = normalize_kline(bar).expect_err("invalid OHLC should be rejected");

    match error {
        AppError::Validation { message, .. } => {
            assert!(message.contains("high"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn accepts_valid_kline_unchanged() {
    let bar = ProviderKline::fixture();
    let expected = bar.clone();

    let normalized = normalize_kline(bar).expect("valid kline should be accepted");

    assert_eq!(normalized, expected);
}

#[test]
fn rejects_kline_when_low_is_above_open_or_close() {
    let mut bar = ProviderKline::fixture();
    bar.low = Decimal::new(106, 0);

    let error = normalize_kline(bar).expect_err("invalid low should be rejected");

    match error {
        AppError::Validation { message, .. } => {
            assert!(message.contains("low"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_kline_when_close_time_is_not_after_open_time() {
    let mut bar = ProviderKline::fixture();
    bar.close_time = bar.open_time;

    let error = normalize_kline(bar).expect_err("non-increasing time range should be rejected");

    match error {
        AppError::Validation { message, .. } => {
            assert!(message.contains("close_time"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_kline_when_volume_is_negative() {
    let mut bar = ProviderKline::fixture();
    bar.volume = Some(Decimal::new(-1, 0));

    let error = normalize_kline(bar).expect_err("negative volume should be rejected");

    match error {
        AppError::Validation { message, .. } => {
            assert!(message.contains("volume"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn market_data_provider_trait_is_send_and_sync() {
    fn assert_send_sync<T: MarketDataProvider + Send + Sync>() {}

    assert_send_sync::<DummyProvider>();
}

struct DummyProvider;

#[async_trait::async_trait]
impl MarketDataProvider for DummyProvider {
    fn name(&self) -> &'static str {
        "dummy"
    }

    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: pa_core::Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        unimplemented!()
    }

    async fn fetch_latest_tick(
        &self,
        _provider_symbol: &str,
    ) -> Result<pa_market::models::ProviderTick, AppError> {
        unimplemented!()
    }

    async fn healthcheck(&self) -> Result<(), AppError> {
        unimplemented!()
    }
}
