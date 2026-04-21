use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use pa_core::{AppError, Timeframe};
use reqwest::Url;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{MarketDataProvider, ProviderKline, ProviderTick};

#[derive(Debug, Clone)]
pub struct TwelveDataProvider {
    client: reqwest::Client,
    base_url: Url,
    api_key: String,
}

#[allow(dead_code)]
impl TwelveDataProvider {
    pub fn new(base_url: impl AsRef<str>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: parse_base_url(base_url.as_ref()),
            api_key: api_key.into(),
        }
    }

    async fn get_text(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<String, AppError> {
        let url = self.base_url.join(path).map_err(|source| AppError::Provider {
            message: format!("failed to build twelvedata url for `{path}`"),
            source: Some(Box::new(source)),
        })?;
        let response = self
            .client
            .get(url)
            .query(query)
            .send()
            .await
            .map_err(|source| AppError::Provider {
                message: "failed to call twelvedata".into(),
                source: Some(Box::new(source)),
            })?;
        let response = response.error_for_status().map_err(|source| AppError::Provider {
            message: "twelvedata returned error status".into(),
            source: Some(Box::new(source)),
        })?;

        response.text().await.map_err(|source| AppError::Provider {
            message: "failed to read twelvedata response body".into(),
            source: Some(Box::new(source)),
        })
    }

    fn timeframe_interval(timeframe: Timeframe) -> &'static str {
        match timeframe {
            Timeframe::M15 => "15min",
            Timeframe::H1 => "1h",
            Timeframe::D1 => "1day",
        }
    }

    fn parse_klines_response(
        body: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let response: TwelveDataKlinesResponse =
            serde_json::from_str(body).map_err(|source| AppError::Provider {
                message: "failed to parse twelvedata kline response".into(),
                source: Some(Box::new(source)),
            })?;
        ensure_success_status(response.status.as_deref(), response.code, response.message.as_deref())?;
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
                let open_time = parse_twelvedata_datetime(&row.datetime, "twelvedata kline datetime")?;

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
        ensure_success_status(response.status.as_deref(), response.code, response.message.as_deref())?;

        Ok(ProviderTick {
            price: parse_decimal(&response.price, "twelvedata tick price")?,
            size: parse_optional_decimal(response.volume.as_deref(), "twelvedata tick volume")?,
            tick_time: parse_tick_time(&response)?,
        })
    }
}

#[async_trait]
impl MarketDataProvider for TwelveDataProvider {
    fn name(&self) -> &'static str {
        "twelvedata"
    }

    async fn fetch_klines(
        &self,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let body = self
            .get_text(
                "time_series",
                &[
                    ("symbol", provider_symbol.to_owned()),
                    ("interval", Self::timeframe_interval(timeframe).to_owned()),
                    ("outputsize", limit.to_string()),
                    ("order", "asc".to_string()),
                    ("timezone", "UTC".to_string()),
                    ("apikey", self.api_key.clone()),
                ],
            )
            .await?;

        Self::parse_klines_response(&body, timeframe)
    }

    async fn fetch_latest_tick(&self, provider_symbol: &str) -> Result<ProviderTick, AppError> {
        let body = self
            .get_text(
                "quote",
                &[
                    ("symbol", provider_symbol.to_owned()),
                    ("timezone", "UTC".to_string()),
                    ("apikey", self.api_key.clone()),
                ],
            )
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
                message: "failed to reach twelvedata health endpoint".into(),
                source: Some(Box::new(source)),
            })?;

        response.error_for_status().map_err(|source| AppError::Provider {
            message: "twelvedata healthcheck returned error status".into(),
            source: Some(Box::new(source)),
        })?;

        Ok(())
    }
}

impl Default for TwelveDataProvider {
    fn default() -> Self {
        Self::new("https://api.twelvedata.com/", "")
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TwelveDataKlinesResponse {
    status: Option<String>,
    code: Option<i64>,
    message: Option<String>,
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
    #[serde(alias = "close")]
    price: String,
    volume: Option<String>,
    last_quote_at: Option<TwelveDataTimestamp>,
    timestamp: Option<TwelveDataTimestamp>,
    datetime: Option<String>,
    status: Option<String>,
    code: Option<i64>,
    message: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TwelveDataTimestamp {
    Integer(i64),
    String(String),
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

fn parse_twelvedata_datetime(value: &str, field: &str) -> Result<DateTime<Utc>, AppError> {
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(value) {
        return Ok(timestamp.with_timezone(&Utc));
    }

    if let Ok(timestamp) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc));
    }

    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let timestamp = date.and_hms_opt(0, 0, 0).ok_or_else(|| AppError::Provider {
            message: format!("invalid {field}"),
            source: None,
        })?;
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc));
    }

    Err(AppError::Provider {
        message: format!("invalid {field}"),
        source: None,
    })
}

