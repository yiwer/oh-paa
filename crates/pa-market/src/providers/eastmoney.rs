use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use pa_core::{AppError, Timeframe};
use reqwest::Url;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{MarketDataProvider, ProviderKline, ProviderTick};

#[derive(Debug, Clone)]
pub struct EastMoneyProvider {
    client: reqwest::Client,
    base_url: Url,
}

#[allow(dead_code)]
impl EastMoneyProvider {
    pub fn new(base_url: impl AsRef<str>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: parse_base_url(base_url.as_ref()),
        }
    }

    async fn get_text(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<String, AppError> {
        let url = self.base_url.join(path).map_err(|source| AppError::Provider {
            message: format!("failed to build eastmoney url for `{path}`"),
            source: Some(Box::new(source)),
        })?;
        let response = self
            .client
            .get(url)
            .query(query)
            .send()
            .await
            .map_err(|source| AppError::Provider {
                message: "failed to call eastmoney".into(),
                source: Some(Box::new(source)),
            })?;
        let response = response.error_for_status().map_err(|source| AppError::Provider {
            message: "eastmoney returned error status".into(),
            source: Some(Box::new(source)),
        })?;

        response.text().await.map_err(|source| AppError::Provider {
            message: "failed to read eastmoney response body".into(),
            source: Some(Box::new(source)),
        })
    }

    fn timeframe_klt(timeframe: Timeframe) -> &'static str {
        match timeframe {
            Timeframe::M15 => "15",
            Timeframe::H1 => "60",
            Timeframe::D1 => "101",
        }
    }

    fn parse_klines_response(
        body: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let response: EastMoneyKlinesResponse =
            serde_json::from_str(body).map_err(|source| AppError::Provider {
                message: "failed to parse eastmoney kline response".into(),
                source: Some(Box::new(source)),
            })?;
        ensure_success_rc(response.rc, "eastmoney kline")?;
        let bar_duration = chrono::Duration::from_std(timeframe.duration()).map_err(|source| {
            AppError::Provider {
                message: "invalid timeframe for eastmoney kline translation".into(),
                source: Some(Box::new(source)),
            }
        })?;
        let data = response.data.ok_or_else(|| AppError::Provider {
            message: "eastmoney kline response did not include data".into(),
            source: None,
        })?;

        data.klines
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
        ensure_success_rc(response.rc, "eastmoney tick")?;
        let data = response.data.ok_or_else(|| AppError::Provider {
            message: "eastmoney tick response did not include data".into(),
            source: None,
        })?;

        Ok(ProviderTick {
            price: parse_tick_price(&data)?,
            size: parse_tick_volume(&data)?,
            tick_time: parse_tick_time(&data)?,
        })
    }
}

#[async_trait]
impl MarketDataProvider for EastMoneyProvider {
    fn name(&self) -> &'static str {
        "eastmoney"
    }

    async fn fetch_klines(
        &self,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let body = self
            .get_text(
                "api/qt/stock/kline/get",
                &[
                    ("secid", provider_symbol.to_owned()),
                    ("fields1", "f1,f2,f3,f4,f5,f6".to_string()),
                    ("fields2", "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61".to_string()),
                    ("klt", Self::timeframe_klt(timeframe).to_owned()),
                    ("fqt", "1".to_string()),
                    ("beg", "0".to_string()),
                    ("end", "20500101".to_string()),
                    ("lmt", limit.to_string()),
                ],
            )
            .await?;

        let mut klines = Self::parse_klines_response(&body, timeframe)?;
        if klines.len() > limit {
            klines = klines.split_off(klines.len() - limit);
        }

        Ok(klines)
    }

    async fn fetch_latest_tick(&self, provider_symbol: &str) -> Result<ProviderTick, AppError> {
        let body = self
            .get_text("api/qt/stock/get", &[("secid", provider_symbol.to_owned())])
            .await?;

        Self::parse_latest_tick_response(&body)
    }

    async fn healthcheck(&self) -> Result<(), AppError> {
        let response = self
            .client
            .get(self.base_url.clone())
            .send()
            .await
            .map_err(|source| AppError::Provider {
                message: "failed to reach eastmoney health endpoint".into(),
                source: Some(Box::new(source)),
            })?;

        response.error_for_status().map_err(|source| AppError::Provider {
            message: "eastmoney healthcheck returned error status".into(),
            source: Some(Box::new(source)),
        })?;

        Ok(())
    }
}

