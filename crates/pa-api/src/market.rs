use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use pa_core::Timeframe;
use pa_market::{
    AggregateCanonicalKlinesRequest, CanonicalKlineQuery, MarketSessionKind, MarketSessionProfile,
    aggregate_canonical_klines, derive_open_bar, list_canonical_klines,
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    router::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/canonical", get(get_canonical_klines))
        .route("/aggregated", get(get_aggregated_klines))
        .route("/session-profile", get(get_session_profile))
        .route("/tick", get(get_latest_tick))
        .route("/open-bar", get(get_open_bar))
}

#[derive(Debug, Deserialize)]
struct CanonicalKlineQueryParams {
    instrument_id: Uuid,
    timeframe: String,
    start_open_time: Option<String>,
    end_open_time: Option<String>,
    limit: Option<usize>,
    descending: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AggregatedKlineQueryParams {
    instrument_id: Uuid,
    source_timeframe: String,
    target_timeframe: String,
    start_open_time: Option<String>,
    end_open_time: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct InstrumentQueryParams {
    instrument_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct OpenBarQueryParams {
    instrument_id: Uuid,
    timeframe: String,
}

async fn get_canonical_klines(
    State(state): State<AppState>,
    Query(query): Query<CanonicalKlineQueryParams>,
) -> ApiResult<Json<Value>> {
    let runtime = market_runtime(&state)?;
    let rows = list_canonical_klines(
        runtime.canonical_kline_repository.as_ref(),
        CanonicalKlineQuery {
            instrument_id: query.instrument_id,
            timeframe: query.timeframe.parse::<Timeframe>()?,
            start_open_time: parse_optional_timestamp(query.start_open_time.as_deref())?,
            end_open_time: parse_optional_timestamp(query.end_open_time.as_deref())?,
            limit: query.limit.unwrap_or(200),
            descending: query.descending.unwrap_or(false),
        },
    )
    .await?;

    Ok(Json(json!({
        "rows": rows.into_iter().map(serialize_canonical_row).collect::<Vec<_>>()
    })))
}

async fn get_aggregated_klines(
    State(state): State<AppState>,
    Query(query): Query<AggregatedKlineQueryParams>,
) -> ApiResult<Json<Value>> {
    let runtime = market_runtime(&state)?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(query.instrument_id)
        .await?;
    let rows = aggregate_canonical_klines(
        runtime.canonical_kline_repository.as_ref(),
        AggregateCanonicalKlinesRequest {
            instrument_id: query.instrument_id,
            source_timeframe: query.source_timeframe.parse::<Timeframe>()?,
            target_timeframe: query.target_timeframe.parse::<Timeframe>()?,
            start_open_time: parse_optional_timestamp(query.start_open_time.as_deref())?,
            end_open_time: parse_optional_timestamp(query.end_open_time.as_deref())?,
            limit: query.limit.unwrap_or(200),
            market_code: Some(context.market.code),
            market_timezone: Some(context.market.timezone),
        },
    )
    .await?;

    Ok(Json(json!({
        "rows": rows.into_iter().map(|row| json!({
            "instrument_id": row.instrument_id,
            "source_timeframe": row.source_timeframe.as_str(),
            "timeframe": row.timeframe.as_str(),
            "open_time": row.open_time.to_rfc3339(),
            "close_time": row.close_time.to_rfc3339(),
            "open": row.open,
            "high": row.high,
            "low": row.low,
            "close": row.close,
            "volume": row.volume,
            "child_bar_count": row.child_bar_count,
            "expected_child_bar_count": row.expected_child_bar_count,
            "complete": row.complete,
            "source_provider": row.source_provider,
        })).collect::<Vec<_>>()
    })))
}

async fn get_session_profile(
    State(state): State<AppState>,
    Query(query): Query<InstrumentQueryParams>,
) -> ApiResult<Json<Value>> {
    let runtime = market_runtime(&state)?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(query.instrument_id)
        .await?;
    let profile = MarketSessionProfile::from_market(
        Some(&context.market.code),
        Some(&context.market.timezone),
    );

    Ok(Json(json!({
        "instrument_id": context.instrument.id,
        "market_id": context.market.id,
        "market_code": context.market.code,
        "market_timezone": context.market.timezone,
        "session_kind": session_kind_label(profile.kind),
    })))
}

async fn get_latest_tick(
    State(state): State<AppState>,
    Query(query): Query<InstrumentQueryParams>,
) -> ApiResult<Json<Value>> {
    let runtime = market_runtime(&state)?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(query.instrument_id)
        .await?;
    let primary_provider = context.policy.tick_primary.as_str();
    let fallback_provider = context
        .policy
        .tick_fallback
        .as_deref()
        .unwrap_or(primary_provider);
    let primary_binding = context.binding_for_provider(primary_provider)?;
    let fallback_binding = context.binding_for_provider(fallback_provider)?;
    let profile = MarketSessionProfile::from_market(
        Some(&context.market.code),
        Some(&context.market.timezone),
    );
    let routed_tick = runtime
        .provider_router
        .fetch_latest_tick_with_fallback_source(
            primary_provider,
            fallback_provider,
            &primary_binding.provider_symbol,
            &fallback_binding.provider_symbol,
        )
        .await?;

    Ok(Json(json!({
        "instrument_id": context.instrument.id,
        "provider": routed_tick.provider_name,
        "market_open": profile.is_market_open(routed_tick.tick.tick_time, Timeframe::M15),
        "tick": {
            "price": routed_tick.tick.price,
            "size": routed_tick.tick.size,
            "tick_time": routed_tick.tick.tick_time.to_rfc3339(),
        }
    })))
}

async fn get_open_bar(
    State(state): State<AppState>,
    Query(query): Query<OpenBarQueryParams>,
) -> ApiResult<Json<Value>> {
    let runtime = market_runtime(&state)?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(query.instrument_id)
        .await?;
    let timeframe = query.timeframe.parse::<Timeframe>()?;
    let primary_provider = context.policy.tick_primary.as_str();
    let fallback_provider = context
        .policy
        .tick_fallback
        .as_deref()
        .unwrap_or(primary_provider);
    let primary_binding = context.binding_for_provider(primary_provider)?;
    let fallback_binding = context.binding_for_provider(fallback_provider)?;
    let row = derive_open_bar(
        runtime.provider_router.as_ref(),
        runtime.canonical_kline_repository.as_ref(),
        pa_market::DeriveOpenBarRequest {
            instrument_id: context.instrument.id,
            timeframe,
            market_code: Some(context.market.code.clone()),
            market_timezone: Some(context.market.timezone.clone()),
            primary_provider,
            fallback_provider,
            primary_provider_symbol: &primary_binding.provider_symbol,
            fallback_provider_symbol: &fallback_binding.provider_symbol,
        },
    )
    .await?;

    Ok(Json(json!({
        "instrument_id": context.instrument.id,
        "timeframe": timeframe.as_str(),
        "market_open": row.is_some(),
        "row": row.map(|row| json!({
            "instrument_id": row.instrument_id,
            "source_timeframe": row.source_timeframe.as_str(),
            "timeframe": row.timeframe.as_str(),
            "open_time": row.open_time.to_rfc3339(),
            "close_time": row.close_time.to_rfc3339(),
            "latest_tick_time": row.latest_tick_time.to_rfc3339(),
            "open": row.open,
            "high": row.high,
            "low": row.low,
            "close": row.close,
            "child_bar_count": row.child_bar_count,
            "source_provider": row.source_provider,
        }))
    })))
}

fn market_runtime(
    state: &AppState,
) -> Result<&std::sync::Arc<crate::router::MarketRuntime>, ApiError> {
    state
        .market_runtime
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("market runtime is not configured"))
}

fn parse_optional_timestamp(value: Option<&str>) -> Result<Option<DateTime<Utc>>, ApiError> {
    value
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|source| {
                    ApiError::bad_request(format!("invalid RFC3339 timestamp `{value}`: {source}"))
                })
        })
        .transpose()
}

fn serialize_canonical_row(row: pa_market::CanonicalKlineRow) -> Value {
    json!({
        "instrument_id": row.instrument_id,
        "timeframe": row.timeframe.as_str(),
        "open_time": row.open_time.to_rfc3339(),
        "close_time": row.close_time.to_rfc3339(),
        "open": row.open,
        "high": row.high,
        "low": row.low,
        "close": row.close,
        "volume": row.volume,
        "source_provider": row.source_provider,
    })
}

fn session_kind_label(kind: MarketSessionKind) -> &'static str {
    match kind {
        MarketSessionKind::ContinuousUtc => "continuous_utc",
        MarketSessionKind::CnA => "cn_a",
        MarketSessionKind::Fx24x5Utc => "fx_24x5_utc",
    }
}
