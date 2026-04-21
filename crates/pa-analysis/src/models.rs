use chrono::{DateTime, NaiveDate, Utc};
use pa_core::Timeframe;
use pa_orchestrator::AnalysisBarState;
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
pub struct SharedBarAnalysisInput {
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub bar_open_time: DateTime<Utc>,
    pub bar_close_time: DateTime<Utc>,
    pub bar_state: AnalysisBarState,
    pub canonical_bar_json: Value,
    pub structure_context_json: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedBarAnalysisOutput {
    pub bar_state: String,
    pub bar_classification: Value,
    pub bullish_case: Value,
    pub bearish_case: Value,
    pub two_sided_summary: Value,
    pub nearby_levels: Value,
    pub signal_strength: Value,
    pub continuation_scenarios: Value,
    pub reversal_scenarios: Value,
    pub invalidation_levels: Value,
    pub execution_bias_notes: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedDailyContextInput {
    pub instrument_id: Uuid,
    pub trading_date: NaiveDate,
    pub m15_structure_json: Value,
    pub h1_structure_json: Value,
    pub d1_structure_json: Value,
    pub recent_shared_bar_analyses_json: Value,
    pub key_levels_json: Value,
    pub signal_bar_candidates_json: Value,
    pub market_background_json: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedDailyContextOutput {
    pub market_background: Value,
    pub market_structure: Value,
    pub key_support_levels: Value,
    pub key_resistance_levels: Value,
    pub signal_bars: Value,
    pub candle_patterns: Value,
    pub decision_tree_nodes: Value,
    pub liquidity_context: Value,
    pub risk_notes: Value,
    pub scenario_map: Value,
}
