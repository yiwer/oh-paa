use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use pa_analysis::{
    SharedBarAnalysisInput, SharedDailyContextInput, SharedPaStateBarInput, shared_bar_analysis_v2,
    shared_daily_context_v2, shared_pa_state_bar_v1,
};
use pa_core::Timeframe;
use pa_instrument::InstrumentMarketDataContext;
use pa_market::{
    AggregateCanonicalKlinesRequest, AggregatedKline, CanonicalKlineQuery, CanonicalKlineRow,
    DerivedOpenBar, MarketSessionKind, MarketSessionProfile, aggregate_canonical_klines,
    derive_open_bar, list_canonical_klines,
};
use pa_orchestrator::AnalysisBarState;
use pa_user::ManualUserAnalysisInput;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
    error::ApiError,
    router::{AppState, MarketRuntime},
};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SharedBarTaskRequest {
    pub instrument_id: Uuid,
    pub timeframe: String,
    pub bar_state: String,
    pub bar_open_time: Option<String>,
    pub bar_close_time: Option<String>,
    pub shared_pa_state_json: Option<Value>,
    pub recent_pa_states_json: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SharedPaStateTaskRequest {
    pub instrument_id: Uuid,
    pub timeframe: String,
    pub bar_state: String,
    pub bar_open_time: Option<String>,
    pub bar_close_time: Option<String>,
    pub bar_json: Option<Value>,
    pub market_context_json: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SharedDailyTaskRequest {
    pub instrument_id: Uuid,
    pub trading_date: Option<String>,
    pub recent_pa_states_json: Option<Value>,
    pub recent_shared_bar_analyses_json: Option<Value>,
    pub multi_timeframe_structure_json: Option<Value>,
    pub market_background_json: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ManualUserTaskRequest {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub timeframe: String,
    pub bar_state: String,
    pub bar_open_time: Option<String>,
    pub bar_close_time: Option<String>,
    pub trading_date: Option<String>,
    pub positions_json: Value,
    pub subscriptions_json: Value,
    pub shared_bar_analysis_json: Option<Value>,
    pub shared_daily_context_json: Option<Value>,
    pub shared_pa_state_json: Option<Value>,
}

#[derive(Debug, Clone)]
struct ResolvedBarInput {
    bar_open_time: DateTime<Utc>,
    bar_close_time: DateTime<Utc>,
    canonical_bar_json: Value,
    structure_context_json: Value,
}

pub(crate) async fn resolve_shared_bar_input(
    state: &AppState,
    request: SharedBarTaskRequest,
) -> Result<SharedBarAnalysisInput, ApiError> {
    let timeframe = parse_timeframe(&request.timeframe)?;
    let bar_state = parse_bar_state(&request.bar_state)?;

    if let Some(input) = build_shared_bar_input_from_request(&request, timeframe, bar_state)? {
        return Ok(input);
    }

    let runtime = market_runtime(state)?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(request.instrument_id)
        .await?;
    let resolved = resolve_bar_input_from_market(
        runtime,
        &context,
        timeframe,
        bar_state,
        parse_optional_timestamp(request.bar_open_time.as_deref())?,
        parse_optional_timestamp(request.bar_close_time.as_deref())?,
    )
    .await?;

    let shared_pa_state_json = match request.shared_pa_state_json {
        Some(value) => value,
        None => find_matching_shared_pa_state(
            state,
            request.instrument_id,
            timeframe,
            bar_state,
            resolved.bar_open_time,
            resolved.bar_close_time,
        )?,
    };
    let recent_pa_states_json = request.recent_pa_states_json.unwrap_or_else(|| {
        collect_recent_shared_pa_states_for_bar(
            state,
            request.instrument_id,
            timeframe,
            resolved.bar_open_time,
            resolved.bar_close_time,
            8,
        )
    });

    Ok(SharedBarAnalysisInput {
        instrument_id: request.instrument_id,
        timeframe,
        bar_open_time: resolved.bar_open_time,
        bar_close_time: resolved.bar_close_time,
        bar_state,
        shared_pa_state_json,
        recent_pa_states_json,
    })
}

pub(crate) async fn resolve_shared_pa_state_input(
    state: &AppState,
    request: SharedPaStateTaskRequest,
) -> Result<SharedPaStateBarInput, ApiError> {
    let timeframe = parse_timeframe(&request.timeframe)?;
    let bar_state = parse_bar_state(&request.bar_state)?;

    if let Some(input) = build_shared_pa_state_input_from_request(&request, timeframe, bar_state)? {
        return Ok(input);
    }

    let runtime = market_runtime(state)?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(request.instrument_id)
        .await?;
    let resolved = resolve_bar_input_from_market(
        runtime,
        &context,
        timeframe,
        bar_state,
        parse_optional_timestamp(request.bar_open_time.as_deref())?,
        parse_optional_timestamp(request.bar_close_time.as_deref())?,
    )
    .await?;

    Ok(SharedPaStateBarInput {
        instrument_id: request.instrument_id,
        timeframe,
        bar_state,
        bar_open_time: resolved.bar_open_time,
        bar_close_time: resolved.bar_close_time,
        bar_json: request.bar_json.unwrap_or(resolved.canonical_bar_json),
        market_context_json: request
            .market_context_json
            .unwrap_or(resolved.structure_context_json),
    })
}

pub(crate) async fn resolve_shared_daily_input(
    state: &AppState,
    request: SharedDailyTaskRequest,
) -> Result<SharedDailyContextInput, ApiError> {
    if let Some(input) = build_shared_daily_input_from_request(&request)? {
        return Ok(input);
    }

    let runtime = market_runtime(state)?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(request.instrument_id)
        .await?;
    let trading_date = match parse_optional_date(request.trading_date.as_deref())? {
        Some(trading_date) => trading_date,
        None => derive_latest_trading_date(runtime, &context).await?,
    };

    let recent_pa_states_json = match request.recent_pa_states_json {
        Some(value) => value,
        None => collect_recent_shared_pa_states_for_trading_date(
            state,
            request.instrument_id,
            &context.market.timezone,
            trading_date,
            8,
        )?,
    };
    let recent_shared_bar_analyses_json =
        request.recent_shared_bar_analyses_json.unwrap_or_else(|| {
            collect_recent_shared_bar_results_for_trading_date(
                state,
                request.instrument_id,
                &context.market.timezone,
                trading_date,
                8,
            )
        });
    let multi_timeframe_structure_json = request
        .multi_timeframe_structure_json
        .unwrap_or(build_multi_timeframe_context_json(runtime, &context).await?);
    let market_background_json = request
        .market_background_json
        .unwrap_or(build_market_background_json(runtime, &context).await?);

    Ok(SharedDailyContextInput {
        instrument_id: request.instrument_id,
        trading_date,
        recent_pa_states_json,
        recent_shared_bar_analyses_json,
        multi_timeframe_structure_json,
        market_background_json,
    })
}

fn missing_shared_pa_state_error(instrument_id: Uuid, trading_date: NaiveDate) -> ApiError {
    ApiError::from(pa_core::AppError::Analysis {
        message: format!(
            "missing shared pa state for instrument_id={instrument_id}, trading_date={trading_date}"
        ),
        source: None,
    })
}

pub(crate) async fn resolve_manual_user_input(
    state: &AppState,
    request: ManualUserTaskRequest,
) -> Result<ManualUserAnalysisInput, ApiError> {
    let timeframe = parse_timeframe(&request.timeframe)?;
    let bar_state = parse_bar_state(&request.bar_state)?;

    if let Some(input) = build_manual_user_input_from_request(&request, timeframe, bar_state)? {
        return Ok(input);
    }

    let runtime = market_runtime(state)?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(request.instrument_id)
        .await?;
    let resolved = resolve_bar_input_from_market(
        runtime,
        &context,
        timeframe,
        bar_state,
        parse_optional_timestamp(request.bar_open_time.as_deref())?,
        parse_optional_timestamp(request.bar_close_time.as_deref())?,
    )
    .await?;
    let trading_date = match parse_optional_date(request.trading_date.as_deref())? {
        Some(trading_date) => trading_date,
        None => trading_date_for_datetime(&context.market.timezone, resolved.bar_open_time)?,
    };
    let shared_bar_analysis_json = match request.shared_bar_analysis_json {
        Some(value) => value,
        None => find_matching_shared_bar_result(
            state,
            request.instrument_id,
            timeframe,
            bar_state,
            resolved.bar_open_time,
            resolved.bar_close_time,
        )?,
    };
    let shared_daily_context_json = match request.shared_daily_context_json {
        Some(value) => value,
        None => find_matching_shared_daily_context(state, request.instrument_id, trading_date)?,
    };
    let shared_pa_state_json = match request.shared_pa_state_json {
        Some(value) => value,
        None => find_matching_shared_pa_state(
            state,
            request.instrument_id,
            timeframe,
            bar_state,
            resolved.bar_open_time,
            resolved.bar_close_time,
        )?,
    };

    Ok(ManualUserAnalysisInput {
        user_id: request.user_id,
        instrument_id: request.instrument_id,
        timeframe,
        bar_state,
        bar_open_time: Some(resolved.bar_open_time),
        bar_close_time: Some(resolved.bar_close_time),
        trading_date: Some(trading_date),
        positions_json: request.positions_json,
        subscriptions_json: request.subscriptions_json,
        shared_bar_analysis_json,
        shared_daily_context_json,
        shared_pa_state_json,
    })
}

fn build_shared_bar_input_from_request(
    request: &SharedBarTaskRequest,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
) -> Result<Option<SharedBarAnalysisInput>, ApiError> {
    let Some(shared_pa_state_json) = request.shared_pa_state_json.clone() else {
        return Ok(None);
    };
    let Some(recent_pa_states_json) = request.recent_pa_states_json.clone() else {
        return Ok(None);
    };
    let bar_open_time =
        parse_optional_timestamp(request.bar_open_time.as_deref())?.ok_or_else(|| {
            ApiError::bad_request("bar_open_time is required when shared_pa_state_json is provided")
        })?;
    let bar_close_time =
        parse_optional_timestamp(request.bar_close_time.as_deref())?.ok_or_else(|| {
            ApiError::bad_request(
                "bar_close_time is required when shared_pa_state_json is provided",
            )
        })?;

    Ok(Some(SharedBarAnalysisInput {
        instrument_id: request.instrument_id,
        timeframe,
        bar_open_time,
        bar_close_time,
        bar_state,
        shared_pa_state_json,
        recent_pa_states_json,
    }))
}

fn build_shared_pa_state_input_from_request(
    request: &SharedPaStateTaskRequest,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
) -> Result<Option<SharedPaStateBarInput>, ApiError> {
    let Some(bar_json) = request.bar_json.clone() else {
        return Ok(None);
    };
    let Some(market_context_json) = request.market_context_json.clone() else {
        return Ok(None);
    };
    let bar_open_time =
        parse_optional_timestamp(request.bar_open_time.as_deref())?.ok_or_else(|| {
            ApiError::bad_request("bar_open_time is required when bar_json is provided")
        })?;
    let bar_close_time =
        parse_optional_timestamp(request.bar_close_time.as_deref())?.ok_or_else(|| {
            ApiError::bad_request("bar_close_time is required when bar_json is provided")
        })?;

    Ok(Some(SharedPaStateBarInput {
        instrument_id: request.instrument_id,
        timeframe,
        bar_state,
        bar_open_time,
        bar_close_time,
        bar_json,
        market_context_json,
    }))
}

fn build_shared_daily_input_from_request(
    request: &SharedDailyTaskRequest,
) -> Result<Option<SharedDailyContextInput>, ApiError> {
    let Some(recent_pa_states_json) = request.recent_pa_states_json.clone() else {
        return Ok(None);
    };
    let Some(recent_shared_bar_analyses_json) = request.recent_shared_bar_analyses_json.clone()
    else {
        return Ok(None);
    };
    let Some(multi_timeframe_structure_json) = request.multi_timeframe_structure_json.clone()
    else {
        return Ok(None);
    };
    let Some(market_background_json) = request.market_background_json.clone() else {
        return Ok(None);
    };
    let trading_date = parse_optional_date(request.trading_date.as_deref())?.ok_or_else(|| {
        ApiError::bad_request(
            "trading_date is required when shared daily input overrides are provided",
        )
    })?;

    Ok(Some(SharedDailyContextInput {
        instrument_id: request.instrument_id,
        trading_date,
        recent_pa_states_json,
        recent_shared_bar_analyses_json,
        multi_timeframe_structure_json,
        market_background_json,
    }))
}

fn build_manual_user_input_from_request(
    request: &ManualUserTaskRequest,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
) -> Result<Option<ManualUserAnalysisInput>, ApiError> {
    let Some(shared_bar_analysis_json) = request.shared_bar_analysis_json.clone() else {
        return Ok(None);
    };
    let Some(shared_daily_context_json) = request.shared_daily_context_json.clone() else {
        return Ok(None);
    };
    let shared_pa_state_json = request.shared_pa_state_json.clone().unwrap_or(Value::Null);

    Ok(Some(ManualUserAnalysisInput {
        user_id: request.user_id,
        instrument_id: request.instrument_id,
        timeframe,
        bar_state,
        bar_open_time: parse_optional_timestamp(request.bar_open_time.as_deref())?,
        bar_close_time: parse_optional_timestamp(request.bar_close_time.as_deref())?,
        trading_date: parse_optional_date(request.trading_date.as_deref())?,
        positions_json: request.positions_json.clone(),
        subscriptions_json: request.subscriptions_json.clone(),
        shared_bar_analysis_json,
        shared_daily_context_json,
        shared_pa_state_json,
    }))
}

async fn resolve_bar_input_from_market(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
    requested_bar_open_time: Option<DateTime<Utc>>,
    requested_bar_close_time: Option<DateTime<Utc>>,
) -> Result<ResolvedBarInput, ApiError> {
    let (bar_open_time, bar_close_time, canonical_bar_json) = match bar_state {
        AnalysisBarState::Closed => {
            resolve_closed_bar_json(
                runtime,
                context,
                timeframe,
                requested_bar_open_time,
                requested_bar_close_time,
            )
            .await?
        }
        AnalysisBarState::Open => {
            resolve_open_bar_json(
                runtime,
                context,
                timeframe,
                requested_bar_open_time,
                requested_bar_close_time,
            )
            .await?
        }
        AnalysisBarState::None => {
            return Err(ApiError::bad_request(
                "bar_state must be `open` or `closed` for shared/user analysis requests",
            ));
        }
    };
    let structure_context_json =
        build_structure_context_json(runtime, context, timeframe, bar_state, &canonical_bar_json)
            .await?;

    Ok(ResolvedBarInput {
        bar_open_time,
        bar_close_time,
        canonical_bar_json,
        structure_context_json,
    })
}

async fn resolve_closed_bar_json(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
    timeframe: Timeframe,
    requested_bar_open_time: Option<DateTime<Utc>>,
    requested_bar_close_time: Option<DateTime<Utc>>,
) -> Result<(DateTime<Utc>, DateTime<Utc>, Value), ApiError> {
    match timeframe {
        Timeframe::M15 => {
            let rows = list_canonical_klines(
                runtime.canonical_kline_repository.as_ref(),
                CanonicalKlineQuery {
                    instrument_id: context.instrument.id,
                    timeframe,
                    start_open_time: None,
                    end_open_time: requested_bar_open_time.or(requested_bar_close_time),
                    limit: 64,
                    descending: true,
                },
            )
            .await?;
            let row = rows
                .into_iter()
                .find(|row| {
                    requested_bar_open_time.is_none_or(|open_time| row.open_time == open_time)
                        && requested_bar_close_time
                            .is_none_or(|close_time| row.close_time == close_time)
                })
                .ok_or_else(|| {
                    ApiError::bad_request("unable to resolve closed bar from canonical data")
                })?;

            Ok((row.open_time, row.close_time, canonical_row_json(&row)))
        }
        Timeframe::H1 | Timeframe::D1 => {
            let rows = aggregate_canonical_klines(
                runtime.canonical_kline_repository.as_ref(),
                AggregateCanonicalKlinesRequest {
                    instrument_id: context.instrument.id,
                    source_timeframe: Timeframe::M15,
                    target_timeframe: timeframe,
                    start_open_time: None,
                    end_open_time: None,
                    limit: 64,
                    market_code: Some(context.market.code.clone()),
                    market_timezone: Some(context.market.timezone.clone()),
                },
            )
            .await?;
            let row = rows
                .into_iter()
                .filter(|row| row.complete)
                .find(|row| {
                    requested_bar_open_time.is_none_or(|open_time| row.open_time == open_time)
                        && requested_bar_close_time
                            .is_none_or(|close_time| row.close_time == close_time)
                })
                .ok_or_else(|| ApiError::bad_request("unable to resolve closed aggregated bar"))?;

            Ok((row.open_time, row.close_time, aggregated_row_json(&row)))
        }
    }
}

async fn resolve_open_bar_json(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
    timeframe: Timeframe,
    requested_bar_open_time: Option<DateTime<Utc>>,
    requested_bar_close_time: Option<DateTime<Utc>>,
) -> Result<(DateTime<Utc>, DateTime<Utc>, Value), ApiError> {
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
    .await?
    .ok_or_else(|| ApiError::bad_request("unable to resolve open bar because market is closed"))?;

    if requested_bar_open_time.is_some_and(|open_time| open_time != row.open_time) {
        return Err(ApiError::bad_request(format!(
            "requested open bar open_time {} does not match current open bar {}",
            requested_bar_open_time.expect("checked above").to_rfc3339(),
            row.open_time.to_rfc3339()
        )));
    }
    if requested_bar_close_time.is_some_and(|close_time| close_time != row.close_time) {
        return Err(ApiError::bad_request(format!(
            "requested open bar close_time {} does not match current open bar {}",
            requested_bar_close_time
                .expect("checked above")
                .to_rfc3339(),
            row.close_time.to_rfc3339()
        )));
    }

    Ok((row.open_time, row.close_time, derived_open_bar_json(&row)))
}

async fn build_structure_context_json(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
    canonical_bar_json: &Value,
) -> Result<Value, ApiError> {
    let profile = MarketSessionProfile::from_market(
        Some(&context.market.code),
        Some(&context.market.timezone),
    );
    let latest_tick_json = fetch_latest_tick_json(runtime, context).await.ok();
    let timeframe_map = build_multi_timeframe_context_json(runtime, context).await?;

    Ok(json!({
        "market": {
            "market_id": context.market.id,
            "market_code": context.market.code,
            "market_timezone": context.market.timezone,
            "session_kind": session_kind_label(profile.kind),
        },
        "focus_timeframe": timeframe.as_str(),
        "bar_state": bar_state.as_str(),
        "focus_bar": canonical_bar_json,
        "latest_tick": latest_tick_json,
        "multi_timeframe_context": timeframe_map,
    }))
}

async fn build_multi_timeframe_context_json(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
) -> Result<Value, ApiError> {
    Ok(json!({
        "15m": build_timeframe_structure_json(runtime, context, Timeframe::M15, 16).await?,
        "1h": build_timeframe_structure_json(runtime, context, Timeframe::H1, 16).await?,
        "1d": build_timeframe_structure_json(runtime, context, Timeframe::D1, 16).await?,
    }))
}

async fn build_timeframe_structure_json(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
    timeframe: Timeframe,
    limit: usize,
) -> Result<Value, ApiError> {
    let rows = match timeframe {
        Timeframe::M15 => list_canonical_klines(
            runtime.canonical_kline_repository.as_ref(),
            CanonicalKlineQuery {
                instrument_id: context.instrument.id,
                timeframe,
                start_open_time: None,
                end_open_time: None,
                limit,
                descending: true,
            },
        )
        .await?
        .into_iter()
        .map(|row| canonical_row_json(&row))
        .collect::<Vec<_>>(),
        Timeframe::H1 | Timeframe::D1 => aggregate_canonical_klines(
            runtime.canonical_kline_repository.as_ref(),
            AggregateCanonicalKlinesRequest {
                instrument_id: context.instrument.id,
                source_timeframe: Timeframe::M15,
                target_timeframe: timeframe,
                start_open_time: None,
                end_open_time: None,
                limit,
                market_code: Some(context.market.code.clone()),
                market_timezone: Some(context.market.timezone.clone()),
            },
        )
        .await?
        .into_iter()
        .map(|row| aggregated_row_json(&row))
        .collect::<Vec<_>>(),
    };

    let current_open_bar = build_current_open_bar_json(runtime, context, timeframe)
        .await
        .ok();

    Ok(json!({
        "timeframe": timeframe.as_str(),
        "rows": rows,
        "current_open_bar": current_open_bar,
    }))
}

async fn build_current_open_bar_json(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
    timeframe: Timeframe,
) -> Result<Value, ApiError> {
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

    row.map(|row| derived_open_bar_json(&row))
        .ok_or_else(|| ApiError::bad_request("market is closed"))
}

async fn fetch_latest_tick_json(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
) -> Result<Value, ApiError> {
    let primary_provider = context.policy.tick_primary.as_str();
    let fallback_provider = context
        .policy
        .tick_fallback
        .as_deref()
        .unwrap_or(primary_provider);
    let primary_binding = context.binding_for_provider(primary_provider)?;
    let fallback_binding = context.binding_for_provider(fallback_provider)?;
    let routed = runtime
        .provider_router
        .fetch_latest_tick_with_fallback_source(
            primary_provider,
            fallback_provider,
            &primary_binding.provider_symbol,
            &fallback_binding.provider_symbol,
        )
        .await?;

    Ok(json!({
        "provider": routed.provider_name,
        "price": routed.tick.price,
        "size": routed.tick.size,
        "tick_time": routed.tick.tick_time.to_rfc3339(),
    }))
}

async fn derive_latest_trading_date(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
) -> Result<NaiveDate, ApiError> {
    if let Some(row) =
        list_latest_canonical_row(runtime, context.instrument.id, Timeframe::M15).await?
    {
        return trading_date_for_datetime(&context.market.timezone, row.open_time);
    }

    let primary_provider = context.policy.tick_primary.as_str();
    let fallback_provider = context
        .policy
        .tick_fallback
        .as_deref()
        .unwrap_or(primary_provider);
    let primary_binding = context.binding_for_provider(primary_provider)?;
    let fallback_binding = context.binding_for_provider(fallback_provider)?;
    let open_bar = derive_open_bar(
        runtime.provider_router.as_ref(),
        runtime.canonical_kline_repository.as_ref(),
        pa_market::DeriveOpenBarRequest {
            instrument_id: context.instrument.id,
            timeframe: Timeframe::M15,
            market_code: Some(context.market.code.clone()),
            market_timezone: Some(context.market.timezone.clone()),
            primary_provider,
            fallback_provider,
            primary_provider_symbol: &primary_binding.provider_symbol,
            fallback_provider_symbol: &fallback_binding.provider_symbol,
        },
    )
    .await?;

    if let Some(open_bar) = open_bar {
        return trading_date_for_datetime(&context.market.timezone, open_bar.open_time);
    }

    trading_date_for_datetime(&context.market.timezone, Utc::now())
}

async fn list_latest_canonical_row(
    runtime: &std::sync::Arc<MarketRuntime>,
    instrument_id: Uuid,
    timeframe: Timeframe,
) -> Result<Option<CanonicalKlineRow>, ApiError> {
    let mut rows = list_canonical_klines(
        runtime.canonical_kline_repository.as_ref(),
        CanonicalKlineQuery {
            instrument_id,
            timeframe,
            start_open_time: None,
            end_open_time: None,
            limit: 1,
            descending: true,
        },
    )
    .await?;

    Ok(rows.pop())
}

fn collect_recent_shared_pa_states_for_bar(
    state: &AppState,
    instrument_id: Uuid,
    timeframe: Timeframe,
    target_bar_open_time: DateTime<Utc>,
    target_bar_close_time: DateTime<Utc>,
    limit: usize,
) -> Value {
    let rows = collect_recent_shared_pa_states_for_bar_from_results(
        state.orchestration_repository.results(),
        instrument_id,
        timeframe,
        target_bar_open_time,
        target_bar_close_time,
        limit,
    );

    Value::Array(rows.into_iter().map(pa_state_result_json).collect())
}

fn collect_recent_shared_pa_states_for_bar_from_results(
    results: Vec<pa_orchestrator::AnalysisResult>,
    instrument_id: Uuid,
    timeframe: Timeframe,
    target_bar_open_time: DateTime<Utc>,
    target_bar_close_time: DateTime<Utc>,
    limit: usize,
) -> Vec<pa_orchestrator::AnalysisResult> {
    let mut rows = results
        .into_iter()
        .filter(|result| is_shared_pa_state_result(result, instrument_id))
        .filter(|result| result.timeframe == Some(timeframe))
        .filter(|result| {
            is_result_at_or_before_target_bar(result, target_bar_open_time, target_bar_close_time)
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|result| result.created_at);
    rows.reverse();
    rows.truncate(limit);
    rows
}

fn collect_recent_shared_pa_states_for_trading_date(
    state: &AppState,
    instrument_id: Uuid,
    market_timezone: &str,
    trading_date: NaiveDate,
    limit: usize,
) -> Result<Value, ApiError> {
    let rows = collect_recent_shared_pa_states_for_trading_date_from_results(
        state.orchestration_repository.results(),
        instrument_id,
        market_timezone,
        trading_date,
        limit,
    )?;

    Ok(Value::Array(
        rows.into_iter().map(pa_state_result_json).collect(),
    ))
}

fn collect_recent_shared_pa_states_for_trading_date_from_results(
    results: Vec<pa_orchestrator::AnalysisResult>,
    instrument_id: Uuid,
    market_timezone: &str,
    trading_date: NaiveDate,
    limit: usize,
) -> Result<Vec<pa_orchestrator::AnalysisResult>, ApiError> {
    let mut rows = results
        .into_iter()
        .filter(|result| is_shared_pa_state_result(result, instrument_id))
        .filter(|result| result_matches_trading_date(result, market_timezone, trading_date))
        .collect::<Vec<_>>();
    rows.sort_by_key(|result| result.created_at);
    rows.reverse();
    rows.truncate(limit);
    if rows.is_empty() {
        return Err(missing_shared_pa_state_error(instrument_id, trading_date));
    }

    Ok(rows)
}

fn collect_recent_shared_bar_results_for_trading_date(
    state: &AppState,
    instrument_id: Uuid,
    market_timezone: &str,
    trading_date: NaiveDate,
    limit: usize,
) -> Value {
    let mut rows = state
        .orchestration_repository
        .results()
        .into_iter()
        .filter(|result| is_shared_bar_result(result, instrument_id))
        .filter(|result| result_matches_trading_date(result, market_timezone, trading_date))
        .collect::<Vec<_>>();
    rows.sort_by_key(|result| result.created_at);
    rows.reverse();
    rows.truncate(limit);

    Value::Array(rows.into_iter().map(shared_bar_result_json).collect())
}

fn is_shared_pa_state_result(
    result: &pa_orchestrator::AnalysisResult,
    instrument_id: Uuid,
) -> bool {
    result.task_type == shared_pa_state_bar_v1().task_type
        && result.instrument_id == instrument_id
        && result.prompt_version == shared_pa_state_bar_v1().step_version
}

fn is_shared_bar_result(result: &pa_orchestrator::AnalysisResult, instrument_id: Uuid) -> bool {
    result.task_type == shared_bar_analysis_v2().task_type
        && result.instrument_id == instrument_id
        && result.prompt_version == shared_bar_analysis_v2().step_version
}

fn is_result_at_or_before_target_bar(
    result: &pa_orchestrator::AnalysisResult,
    target_bar_open_time: DateTime<Utc>,
    target_bar_close_time: DateTime<Utc>,
) -> bool {
    let has_bar_time = result.bar_open_time.is_some() || result.bar_close_time.is_some();
    if !has_bar_time {
        return false;
    }

    result
        .bar_open_time
        .is_none_or(|open_time| open_time <= target_bar_open_time)
        && result
            .bar_close_time
            .is_none_or(|close_time| close_time <= target_bar_close_time)
}

fn result_matches_trading_date(
    result: &pa_orchestrator::AnalysisResult,
    market_timezone: &str,
    trading_date: NaiveDate,
) -> bool {
    result_trading_date(result, market_timezone) == Some(trading_date)
}

fn result_trading_date(
    result: &pa_orchestrator::AnalysisResult,
    market_timezone: &str,
) -> Option<NaiveDate> {
    result
        .trading_date
        .or_else(|| {
            result
                .bar_open_time
                .and_then(|value| trading_date_for_datetime(market_timezone, value).ok())
        })
        .or_else(|| {
            result
                .bar_close_time
                .and_then(|value| trading_date_for_datetime(market_timezone, value).ok())
        })
}

async fn build_market_background_json(
    runtime: &std::sync::Arc<MarketRuntime>,
    context: &InstrumentMarketDataContext,
) -> Result<Value, ApiError> {
    let profile = MarketSessionProfile::from_market(
        Some(&context.market.code),
        Some(&context.market.timezone),
    );

    Ok(json!({
        "market": {
            "market_id": context.market.id,
            "market_code": context.market.code,
            "market_timezone": context.market.timezone,
            "session_kind": session_kind_label(profile.kind),
        },
        "latest_tick": fetch_latest_tick_json(runtime, context).await.ok(),
        "open_bars": {
            "15m": build_current_open_bar_json(runtime, context, Timeframe::M15).await.ok(),
            "1h": build_current_open_bar_json(runtime, context, Timeframe::H1).await.ok(),
            "1d": build_current_open_bar_json(runtime, context, Timeframe::D1).await.ok(),
        },
    }))
}

fn find_matching_shared_bar_result(
    state: &AppState,
    instrument_id: Uuid,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
    bar_open_time: DateTime<Utc>,
    bar_close_time: DateTime<Utc>,
) -> Result<Value, ApiError> {
    latest_matching_shared_bar_result_from_results(
        state.orchestration_repository.results(),
        instrument_id,
        timeframe,
        bar_state,
        bar_open_time,
        bar_close_time,
    )
    .map(|result| result.output_json)
    .ok_or_else(|| ApiError::from(pa_core::AppError::Analysis {
        message: format!(
            "missing shared bar analysis for instrument_id={instrument_id}, timeframe={}, bar_state={}, bar_open_time={}, bar_close_time={}",
            timeframe.as_str(),
            bar_state.as_str(),
            bar_open_time.to_rfc3339(),
            bar_close_time.to_rfc3339()
        ),
        source: None,
    }))
}

fn latest_matching_shared_bar_result_from_results(
    results: Vec<pa_orchestrator::AnalysisResult>,
    instrument_id: Uuid,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
    bar_open_time: DateTime<Utc>,
    bar_close_time: DateTime<Utc>,
) -> Option<pa_orchestrator::AnalysisResult> {
    results
        .into_iter()
        .filter(|result| {
            is_shared_bar_result(result, instrument_id)
                && result.timeframe == Some(timeframe)
                && result.bar_state == bar_state
        })
        .filter(|result| match bar_state {
            AnalysisBarState::Closed => result.bar_close_time == Some(bar_close_time),
            AnalysisBarState::Open => result.bar_open_time == Some(bar_open_time),
            AnalysisBarState::None => false,
        })
        .max_by_key(|result| result.created_at)
}

fn find_matching_shared_pa_state(
    state: &AppState,
    instrument_id: Uuid,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
    bar_open_time: DateTime<Utc>,
    bar_close_time: DateTime<Utc>,
) -> Result<Value, ApiError> {
    latest_matching_shared_pa_state_from_results(
        state.orchestration_repository.results(),
        instrument_id,
        timeframe,
        bar_state,
        bar_open_time,
        bar_close_time,
    )
    .map(|result| result.output_json)
    .ok_or_else(|| ApiError::from(pa_core::AppError::Analysis {
        message: format!(
            "missing shared pa state for instrument_id={instrument_id}, timeframe={}, bar_state={}, bar_open_time={}, bar_close_time={}",
            timeframe.as_str(),
            bar_state.as_str(),
            bar_open_time.to_rfc3339(),
            bar_close_time.to_rfc3339()
        ),
        source: None,
    }))
}

fn latest_matching_shared_pa_state_from_results(
    results: Vec<pa_orchestrator::AnalysisResult>,
    instrument_id: Uuid,
    timeframe: Timeframe,
    bar_state: AnalysisBarState,
    bar_open_time: DateTime<Utc>,
    bar_close_time: DateTime<Utc>,
) -> Option<pa_orchestrator::AnalysisResult> {
    results
        .into_iter()
        .filter(|result| {
            is_shared_pa_state_result(result, instrument_id)
                && result.timeframe == Some(timeframe)
                && result.bar_state == bar_state
        })
        .filter(|result| match bar_state {
            AnalysisBarState::Closed => result.bar_close_time == Some(bar_close_time),
            AnalysisBarState::Open => result.bar_open_time == Some(bar_open_time),
            AnalysisBarState::None => false,
        })
        .max_by_key(|result| result.created_at)
}

fn find_matching_shared_daily_context(
    state: &AppState,
    instrument_id: Uuid,
    trading_date: NaiveDate,
) -> Result<Value, ApiError> {
    state
        .orchestration_repository
        .results()
        .into_iter()
        .filter(|result| {
            result.task_type == shared_daily_context_v2().task_type
                && result.instrument_id == instrument_id
                && result.prompt_version == shared_daily_context_v2().step_version
        })
        .filter(|result| result.trading_date == Some(trading_date))
        .max_by_key(|result| result.created_at)
        .map(|result| result.output_json)
        .ok_or_else(|| ApiError::from(pa_core::AppError::Analysis {
            message: format!(
                "missing shared daily market context for instrument_id={instrument_id}, trading_date={trading_date}"
            ),
            source: None,
        }))
}

fn shared_bar_result_json(result: pa_orchestrator::AnalysisResult) -> Value {
    shared_result_json(result)
}

fn pa_state_result_json(result: pa_orchestrator::AnalysisResult) -> Value {
    shared_result_json(result)
}

fn shared_result_json(result: pa_orchestrator::AnalysisResult) -> Value {
    let mut object = match result.output_json {
        Value::Object(object) => object,
        other => {
            let mut object = Map::new();
            object.insert("output".to_string(), other);
            object
        }
    };
    object.insert(
        "timeframe".to_string(),
        json!(result.timeframe.map(|timeframe| timeframe.as_str())),
    );
    object.insert("bar_state".to_string(), json!(result.bar_state.as_str()));
    object.insert(
        "bar_open_time".to_string(),
        json!(result.bar_open_time.map(|value| value.to_rfc3339())),
    );
    object.insert(
        "bar_close_time".to_string(),
        json!(result.bar_close_time.map(|value| value.to_rfc3339())),
    );
    Value::Object(object)
}

fn canonical_row_json(row: &CanonicalKlineRow) -> Value {
    json!({
        "kind": "canonical_closed_bar",
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

fn aggregated_row_json(row: &AggregatedKline) -> Value {
    json!({
        "kind": "aggregated_closed_bar",
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
    })
}

fn derived_open_bar_json(row: &DerivedOpenBar) -> Value {
    json!({
        "kind": "derived_open_bar",
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
    })
}

fn parse_timeframe(value: &str) -> Result<Timeframe, ApiError> {
    value.parse::<Timeframe>().map_err(ApiError::from)
}

fn parse_bar_state(value: &str) -> Result<AnalysisBarState, ApiError> {
    AnalysisBarState::from_db(value)
        .ok_or_else(|| ApiError::bad_request(format!("invalid bar_state: {value}")))
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

fn parse_optional_date(value: Option<&str>) -> Result<Option<NaiveDate>, ApiError> {
    value
        .map(|value| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|source| {
                ApiError::bad_request(format!("invalid date `{value}`: {source}"))
            })
        })
        .transpose()
}

fn market_runtime(state: &AppState) -> Result<&std::sync::Arc<MarketRuntime>, ApiError> {
    state
        .market_runtime
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("market runtime is not configured"))
}

fn trading_date_for_datetime(
    market_timezone: &str,
    datetime: DateTime<Utc>,
) -> Result<NaiveDate, ApiError> {
    let offset = timezone_offset(market_timezone)?;
    Ok(datetime.with_timezone(&offset).date_naive())
}

fn timezone_offset(timezone: &str) -> Result<FixedOffset, ApiError> {
    match timezone {
        "Asia/Shanghai" => FixedOffset::east_opt(8 * 60 * 60).ok_or_else(|| {
            ApiError::bad_request("failed to construct timezone offset for Asia/Shanghai")
        }),
        "UTC" | "Etc/UTC" => FixedOffset::east_opt(0)
            .ok_or_else(|| ApiError::bad_request("failed to construct timezone offset for UTC")),
        other => Err(ApiError::bad_request(format!(
            "unsupported market timezone for analysis runtime: {other}"
        ))),
    }
}

fn session_kind_label(kind: MarketSessionKind) -> &'static str {
    match kind {
        MarketSessionKind::ContinuousUtc => "continuous_utc",
        MarketSessionKind::CnA => "cn_a",
        MarketSessionKind::Fx24x5Utc => "fx_24x5_utc",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pa_core::Timeframe;
    use pa_orchestrator::AnalysisResult;
    use uuid::Uuid;

    #[test]
    fn historical_shared_bar_recent_pa_states_exclude_future_bars() {
        let instrument_id = Uuid::new_v4();
        let target_open_time = utc("2026-04-21T10:00:00Z");
        let target_close_time = utc("2026-04-21T10:15:00Z");
        let rows = collect_recent_shared_pa_states_for_bar_from_results(
            vec![
                shared_pa_state_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Closed,
                    utc("2026-04-21T09:45:00Z"),
                    utc("2026-04-21T10:00:00Z"),
                    utc("2026-04-21T10:01:00Z"),
                    "past",
                ),
                shared_pa_state_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Closed,
                    target_open_time,
                    target_close_time,
                    utc("2026-04-21T10:20:00Z"),
                    "target",
                ),
                shared_pa_state_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Closed,
                    utc("2026-04-21T10:15:00Z"),
                    utc("2026-04-21T10:30:00Z"),
                    utc("2026-04-21T10:31:00Z"),
                    "future",
                ),
            ],
            instrument_id,
            Timeframe::M15,
            target_open_time,
            target_close_time,
            8,
        );
        let tags = rows
            .into_iter()
            .filter_map(|row| row.output_json["tag"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();

        assert_eq!(tags, vec!["target".to_string(), "past".to_string()]);
    }

    #[test]
    fn daily_pa_state_selection_filters_and_rejects_wrong_day_states() {
        let instrument_id = Uuid::new_v4();
        let trading_date = NaiveDate::from_ymd_opt(2026, 4, 21).expect("valid date");
        let selected = collect_recent_shared_pa_states_for_trading_date_from_results(
            vec![
                shared_pa_state_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Closed,
                    utc("2026-04-22T00:00:00Z"),
                    utc("2026-04-22T00:15:00Z"),
                    utc("2026-04-22T00:20:00Z"),
                    "wrong-day",
                ),
                shared_pa_state_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Closed,
                    utc("2026-04-21T10:00:00Z"),
                    utc("2026-04-21T10:15:00Z"),
                    utc("2026-04-21T10:20:00Z"),
                    "same-day",
                ),
            ],
            instrument_id,
            "UTC",
            trading_date,
            8,
        )
        .expect("same-day rows should be selected");

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].output_json["tag"], "same-day");

        let rejected = collect_recent_shared_pa_states_for_trading_date_from_results(
            vec![shared_pa_state_result(
                instrument_id,
                Timeframe::M15,
                AnalysisBarState::Closed,
                utc("2026-04-22T00:00:00Z"),
                utc("2026-04-22T00:15:00Z"),
                utc("2026-04-22T00:20:00Z"),
                "wrong-day",
            )],
            instrument_id,
            "UTC",
            trading_date,
            8,
        );
        assert!(rejected.is_err());
    }

    #[test]
    fn exact_match_helpers_prefer_latest_rerun_for_open_bars() {
        let instrument_id = Uuid::new_v4();
        let bar_open_time = utc("2026-04-21T10:00:00Z");
        let bar_close_time = utc("2026-04-21T10:15:00Z");

        let latest_pa_state = latest_matching_shared_pa_state_from_results(
            vec![
                shared_pa_state_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Open,
                    bar_open_time,
                    bar_close_time,
                    utc("2026-04-21T10:16:00Z"),
                    "older-open-rerun",
                ),
                shared_pa_state_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Open,
                    bar_open_time,
                    bar_close_time,
                    utc("2026-04-21T10:18:00Z"),
                    "newest-open-rerun",
                ),
            ],
            instrument_id,
            Timeframe::M15,
            AnalysisBarState::Open,
            bar_open_time,
            bar_close_time,
        )
        .expect("latest PA-state rerun should resolve");
        assert_eq!(latest_pa_state.output_json["tag"], "newest-open-rerun");

        let latest_shared_bar = latest_matching_shared_bar_result_from_results(
            vec![
                shared_bar_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Open,
                    bar_open_time,
                    bar_close_time,
                    utc("2026-04-21T10:17:00Z"),
                    "older-shared-bar-rerun",
                ),
                shared_bar_result(
                    instrument_id,
                    Timeframe::M15,
                    AnalysisBarState::Open,
                    bar_open_time,
                    bar_close_time,
                    utc("2026-04-21T10:19:00Z"),
                    "newest-shared-bar-rerun",
                ),
            ],
            instrument_id,
            Timeframe::M15,
            AnalysisBarState::Open,
            bar_open_time,
            bar_close_time,
        )
        .expect("latest shared-bar rerun should resolve");
        assert_eq!(
            latest_shared_bar.output_json["tag"],
            "newest-shared-bar-rerun"
        );
    }

    fn shared_pa_state_result(
        instrument_id: Uuid,
        timeframe: Timeframe,
        bar_state: AnalysisBarState,
        bar_open_time: DateTime<Utc>,
        bar_close_time: DateTime<Utc>,
        created_at: DateTime<Utc>,
        tag: &str,
    ) -> AnalysisResult {
        let step = shared_pa_state_bar_v1();
        AnalysisResult {
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            task_type: step.task_type,
            instrument_id,
            user_id: None,
            timeframe: Some(timeframe),
            bar_state,
            bar_open_time: Some(bar_open_time),
            bar_close_time: Some(bar_close_time),
            trading_date: None,
            prompt_key: step.step_key,
            prompt_version: step.step_version,
            output_json: json!({ "tag": tag }),
            created_at,
        }
    }

    fn shared_bar_result(
        instrument_id: Uuid,
        timeframe: Timeframe,
        bar_state: AnalysisBarState,
        bar_open_time: DateTime<Utc>,
        bar_close_time: DateTime<Utc>,
        created_at: DateTime<Utc>,
        tag: &str,
    ) -> AnalysisResult {
        let step = shared_bar_analysis_v2();
        AnalysisResult {
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            task_type: step.task_type,
            instrument_id,
            user_id: None,
            timeframe: Some(timeframe),
            bar_state,
            bar_open_time: Some(bar_open_time),
            bar_close_time: Some(bar_close_time),
            trading_date: None,
            prompt_key: step.step_key,
            prompt_version: step.step_version,
            output_json: json!({ "tag": tag }),
            created_at,
        }
    }

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp should be RFC3339")
            .with_timezone(&Utc)
    }
}
