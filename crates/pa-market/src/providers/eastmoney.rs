use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use pa_core::{AppError, Timeframe};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{MarketDataProvider, ProviderKline, ProviderTick};

#[derive(Debug, Default)]
pub struct EastMoneyProvider;

#[allow(dead_code)]
impl EastMoneyProvider {
    fn parse_klines_response(
        body: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let response: EastMoneyKlinesResponse =
            serde_json::from_str(body).map_err(|source| AppError::Provider {
                message: "failed to parse eastmoney kline response".into(),
                source: Some(Box::new(source)),
            })?;
        let bar_duration = chrono::Duration::from_std(timeframe.duration()).map_err(|source| {
            AppError::Provider {
                message: "invalid timeframe for eastmoney kline translation".into(),
                source: Some(Box::new(source)),
            }
        })?;

        response
            .data
            .klines
            .into_iter()
            .map(|row| {
                let mut columns = row.split(',');
                let open_time = parse_naive_timestamp(columns.next(), "eastmoney kline time")?;
                let open = parse_decimal(columns.next(), "eastmoney kline open")?;
                let close = parse_decimal(columns.next(), "eastmoney kline close")?;
                let high = parse_decimal(columns.next(), "eastmoney kline high")?;
                let low = parse_decimal(columns.next(), "eastmoney kline low")?;
                let volume = parse_optional_decimal(columns.next(), "eastmoney kline volume")?;

                Ok(ProviderKline {
                    open_time,
                    close_time: open_time + bar_duration,
                    open,
                    high,
                    low,
                    close,
                    volume,
                })
            })
            .collect()
    }

    fn parse_latest_tick_response(body: &str) -> Result<ProviderTick, AppError> {
        let response: EastMoneyTickResponse =
            serde_json::from_str(body).map_err(|source| AppError::Provider {
                message: "failed to parse eastmoney tick response".into(),
                source: Some(Box::new(source)),
            })?;

        Ok(ProviderTick {
            price: parse_decimal(Some(response.data.price.as_str()), "eastmoney tick price")?,
            size: parse_optional_decimal(response.data.volume.as_deref(), "eastmoney tick volume")?,
            tick_time: parse_rfc3339_timestamp(
                Some(response.data.timestamp.as_str()),
                "eastmoney tick timestamp",
            )?,
        })
    }

    fn transport_not_wired_error() -> AppError {
        AppError::Provider {
            message: "eastmoney transport is not wired yet".into(),
            source: None,
        }
    }
}

#[async_trait]
impl MarketDataProvider for EastMoneyProvider {
    fn name(&self) -> &'static str {
        "eastmoney"
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
struct EastMoneyKlinesResponse {
    data: EastMoneyKlinesData,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EastMoneyKlinesData {
    klines: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EastMoneyTickResponse {
    data: EastMoneyTickData,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EastMoneyTickData {
    price: String,
    volume: Option<String>,
    timestamp: String,
}

#[allow(dead_code)]
fn parse_decimal(value: Option<&str>, field: &str) -> Result<Decimal, AppError> {
    let value = value.ok_or_else(|| AppError::Provider {
        message: format!("missing {field}"),
        source: None,
    })?;

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
fn parse_naive_timestamp(value: Option<&str>, field: &str) -> Result<DateTime<Utc>, AppError> {
    let value = value.ok_or_else(|| AppError::Provider {
        message: format!("missing {field}"),
        source: None,
    })?;
    let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M").map_err(|source| {
        AppError::Provider {
            message: format!("invalid {field}"),
            source: Some(Box::new(source)),
        }
    })?;

    Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

#[allow(dead_code)]
fn parse_rfc3339_timestamp(value: Option<&str>, field: &str) -> Result<DateTime<Utc>, AppError> {
    let value = value.ok_or_else(|| AppError::Provider {
        message: format!("missing {field}"),
        source: None,
    })?;

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

    use super::EastMoneyProvider;

    #[test]
    fn parses_kline_payload_into_provider_klines() {
        let klines = EastMoneyProvider::parse_klines_response(
            r#"{
                "data": {
                    "klines": [
                        "2024-01-02 09:30,10.1,10.8,11.0,10.0,12345"
                    ]
                }
            }"#,
            Timeframe::M15,
        )
        .expect("eastmoney kline payload should parse");

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
        let tick = EastMoneyProvider::parse_latest_tick_response(
            r#"{
                "data": {
                    "price": "10.8",
                    "volume": "2300",
                    "timestamp": "2024-01-02T09:30:05Z"
                }
            }"#,
        )
        .expect("eastmoney tick payload should parse");

        assert_eq!(tick.price, Decimal::new(108, 1));
        assert_eq!(tick.size, Some(Decimal::new(2_300, 0)));
        assert_eq!(tick.tick_time, utc("2024-01-02T09:30:05Z"));
    }

    #[test]
    fn parses_blank_optional_numeric_fields_as_none() {
        let klines = EastMoneyProvider::parse_klines_response(
            r#"{
                "data": {
                    "klines": [
                        "2024-01-02 09:30,10.1,10.8,11.0,10.0,   "
                    ]
                }
            }"#,
            Timeframe::M15,
        )
        .expect("blank eastmoney kline volume should parse as none");
        let tick = EastMoneyProvider::parse_latest_tick_response(
            r#"{
                "data": {
                    "price": "10.8",
                    "volume": "   ",
                    "timestamp": "2024-01-02T09:30:05Z"
                }
            }"#,
        )
        .expect("blank eastmoney tick size should parse as none");

        assert_eq!(klines[0].volume, None);
        assert_eq!(tick.size, None);
    }

    #[tokio::test]
    async fn unwired_provider_healthcheck_returns_provider_error() {
        let error = EastMoneyProvider
            .healthcheck()
            .await
            .expect_err("unwired provider should be unhealthy");

        match error {
            pa_core::AppError::Provider { message, .. } => {
                assert_eq!(message, "eastmoney transport is not wired yet");
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
