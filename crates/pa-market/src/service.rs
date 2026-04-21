use chrono::{DateTime, Duration, Utc};
use pa_core::{AppError, Timeframe};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    normalize_kline,
    provider::ProviderRouter,
    repository::{CanonicalKlineQuery, CanonicalKlineRepository, CanonicalKlineRow},
    session::MarketSessionProfile,
};

#[derive(Debug, Clone)]
pub struct BackfillCanonicalKlinesRequest<'a> {
    pub instrument_id: Uuid,
    pub primary_provider_symbol: &'a str,
    pub fallback_provider_symbol: &'a str,
    pub timeframe: Timeframe,
    pub limit: usize,
    pub primary_provider: &'a str,
    pub fallback_provider: &'a str,
    pub market_code: Option<&'a str>,
    pub market_timezone: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AggregatedKline {
    pub instrument_id: Uuid,
    pub source_timeframe: Timeframe,
    pub timeframe: Timeframe,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
    pub child_bar_count: usize,
    pub expected_child_bar_count: usize,
    pub complete: bool,
    pub source_provider: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AggregateCanonicalKlinesRequest {
    pub instrument_id: Uuid,
    pub source_timeframe: Timeframe,
    pub target_timeframe: Timeframe,
    pub start_open_time: Option<DateTime<Utc>>,
    pub end_open_time: Option<DateTime<Utc>>,
    pub limit: usize,
    pub market_code: Option<String>,
    pub market_timezone: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeriveOpenBarRequest<'a> {
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub market_code: Option<String>,
    pub market_timezone: Option<String>,
    pub primary_provider: &'a str,
    pub fallback_provider: &'a str,
    pub primary_provider_symbol: &'a str,
    pub fallback_provider_symbol: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedOpenBar {
    pub instrument_id: Uuid,
    pub source_timeframe: Timeframe,
    pub timeframe: Timeframe,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub latest_tick_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub child_bar_count: usize,
    pub source_provider: String,
}

pub async fn backfill_canonical_klines(
    router: &ProviderRouter,
    repository: &dyn CanonicalKlineRepository,
    request: BackfillCanonicalKlinesRequest<'_>,
) -> Result<(), AppError> {
    let session_profile =
        MarketSessionProfile::from_market(request.market_code, request.market_timezone);
    let routed = router
        .fetch_klines_with_fallback_source(
            request.primary_provider,
            request.fallback_provider,
            request.primary_provider_symbol,
            request.fallback_provider_symbol,
            request.timeframe,
            request.limit,
        )
        .await?;

    for bar in routed.klines {
        let normalized = normalize_kline(bar)?;
        if normalized.close_time > Utc::now() {
            continue;
        }
        if !session_profile.accepts_bar_open(request.timeframe, normalized.open_time) {
            continue;
        }

        repository
            .upsert_canonical_kline(CanonicalKlineRow {
                instrument_id: request.instrument_id,
                timeframe: request.timeframe,
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

pub async fn list_canonical_klines(
    repository: &dyn CanonicalKlineRepository,
    query: CanonicalKlineQuery,
) -> Result<Vec<CanonicalKlineRow>, AppError> {
    repository.list_canonical_klines(query).await
}

pub async fn aggregate_canonical_klines(
    repository: &dyn CanonicalKlineRepository,
    request: AggregateCanonicalKlinesRequest,
) -> Result<Vec<AggregatedKline>, AppError> {
    let session_profile = MarketSessionProfile::from_market(
        request.market_code.as_deref(),
        request.market_timezone.as_deref(),
    );
    let source_rows = repository
        .list_canonical_klines(CanonicalKlineQuery {
            instrument_id: request.instrument_id,
            timeframe: request.source_timeframe,
            start_open_time: request.start_open_time,
            end_open_time: request.end_open_time,
            limit: request.limit.saturating_mul(session_profile.expected_child_bar_count(
                request.source_timeframe,
                request.target_timeframe,
            )?),
            descending: true,
        })
        .await?;
    let mut source_rows = source_rows
        .into_iter()
        .filter(|row| session_profile.accepts_bar_open(request.source_timeframe, row.open_time))
        .collect::<Vec<_>>();
    source_rows.reverse();

    aggregate_rows(
        &source_rows,
        request.instrument_id,
        request.source_timeframe,
        request.target_timeframe,
        &session_profile,
    )
}

pub async fn derive_open_bar(
    router: &ProviderRouter,
    repository: &dyn CanonicalKlineRepository,
    request: DeriveOpenBarRequest<'_>,
) -> Result<Option<DerivedOpenBar>, AppError> {
    let session_profile = MarketSessionProfile::from_market(
        request.market_code.as_deref(),
        request.market_timezone.as_deref(),
    );
    let routed_tick = router
        .fetch_latest_tick_with_fallback_source(
            request.primary_provider,
            request.fallback_provider,
            request.primary_provider_symbol,
            request.fallback_provider_symbol,
        )
        .await?;
    let Some(bucket) = session_profile.current_bucket_for_tick(request.timeframe, routed_tick.tick.tick_time)? else {
        return Ok(None);
    };
    let source_timeframe = source_timeframe_for_open_bar(request.timeframe);
    let source_duration = duration_from_timeframe(source_timeframe)?;
    let bucket_end_open_time = bucket
        .close_time
        .checked_sub_signed(source_duration)
        .ok_or_else(|| AppError::Validation {
            message: format!(
                "failed to compute bucket end open time for {} {}",
                request.timeframe,
                bucket.close_time.to_rfc3339()
            ),
            source: None,
        })?;
    let bucket_rows = repository
        .list_canonical_klines(CanonicalKlineQuery {
            instrument_id: request.instrument_id,
            timeframe: source_timeframe,
            start_open_time: Some(bucket.open_time),
            end_open_time: Some(bucket_end_open_time),
            limit: session_profile.expected_child_bar_count(source_timeframe, request.timeframe)?,
            descending: false,
        })
        .await?
        .into_iter()
        .filter(|row| session_profile.accepts_bar_open(source_timeframe, row.open_time))
        .filter(|row| row.close_time <= routed_tick.tick.tick_time)
        .collect::<Vec<_>>();

    if let Some(last_row) = bucket_rows.last()
        && last_row.close_time > routed_tick.tick.tick_time
    {
        return Err(AppError::Validation {
            message: format!(
                "latest tick {} is older than latest closed child bar {}",
                routed_tick.tick.tick_time.to_rfc3339(),
                last_row.close_time.to_rfc3339()
            ),
            source: None,
        });
    }

    let (open, mut high, mut low, child_bar_count) =
        if let Some(first_row) = bucket_rows.first() {
            (
                first_row.open,
                bucket_rows
                    .iter()
                    .map(|row| row.high)
                    .max()
                    .expect("bucket rows should not be empty"),
                bucket_rows
                    .iter()
                    .map(|row| row.low)
                    .min()
                    .expect("bucket rows should not be empty"),
                bucket_rows.len(),
            )
        } else if let Some(previous_row) =
            latest_closed_row_before(
                repository,
                request.instrument_id,
                source_timeframe,
                bucket.open_time,
                &session_profile,
            )
            .await?
        {
            (
                previous_row.close,
                previous_row.close,
                previous_row.close,
                0,
            )
        } else {
            (
                routed_tick.tick.price,
                routed_tick.tick.price,
                routed_tick.tick.price,
                0,
            )
        };

    high = high.max(routed_tick.tick.price);
    low = low.min(routed_tick.tick.price);

    Ok(Some(DerivedOpenBar {
        instrument_id: request.instrument_id,
        source_timeframe,
        timeframe: request.timeframe,
        open_time: bucket.open_time,
        close_time: bucket.close_time,
        latest_tick_time: routed_tick.tick.tick_time,
        open,
        high,
        low,
        close: routed_tick.tick.price,
        child_bar_count,
        source_provider: routed_tick.provider_name,
    }))
}

fn aggregate_rows(
    rows: &[CanonicalKlineRow],
    instrument_id: Uuid,
    source_timeframe: Timeframe,
    target_timeframe: Timeframe,
    session_profile: &MarketSessionProfile,
) -> Result<Vec<AggregatedKline>, AppError> {
    let mut aggregated = Vec::new();
    let mut index = 0usize;

    while index < rows.len() {
        let bucket = session_profile.bucket_for_open_time(
            source_timeframe,
            target_timeframe,
            rows[index].open_time,
        )?;
        let bucket_rows = rows[index..]
            .iter()
            .take_while(|row| row.open_time < bucket.close_time)
            .cloned()
            .collect::<Vec<_>>();
        index += bucket_rows.len();

        if bucket_rows.is_empty() {
            continue;
        }

        let first = &bucket_rows[0];
        let last = &bucket_rows[bucket_rows.len() - 1];
        let expected_child_bar_count = bucket.expected_open_times.len();
        let complete = bucket_rows.len() == expected_child_bar_count
            && bucket_rows
                .iter()
                .zip(bucket.expected_open_times.iter())
                .all(|(row, expected_open_time)| row.open_time == *expected_open_time);
        let volume = sum_optional_volume(&bucket_rows);
        let source_provider = if bucket_rows
            .iter()
            .all(|row| row.source_provider == first.source_provider)
        {
            first.source_provider.clone()
        } else {
            "mixed".to_string()
        };

        aggregated.push(AggregatedKline {
            instrument_id,
            source_timeframe,
            timeframe: target_timeframe,
            open_time: bucket.open_time,
            close_time: bucket.close_time,
            open: first.open,
            high: bucket_rows
                .iter()
                .map(|row| row.high)
                .max()
                .expect("bucket rows should not be empty"),
            low: bucket_rows
                .iter()
                .map(|row| row.low)
                .min()
                .expect("bucket rows should not be empty"),
            close: last.close,
            volume,
            child_bar_count: bucket_rows.len(),
            expected_child_bar_count,
            complete,
            source_provider,
        });
    }

    Ok(aggregated)
}

fn sum_optional_volume(rows: &[CanonicalKlineRow]) -> Option<Decimal> {
    let mut total = Decimal::ZERO;
    let mut has_volume = false;

    for row in rows {
        if let Some(volume) = row.volume {
            total += volume;
            has_volume = true;
        }
    }

    has_volume.then_some(total)
}

async fn latest_closed_row_before(
    repository: &dyn CanonicalKlineRepository,
    instrument_id: Uuid,
    timeframe: Timeframe,
    bucket_open_time: DateTime<Utc>,
    session_profile: &MarketSessionProfile,
) -> Result<Option<CanonicalKlineRow>, AppError> {
    Ok(repository
        .list_canonical_klines(CanonicalKlineQuery {
            instrument_id,
            timeframe,
            start_open_time: None,
            end_open_time: Some(bucket_open_time),
            limit: 8,
            descending: true,
        })
        .await?
        .into_iter()
        .filter(|row| session_profile.accepts_bar_open(timeframe, row.open_time))
        .find(|row| row.close_time <= bucket_open_time))
}

fn source_timeframe_for_open_bar(timeframe: Timeframe) -> Timeframe {
    match timeframe {
        Timeframe::M15 | Timeframe::H1 | Timeframe::D1 => Timeframe::M15,
    }
}

fn duration_from_timeframe(timeframe: Timeframe) -> Result<Duration, AppError> {
    Duration::from_std(timeframe.duration()).map_err(|source| AppError::Validation {
        message: format!("failed to convert timeframe duration for {}", timeframe),
        source: Some(Box::new(source)),
    })
}
