use pa_core::AppError;

use crate::models::ProviderPolicy;

pub fn resolve_policy(
    instrument_policy: Option<&ProviderPolicy>,
    market_policy: Option<&ProviderPolicy>,
) -> Result<ProviderPolicy, AppError> {
    instrument_policy
        .cloned()
        .or_else(|| market_policy.cloned())
        .ok_or_else(|| AppError::Validation {
            message: "provider policy is required for instrument resolution".to_string(),
            source: None,
        })
}
