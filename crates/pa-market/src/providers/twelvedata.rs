use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{MarketDataProvider, ProviderKline, ProviderTick};

#[derive(Debug, Default)]
pub struct TwelveDataProvider;

#[allow(dead_code)]
impl TwelveDataProvider {
    fn parse_klines_response(
        body: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let response: TwelveDataKlinesResponse =
            serde_json::from_str(body).map_err(|source| AppError::Provider {
                message: "failed to parse twelvedata kline response".into(),
                source: Some(Box::new(source)),
            })?;
        let bar_duration = chrono::Duration::from_std(timeframe.duration()).map_err(|source| {
            AppError::Provider {
                message: "invalid timeframe for twelvedata kline translation".into(),
                source: Some(Box::new(source)),
            }
        })?;

        response
            .values
            .into_iter()
            .map(|row| {
                let open_time =
                    parse_rfc3339_timestamp(&row.datetime, "twelvedata kline datetime")?;

                Ok(ProviderKline {
                    open_time,
                    close_time: open_time + bar_duration,
                    open: parse_decimal(&row.open, "twelvedata kline open")?,
                    high: parse_decimal(&row.high, "twelvedata kline high")?,
                    low: parse_decimal(&row.low, "twelvedata kline low")?,
                    close: parse_decimal(&row.close, "twelvedata kline close")?,
                    volume: parse_optional_decimal(
                        row.volume.as_deref(),
                        "twelvedata kline volume",
                    )?,
                })
            })
            .collect()
    }

    fn parse_latest_tick_response(body: &str) -> Result<ProviderTick, AppError> {
        let response: TwelveDataTickResponse =
            serde_json::from_str(body).map_err(|source| AppError::Provider {
                message: "failed to parse twelvedata tick response".into(),
                source: Some(Box::new(source)),
            })?;

        Ok(ProviderTick {
            price: parse_decimal(&response.price, "twelvedata tick price")?,
            size: parse_optional_decimal(response.volume.as_deref(), "twelvedata tick volume")?,
            tick_time: parse_rfc3339_timestamp(&response.timestamp, "twelvedata tick timestamp")?,
        })
    }

    fn transport_not_wired_error() -> AppError {
        AppError::Provider {
            message: "twelvedata transport is not wired yet".into(),
            source: None,
        }
    }
}

#[async_trait]
impl MarketDataProvider for TwelveDataProvider {
    fn name(&self) -> &'static str {
        "twelvedata"
    }

    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        Err(Self::transport_not_wired_error())
    }

    async fn fetch_latest_tick(&self, _provider_symbol: &str) -> Result<ProviderTick, AppError> {
        Err(Self::transport_not_wired_error())
    }

    async fn healthcheck(&self) -> Result<(), AppError> {
        Err(Self::transport_not_wired_error())
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TwelveDataKlinesResponse {
    values: Vec<TwelveDataKlineRow>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TwelveDataKlineRow {
    datetime: String,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TwelveDataTickResponse {
    price: String,
    volume: Option<String>,
    timestamp: String,
}

#[allow(dead_code)]
fn parse_decimal(value: &str, field: &str) -> Result<Decimal, AppError> {
    Decimal::from_str(value).map_err(|source| AppError::Provider {
        message: format!("invalid {field}"),
        source: Some(Box::new(source)),
    })
}

#[allow(dead_code)]
fn parse_optional_decimal(value: Option<&str>, field: &str) -> Result<Option<Decimal>, AppError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            Decimal::from_str(value).map_err(|source| AppError::Provider {
                message: format!("invalid {field}"),
                source: Some(Box::new(source)),
            })
        })
        .transpose()
}

#[allow(dead_code)]
fn parse_rfc3339_timestamp(value: &str, field: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|source| AppError::Provider {
            message: format!("invalid {field}"),
            source: Some(Box::new(source)),
        })
}

#[cfg(test)]
mod tests {
    use crate::MarketDataProvider;
    use chrono::{DateTime, Utc};
    use pa_core::Timeframe;
    use rust_decimal::Decimal;

    use super::TwelveDataProvider;

    #[test]
    fn parses_kline_payload_into_provider_klines() {
        let klines = TwelveDataProvider::parse_klines_response(
            r#"{
                "values": [
                    {
                        "datetime": "2024-01-02T09:30:00Z",
                        "open": "10.1",
                        "high": "11.0",
                        "low": "10.0",
                        "close": "10.8",
                        "volume": "12345"
                    }
                ]
            }"#,
            Timeframe::M15,
        )
        .expect("twelvedata kline payload should parse");

        assert_eq!(klines.len(), 1);
        assert_eq!(klines[0].open, Decimal::new(101, 1));
        assert_eq!(klines[0].high, Decimal::new(110, 1));
        assert_eq!(klines[0].low, Decimal::new(100, 1));
        assert_eq!(klines[0].close, Decimal::new(108, 1));
        assert_eq!(klines[0].volume, Some(Decimal::new(12_345, 0)));
        assert_eq!(klines[0].open_time, utc("2024-01-02T09:30:00Z"));
        assert_eq!(klines[0].close_time, utc("2024-01-02T09:45:00Z"));
    }

    #[test]
    fn parses_tick_payload_into_provider_tick() {
        let tick = TwelveDataProvider::parse_latest_tick_response(
            r#"{
                "price": "10.8",
                "volume": "2300",
                "timestamp": "2024-01-02T09:30:05Z"
            }"#,
        )
        .expect("twelvedata tick payload should parse");

        assert_eq!(tick.price, Decimal::new(108, 1));
        assert_eq!(tick.size, Some(Decimal::new(2_300, 0)));
        assert_eq!(tick.tick_time, utc("2024-01-02T09:30:05Z"));
    }

    #[test]
    fn parses_blank_optional_numeric_fields_as_none() {
        let klines = TwelveDataProvider::parse_klines_response(
            r#"{
                "values": [
                    {
                        "datetime": "2024-01-02T09:30:00Z",
                        "open": "10.1",
                        "high": "11.0",
                        "low": "10.0",
                        "close": "10.8",
                        "volume": "  "
                    }
                ]
            }"#,
            Timeframe::M15,
        )
        .expect("blank twelvedata kline volume should parse as none");
        let tick = TwelveDataProvider::parse_latest_tick_response(
            r#"{
                "price": "10.8",
                "volume": "   ",
                "timestamp": "2024-01-02T09:30:05Z"
            }"#,
        )
        .expect("blank twelvedata tick size should parse as none");

        assert_eq!(klines[0].volume, None);
        assert_eq!(tick.size, None);
    }

    #[tokio::test]
    async fn unwired_provider_healthcheck_returns_provider_error() {
        let error = TwelveDataProvider
            .healthcheck()
            .await
            .expect_err("unwired provider should be unhealthy");

        match error {
            pa_core::AppError::Provider { message, .. } => {
                assert_eq!(message, "twelvedata transport is not wired yet");
            }
            other => panic!("expected provider error, got {other:?}"),
        }
    }

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("fixture timestamp should be valid")
            .with_timezone(&Utc)
    }
}
