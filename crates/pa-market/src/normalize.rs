use pa_core::AppError;

use crate::models::ProviderKline;

pub fn normalize_kline(bar: ProviderKline) -> Result<ProviderKline, AppError> {
    if bar.high < bar.open || bar.high < bar.close {
        return Err(AppError::Validation {
            message: "high must be greater than or equal to open and close".to_string(),
            source: None,
        });
    }

    if bar.low > bar.open || bar.low > bar.close {
        return Err(AppError::Validation {
            message: "low must be less than or equal to open and close".to_string(),
            source: None,
        });
    }

    if bar.close_time <= bar.open_time {
        return Err(AppError::Validation {
            message: "close_time must be later than open_time".to_string(),
            source: None,
        });
    }

    if bar.volume.is_some_and(|volume| volume.is_sign_negative()) {
        return Err(AppError::Validation {
            message: "volume must be greater than or equal to zero".to_string(),
            source: None,
        });
    }

    Ok(bar)
}
