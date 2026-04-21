use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::AnalysisBarState;

pub fn build_shared_bar_dedupe_key(
    instrument_id: Uuid,
    timeframe: Timeframe,
    bar_close_time: DateTime<Utc>,
    prompt_key: &str,
    prompt_version: &str,
    bar_state: AnalysisBarState,
) -> Option<String> {
    matches!(bar_state, AnalysisBarState::Closed).then(|| {
        format!(
            "shared_bar_analysis:{instrument_id}:{timeframe}:{bar_close_time}:{prompt_key}:{prompt_version}:closed",
            timeframe = timeframe.as_str(),
            bar_close_time = bar_close_time.to_rfc3339(),
        )
    })
}

pub fn sha256_json(value: &serde_json::Value) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value).map_err(|err| AppError::Analysis {
        message: format!("failed to serialize json for hashing: {err}"),
        source: None,
    })?;

    Ok(format!("{:x}", Sha256::digest(bytes)))
}
