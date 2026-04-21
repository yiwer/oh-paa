use chrono::Utc;
use pa_core::{AppError, Timeframe};
use uuid::Uuid;

use crate::{
    normalize_kline,
    provider::ProviderRouter,
    repository::{CanonicalKlineRepository, CanonicalKlineRow},
};

pub async fn backfill_canonical_klines(
    router: &ProviderRouter,
    repository: &dyn CanonicalKlineRepository,
    instrument_id: Uuid,
    provider_symbol: &str,
    timeframe: Timeframe,
    limit: usize,
    primary_provider: &str,
    fallback_provider: &str,
) -> Result<(), AppError> {
    let routed = router
        .fetch_klines_with_fallback_source(
            primary_provider,
            fallback_provider,
            provider_symbol,
            timeframe,
            limit,
        )
        .await?;

    for bar in routed.klines {
        let normalized = normalize_kline(bar)?;
        if normalized.close_time > Utc::now() {
            continue;
        }

        repository
            .upsert_canonical_kline(CanonicalKlineRow {
                instrument_id,
                timeframe,
                open_time: normalized.open_time,
                close_time: normalized.close_time,
                open: normalized.open,
                high: normalized.high,
                low: normalized.low,
                close: normalized.close,
                volume: normalized.volume,
                source_provider: routed.provider_name.to_string(),
            })
            .await?;
    }

    Ok(())
}