impl Default for EastMoneyProvider {
    fn default() -> Self {
        Self::new("https://push2his.eastmoney.com/")
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EastMoneyKlinesResponse {
    #[serde(default)]
    rc: i64,
    data: Option<EastMoneyKlinesData>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EastMoneyKlinesData {
    klines: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EastMoneyTickResponse {
    #[serde(default)]
    rc: i64,
    data: Option<EastMoneyTickData>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EastMoneyTickData {
    #[serde(rename = "f43", alias = "price")]
    price: Option<EastMoneyNumber>,
    #[serde(rename = "f47", alias = "volume")]
    volume: Option<EastMoneyNumber>,
    #[serde(rename = "f59")]
    decimal: Option<u32>,
    #[serde(rename = "f86", alias = "timestamp")]
    timestamp: Option<EastMoneyTimestamp>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum EastMoneyNumber {
    Integer(i64),
    String(String),
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum EastMoneyTimestamp {
    Integer(i64),
    String(String),
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

    Ok(DateTime::<Utc>::from_naive_utc_and_offset(
        naive - chrono::Duration::hours(8),
        Utc,
    ))
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

fn parse_tick_price(data: &EastMoneyTickData) -> Result<Decimal, AppError> {
    let scale = data.decimal.unwrap_or(2);
    let value = data.price.as_ref().ok_or_else(|| AppError::Provider {
        message: "missing eastmoney tick price".into(),
        source: None,
    })?;

    match value {
        EastMoneyNumber::Integer(value) => Ok(Decimal::new(*value, scale)),
        EastMoneyNumber::String(value) => parse_decimal(Some(value.as_str()), "eastmoney tick price"),
    }
}

fn parse_tick_volume(data: &EastMoneyTickData) -> Result<Option<Decimal>, AppError> {
    match data.volume.as_ref() {
        Some(EastMoneyNumber::Integer(value)) => Ok(Some(Decimal::new(*value, 0))),
        Some(EastMoneyNumber::String(value)) => {
            parse_optional_decimal(Some(value.as_str()), "eastmoney tick volume")
        }
        None => Ok(None),
    }
}

fn parse_tick_time(data: &EastMoneyTickData) -> Result<DateTime<Utc>, AppError> {
    let value = data.timestamp.as_ref().ok_or_else(|| AppError::Provider {
        message: "missing eastmoney tick timestamp".into(),
        source: None,
    })?;

    match value {
        EastMoneyTimestamp::Integer(value) => {
            DateTime::<Utc>::from_timestamp(*value, 0).ok_or_else(|| AppError::Provider {
                message: "invalid eastmoney tick timestamp".into(),
                source: None,
            })
        }
        EastMoneyTimestamp::String(value) => {
            parse_rfc3339_timestamp(Some(value.as_str()), "eastmoney tick timestamp")
        }
    }
}

fn ensure_success_rc(rc: i64, field: &str) -> Result<(), AppError> {
    if rc == 0 {
        return Ok(());
    }

    Err(AppError::Provider {
        message: format!("{field} response returned rc={rc}"),
        source: None,
    })
}

fn parse_base_url(value: &str) -> Url {
    let normalized = if value.ends_with('/') {
        value.to_owned()
    } else {
        format!("{value}/")
    };

    Url::parse(&normalized).expect("provider base url should be valid")
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use pa_core::Timeframe;
    use rust_decimal::Decimal;

    use super::EastMoneyProvider;

    #[test]
    fn parses_kline_payload_into_provider_klines() {
        let klines = EastMoneyProvider::parse_klines_response(
            r#"{
                "rc": 0,
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
        assert_eq!(klines[0].open_time, utc("2024-01-02T01:30:00Z"));
        assert_eq!(klines[0].close_time, utc("2024-01-02T01:45:00Z"));
    }

    #[test]
    fn parses_tick_payload_into_provider_tick() {
        let tick = EastMoneyProvider::parse_latest_tick_response(
            r#"{
                "rc": 0,
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
                "rc": 0,
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
                "rc": 0,
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

    #[test]
    fn maps_timeframe_to_eastmoney_klt() {
        assert_eq!(EastMoneyProvider::timeframe_klt(Timeframe::M15), "15");
        assert_eq!(EastMoneyProvider::timeframe_klt(Timeframe::H1), "60");
        assert_eq!(EastMoneyProvider::timeframe_klt(Timeframe::D1), "101");
    }

    #[test]
    fn parses_live_eastmoney_quote_shape() {
        let tick = EastMoneyProvider::parse_latest_tick_response(
            r#"{
                "rc": 0,
                "data": {
                    "f43": 1108,
                    "f47": 806946,
                    "f59": 2,
                    "f86": 1776756873
                }
            }"#,
        )
        .expect("live eastmoney quote shape should parse");

        assert_eq!(tick.price, Decimal::new(1108, 2));
        assert_eq!(tick.size, Some(Decimal::new(806_946, 0)));
    }

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("fixture timestamp should be valid")
            .with_timezone(&Utc)
    }
}