fn parse_tick_time(response: &TwelveDataTickResponse) -> Result<DateTime<Utc>, AppError> {
    if let Some(timestamp) = &response.last_quote_at {
        return match timestamp {
            TwelveDataTimestamp::Integer(value) => {
                DateTime::<Utc>::from_timestamp(*value, 0).ok_or_else(|| AppError::Provider {
                    message: "invalid twelvedata last_quote_at timestamp".into(),
                    source: None,
                })
            }
            TwelveDataTimestamp::String(value) => {
                parse_rfc3339_timestamp(value, "twelvedata last_quote_at timestamp")
                    .or_else(|_| parse_twelvedata_datetime(value, "twelvedata last_quote_at timestamp"))
            }
        };
    }

    if let Some(timestamp) = &response.timestamp {
        return match timestamp {
            TwelveDataTimestamp::Integer(value) => {
                DateTime::<Utc>::from_timestamp(*value, 0).ok_or_else(|| AppError::Provider {
                    message: "invalid twelvedata tick timestamp".into(),
                    source: None,
                })
            }
            TwelveDataTimestamp::String(value) => {
                parse_rfc3339_timestamp(value, "twelvedata tick timestamp")
                    .or_else(|_| parse_twelvedata_datetime(value, "twelvedata tick timestamp"))
            }
        };
    }

    if let Some(datetime) = response.datetime.as_deref() {
        return parse_twelvedata_datetime(datetime, "twelvedata tick datetime");
    }

    Err(AppError::Provider {
        message: "missing twelvedata tick timestamp".into(),
        source: None,
    })
}

fn ensure_success_status(
    status: Option<&str>,
    code: Option<i64>,
    message: Option<&str>,
) -> Result<(), AppError> {
    if status.is_some_and(|status| status != "ok") {
        return Err(AppError::Provider {
            message: format!(
                "twelvedata returned status={} code={} message={}",
                status.unwrap_or("unknown"),
                code.unwrap_or_default(),
                message.unwrap_or("unknown error"),
            ),
            source: None,
        });
    }

    Ok(())
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
                "timestamp": "2024-01-02T09:30:05Z",
                "status": "ok"
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
                "status": "ok",
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
                "timestamp": "2024-01-02T09:30:05Z",
                "status": "ok"
            }"#,
        )
        .expect("blank twelvedata tick size should parse as none");

        assert_eq!(klines[0].volume, None);
        assert_eq!(tick.size, None);
    }

    #[test]
    fn maps_timeframe_to_twelvedata_interval() {
        assert_eq!(TwelveDataProvider::timeframe_interval(Timeframe::M15), "15min");
        assert_eq!(TwelveDataProvider::timeframe_interval(Timeframe::H1), "1h");
        assert_eq!(TwelveDataProvider::timeframe_interval(Timeframe::D1), "1day");
    }

    #[test]
    fn parses_naive_datetime_formats_from_live_twelvedata_payloads() {
        let klines = TwelveDataProvider::parse_klines_response(
            r#"{
                "status": "ok",
                "values": [
                    {
                        "datetime": "2024-01-02 09:30:00",
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
        .expect("naive datetime payload should parse");

        assert_eq!(klines[0].open_time, utc("2024-01-02T09:30:00Z"));

        let daily = TwelveDataProvider::parse_klines_response(
            r#"{
                "status": "ok",
                "values": [
                    {
                        "datetime": "2024-01-02",
                        "open": "10.1",
                        "high": "11.0",
                        "low": "10.0",
                        "close": "10.8",
                        "volume": "12345"
                    }
                ]
            }"#,
            Timeframe::D1,
        )
        .expect("date-only payload should parse");

        assert_eq!(daily[0].open_time, utc("2024-01-02T00:00:00Z"));
    }

    #[test]
    fn parses_integer_quote_timestamp_from_live_twelvedata_payloads() {
        let tick = TwelveDataProvider::parse_latest_tick_response(
            r#"{
                "price": "10.8",
                "volume": "2300",
                "timestamp": 1704187805,
                "status": "ok"
            }"#,
        )
        .expect("integer timestamp payload should parse");

        assert_eq!(tick.tick_time, utc("2024-01-02T09:30:05Z"));
    }

    #[test]
    fn parses_last_quote_at_from_live_twelvedata_quote_payloads() {
        let tick = TwelveDataProvider::parse_latest_tick_response(
            r#"{
                "close": "75381.57",
                "volume": "",
                "timestamp": 1776729600,
                "last_quote_at": 1776791820,
                "is_market_open": true
            }"#,
        )
        .expect("live last_quote_at payload should parse");

        assert_eq!(tick.tick_time, utc("2026-04-21T17:17:00Z"));
    }

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("fixture timestamp should be valid")
            .with_timezone(&Utc)
    }
}
