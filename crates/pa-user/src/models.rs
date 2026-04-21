use chrono::{DateTime, NaiveDate, Utc};
use pa_core::Timeframe;
use pa_orchestrator::AnalysisBarState;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualUserAnalysisInput {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    #[serde(with = "timeframe_serde")]
    pub timeframe: Timeframe,
    #[serde(with = "bar_state_serde")]
    pub bar_state: AnalysisBarState,
    pub bar_open_time: Option<DateTime<Utc>>,
    pub bar_close_time: Option<DateTime<Utc>>,
    pub trading_date: Option<NaiveDate>,
    pub positions_json: Value,
    pub subscriptions_json: Value,
    pub shared_bar_analysis_json: Value,
    pub shared_daily_context_json: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledUserAnalysisInput {
    pub schedule_id: Uuid,
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    #[serde(with = "timeframe_serde")]
    pub timeframe: Timeframe,
    #[serde(with = "bar_state_serde")]
    pub bar_state: AnalysisBarState,
    pub bar_open_time: Option<DateTime<Utc>>,
    pub bar_close_time: Option<DateTime<Utc>>,
    pub trading_date: Option<NaiveDate>,
    pub positions_json: Value,
    pub subscriptions_json: Value,
    pub shared_bar_analysis_json: Value,
    pub shared_daily_context_json: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPositionAdviceOutput {
    pub position_state: Value,
    pub market_read_through: Value,
    pub bullish_path_for_user: Value,
    pub bearish_path_for_user: Value,
    pub hold_reduce_exit_conditions: Value,
    pub risk_control_levels: Value,
    pub invalidations: Value,
    pub action_candidates: Value,
}

mod timeframe_serde {
    use pa_core::Timeframe;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Timeframe, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value.as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Timeframe, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "15m" => Ok(Timeframe::M15),
            "1h" => Ok(Timeframe::H1),
            "1d" => Ok(Timeframe::D1),
            other => Err(serde::de::Error::custom(format!(
                "invalid timeframe: {other}"
            ))),
        }
    }
}

mod bar_state_serde {
    use pa_orchestrator::AnalysisBarState;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &AnalysisBarState, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value.as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<AnalysisBarState, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        AnalysisBarState::from_db(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid bar state: {value}")))
    }
}
