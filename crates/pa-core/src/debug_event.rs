use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DebugEvent {
    KlineIngested {
        instrument_id: Uuid,
        timeframe: String,
        open_time: DateTime<Utc>,
        provider: String,
        latency_ms: u64,
    },
    ProviderFallback {
        instrument_id: Uuid,
        primary_provider: String,
        fallback_provider: String,
        error: String,
    },
    NormalizationResult {
        instrument_id: Uuid,
        timeframe: String,
        open_time: DateTime<Utc>,
        success: bool,
        error: Option<String>,
    },
    TaskStatusChanged {
        task_id: Uuid,
        instrument_id: Uuid,
        task_type: String,
        old_status: String,
        new_status: String,
    },
    AttemptCompleted {
        task_id: Uuid,
        attempt_number: i32,
        provider: String,
        model: String,
        latency_ms: u64,
        success: bool,
        error: Option<String>,
    },
    OpenBarUpdate {
        instrument_id: Uuid,
        timeframe: String,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
    },
}
