use chrono::{DateTime, NaiveDate, Utc};
use pa_core::Timeframe;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct BarAnalysis {
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub bar_close_time: DateTime<Utc>,
    pub analysis_version: String,
    pub result_json: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyMarketContext {
    pub instrument_id: Uuid,
    pub trading_date: NaiveDate,
    pub analysis_version: String,
    pub context_json: Value,
}
