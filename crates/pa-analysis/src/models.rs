use chrono::{DateTime, NaiveDate, Utc};
use pa_core::Timeframe;
use pa_orchestrator::AnalysisBarState;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, PartialEq)]
pub struct PaStateBar {
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub bar_state: AnalysisBarState,
    pub bar_open_time: DateTime<Utc>,
    pub bar_close_time: DateTime<Utc>,
    pub analysis_version: String,
    pub state_json: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedBarAnalysisInput {
    pub instrument_id: Uuid,
    #[serde(with = "timeframe_serde")]
    pub timeframe: Timeframe,
    pub bar_open_time: DateTime<Utc>,
    pub bar_close_time: DateTime<Utc>,
    #[serde(with = "bar_state_serde")]
    pub bar_state: AnalysisBarState,
    pub shared_pa_state_json: Value,
    pub recent_pa_states_json: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedBarAnalysisOutput {
    pub bar_identity: Value,
    pub bar_summary: Value,
    pub market_story: Value,
    pub bullish_case: Value,
    pub bearish_case: Value,
    pub two_sided_balance: Value,
    pub key_levels: Value,
    pub signal_bar_verdict: Value,
    pub continuation_path: Value,
    pub reversal_path: Value,
    pub invalidation_map: Value,
    pub follow_through_checkpoints: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedPaStateBarInput {
    pub instrument_id: Uuid,
    #[serde(with = "timeframe_serde")]
    pub timeframe: Timeframe,
    #[serde(with = "bar_state_serde")]
    pub bar_state: AnalysisBarState,
    pub bar_open_time: DateTime<Utc>,
    pub bar_close_time: DateTime<Utc>,
    pub bar_json: Value,
    pub market_context_json: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedPaStateBarOutput {
    pub bar_identity: Value,
    pub market_session_context: Value,
    pub bar_observation: Value,
    pub bar_shape: Value,
    pub location_context: Value,
    pub multi_timeframe_alignment: Value,
    pub support_resistance_map: Value,
    pub signal_assessment: Value,
    pub decision_tree_state: Value,
    pub evidence_log: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedDailyContextInput {
    pub instrument_id: Uuid,
    pub trading_date: NaiveDate,
    pub recent_pa_states_json: Value,
    pub recent_shared_bar_analyses_json: Value,
    pub multi_timeframe_structure_json: Value,
    pub market_background_json: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedDailyContextOutput {
    pub context_identity: Value,
    pub market_background: Value,
    pub dominant_structure: Value,
    pub intraday_vs_higher_timeframe_state: Value,
    pub key_support_levels: Value,
    pub key_resistance_levels: Value,
    pub signal_bars: Value,
    pub candle_pattern_map: Value,
    pub decision_tree_nodes: Value,
    pub liquidity_context: Value,
    pub scenario_map: Value,
    pub risk_notes: Value,
    pub session_playbook: Value,
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
