use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderKline {
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
}

impl ProviderKline {
    pub fn fixture() -> Self {
        let open_time = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .expect("valid fixture open time")
            .with_timezone(&Utc);
        let close_time = DateTime::parse_from_rfc3339("2024-01-01T00:15:00Z")
            .expect("valid fixture close time")
            .with_timezone(&Utc);

        Self {
            open_time,
            close_time,
            open: Decimal::new(100, 0),
            high: Decimal::new(110, 0),
            low: Decimal::new(90, 0),
            close: Decimal::new(105, 0),
            volume: Some(Decimal::new(1_000, 0)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderTick {
    pub price: Decimal,
    pub size: Option<Decimal>,
    pub tick_time: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalKline {
    pub instrument_id: Uuid,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
    pub source_provider: &'static str,
}
