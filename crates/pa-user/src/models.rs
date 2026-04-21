use chrono::{DateTime, NaiveDate, Utc};
use pa_core::Timeframe;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSubscription {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionSnapshot {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub side: PositionSide,
    pub quantity: Decimal,
    pub average_cost: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualUserAnalysisRequest {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub bar_close_time: DateTime<Utc>,
    pub trading_date: NaiveDate,
    pub analysis_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserAnalysisReport {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub subscriptions: Vec<UserSubscription>,
    pub positions: Vec<PositionSnapshot>,
    pub bar_analysis: Value,
    pub daily_market_context: Value,
}

impl UserAnalysisReport {
    pub fn analysis_payload(&self) -> Value {
        serde_json::json!({
            "user_id": self.user_id,
            "instrument_id": self.instrument_id,
            "subscriptions": self.subscriptions,
            "positions": self.positions,
            "bar_analysis": self.bar_analysis,
            "daily_market_context": self.daily_market_context,
        })
    }
}
