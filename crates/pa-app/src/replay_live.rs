use std::{
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::Instant,
};

use pa_analysis::{SharedBarAnalysisInput, SharedDailyContextInput, SharedPaStateBarInput};
use pa_core::{AppError, Timeframe};
use pa_market::{
    AggregatedKline, CanonicalKlineRow, HistoricalKlineQuery, ProviderRouter,
    aggregate_replay_window_rows, normalize_kline, provider::providers::TwelveDataProvider,
};
use pa_orchestrator::{AnalysisBarState, ExecutionOutcome, Executor, LlmClient};
use pa_user::ManualUserAnalysisInput;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::types::{
    Uuid,
    chrono::{DateTime, Utc},
};
use tracing::info;

use crate::{
    build_step_registry_from_config, build_worker_executor_from_config,
    replay::{
        ReplayExecutionMode, ReplayExperimentReport, ReplayStepRun, build_experiment_id,
        build_replay_summary,
        score_step_runs,
    },
    replay_config::{ResolvedReplayConfig, load_replay_config},
    workspace_root,
};

const REQUIRED_MARKET: &str = "crypto";
const REQUIRED_TIMEFRAME: &str = "15m";
const REQUIRED_PIPELINE_VARIANT: &str = "baseline_a";
const MIN_WARMUP_BAR_COUNT: usize = 8;
const REPLAY_USER_ID: Uuid = Uuid::from_u128(0);
const TARGET_STEPS: [ReplayStepSpec; 4] = [
    ReplayStepSpec {
        step_key: "shared_pa_state_bar",
        step_version: "v1",
    },
    ReplayStepSpec {
        step_key: "shared_bar_analysis",
        step_version: "v2",
    },
    ReplayStepSpec {
        step_key: "shared_daily_context",
        step_version: "v2",
    },
    ReplayStepSpec {
        step_key: "user_position_advice",
        step_version: "v2",
    },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveReplayDataset {
    pub dataset_id: String,
    pub market: String,
    pub timeframe: String,
    pub pipeline_variant: String,
    pub samples: Vec<LiveReplaySample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveReplaySample {
    pub sample_id: String,
    pub instrument_id: Uuid,
    pub provider: String,
    pub provider_symbol: String,
    pub display_symbol: String,
    pub target_bar_open_time: DateTime<Utc>,
    pub target_bar_close_time: DateTime<Utc>,
    pub lookback_15m_bar_count: usize,
    pub warmup_bar_count: usize,
    pub user_position_json: Value,
    pub user_subscription_json: Value,
}

#[derive(Debug, Clone, Copy)]
struct ReplayStepSpec {
    step_key: &'static str,
    step_version: &'static str,
}

pub trait LiveReplayExecutor: Send + Sync {
    fn execute_json<'a>(
        &'a self,
        step_key: &'a str,
        step_version: &'a str,
        input_json: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionOutcome, AppError>> + Send + 'a>>;
}

impl<C> LiveReplayExecutor for Executor<C>
where
    C: LlmClient + Send + Sync,
{
    fn execute_json<'a>(
        &'a self,
        step_key: &'a str,
        step_version: &'a str,
        input_json: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionOutcome, AppError>> + Send + 'a>> {
        Box::pin(self.execute_json(step_key, step_version, input_json))
    }
}

pub async fn run_live_historical_replay_from_path(
    dataset_path: impl AsRef<Path>,
    config_path: impl AsRef<Path>,
    pipeline_variant: &str,
) -> Result<ReplayExperimentReport, AppError> {
    let dataset = load_live_replay_dataset(dataset_path)?;
    if dataset.pipeline_variant != pipeline_variant {
        return Err(AppError::Validation {
            message: format!(
                "live replay dataset pipeline_variant {} does not match requested variant {pipeline_variant}",
                dataset.pipeline_variant
            ),
            source: None,
        });
    }

    let resolved_config = load_replay_config(resolve_input_path(config_path))?;
    let executor = build_worker_executor_from_config(&resolved_config.app_config)?;
    let provider_router = build_live_provider_router(&resolved_config);

    run_live_replay_with_dependencies(&dataset, &resolved_config, &provider_router, &executor).await
}

pub async fn run_live_replay_with_dependencies<E>(
    dataset: &LiveReplayDataset,
    resolved_config: &ResolvedReplayConfig,
    provider_router: &ProviderRouter,
    executor: &E,
) -> Result<ReplayExperimentReport, AppError>
where
    E: LiveReplayExecutor,
{
    validate_dataset(dataset)?;

    let step_registry = build_step_registry_from_config(&resolved_config.app_config)?;
    let mut step_runs = Vec::new();

    for sample in &dataset.samples {
        info!(
            sample_id = %sample.sample_id,
            provider = %sample.provider,
            symbol = %sample.provider_symbol,
            target_close_time = %sample.target_bar_close_time.to_rfc3339(),
            "live replay sample start"
        );
        let fetched_rows = fetch_sample_rows(sample, provider_router).await?;
        let sample_runs =
            run_sample_target_chain(sample, dataset, &fetched_rows, &step_registry, executor)
                .await?;
        step_runs.extend(sample_runs);
    }

    let programmatic_scores = score_step_runs(&step_runs);
    let summary = build_replay_summary(&step_runs);
    let execution_mode = ReplayExecutionMode::LiveHistorical;
    let config_source_path = Some(resolved_config.source_path.display().to_string());
    let experiment_id = build_experiment_id(
        &dataset.dataset_id,
        &dataset.pipeline_variant,
        &step_runs,
        execution_mode.clone(),
        config_source_path.as_deref(),
    )?;

    Ok(ReplayExperimentReport {
        experiment_id,
        dataset_id: dataset.dataset_id.clone(),
        pipeline_variant: dataset.pipeline_variant.clone(),
        candidate_id: Some(dataset.pipeline_variant.clone()),
        execution_mode,
        config_source_path,
        step_runs,
        programmatic_scores,
        summary,
    })
}

pub fn load_live_replay_dataset(path: impl AsRef<Path>) -> Result<LiveReplayDataset, AppError> {
    let resolved_path = resolve_input_path(path);
    let raw = fs::read_to_string(&resolved_path).map_err(|err| AppError::Storage {
        message: format!(
            "failed to read live replay dataset from {}",
            resolved_path.display()
        ),
        source: Some(Box::new(err)),
    })?;

    let dataset: LiveReplayDataset =
        serde_json::from_str(&raw).map_err(|err| AppError::Validation {
            message: format!(
                "failed to deserialize live replay dataset from {}",
                resolved_path.display()
            ),
            source: Some(Box::new(err)),
        })?;

    validate_dataset(&dataset)?;

    Ok(dataset)
}

async fn run_sample_target_chain<E>(
    sample: &LiveReplaySample,
    dataset: &LiveReplayDataset,
    rows: &[CanonicalKlineRow],
    step_registry: &pa_orchestrator::StepRegistry,
    executor: &E,
) -> Result<Vec<ReplayStepRun>, AppError>
where
    E: LiveReplayExecutor,
{
    let target_index = rows
        .iter()
        .position(|row| {
            row.open_time == sample.target_bar_open_time
                && row.close_time == sample.target_bar_close_time
        })
        .ok_or_else(|| AppError::Validation {
            message: format!(
                "live replay sample {} missing target closed bar {} -> {} in fetched window",
                sample.sample_id,
                sample.target_bar_open_time.to_rfc3339(),
                sample.target_bar_close_time.to_rfc3339()
            ),
            source: None,
        })?;
    let target_row = rows[target_index].clone();
    let warmup_start = target_index.saturating_sub(sample.warmup_bar_count);
    let warmup_rows = &rows[warmup_start..target_index];
    let target_rows = &rows[..=target_index];
    let mut warmup_pa_states = Vec::new();
    let mut warmup_shared_bar_analyses = Vec::new();

    for (warmup_offset, warmup_row) in warmup_rows.iter().enumerate() {
        let row_index = warmup_start + warmup_offset;
        let visible_rows = &rows[..=row_index];
        let pa_state_input = build_shared_pa_state_input(sample, warmup_row, visible_rows)?;
        let pa_state_output = execute_warmup_step(
            executor,
            &TARGET_STEPS[0],
            &pa_state_input,
            &sample.sample_id,
        )
        .await?;
        let shared_bar_input = build_shared_bar_analysis_input(
            sample,
            warmup_row,
            &pa_state_output,
            &warmup_pa_states,
        )?;
        let shared_bar_output = execute_warmup_step(
            executor,
            &TARGET_STEPS[1],
            &shared_bar_input,
            &sample.sample_id,
        )
        .await?;

        warmup_pa_states.push(pa_state_output);
        warmup_shared_bar_analyses.push(shared_bar_output);
    }

    let mut target_step_runs = Vec::new();
    let target_pa_state_input = build_shared_pa_state_input(sample, &target_row, target_rows)?;
    let (target_pa_state_run, target_pa_state_output) = execute_reported_step(
        executor,
        &TARGET_STEPS[0],
        sample,
        dataset,
        step_registry,
        &target_pa_state_input,
    )
    .await?;
    target_step_runs.push(target_pa_state_run);
    let Some(target_pa_state_output) = target_pa_state_output else {
        return Ok(target_step_runs);
    };

    let target_shared_bar_input = build_shared_bar_analysis_input(
        sample,
        &target_row,
        &target_pa_state_output,
        &warmup_pa_states,
    )?;
    let (target_shared_bar_run, target_shared_bar_output) = execute_reported_step(
        executor,
        &TARGET_STEPS[1],
        sample,
        dataset,
        step_registry,
        &target_shared_bar_input,
    )
    .await?;
    target_step_runs.push(target_shared_bar_run);
    let Some(target_shared_bar_output) = target_shared_bar_output else {
        return Ok(target_step_runs);
    };

    let target_daily_input = build_shared_daily_context_input(
        sample,
        &target_row,
        target_rows,
        &warmup_pa_states,
        &target_pa_state_output,
        &warmup_shared_bar_analyses,
        &target_shared_bar_output,
    )?;
    let (target_daily_run, target_daily_output) = execute_reported_step(
        executor,
        &TARGET_STEPS[2],
        sample,
        dataset,
        step_registry,
        &target_daily_input,
    )
    .await?;
    target_step_runs.push(target_daily_run);
    let Some(target_daily_output) = target_daily_output else {
        return Ok(target_step_runs);
    };

    let target_user_input = build_user_position_input(
        sample,
        &target_row,
        &target_pa_state_output,
        &target_shared_bar_output,
        &target_daily_output,
    )?;
    let (target_user_run, _) = execute_reported_step(
        executor,
        &TARGET_STEPS[3],
        sample,
        dataset,
        step_registry,
        &target_user_input,
    )
    .await?;
    target_step_runs.push(target_user_run);

    Ok(target_step_runs)
}

async fn execute_warmup_step<E>(
    executor: &E,
    step: &ReplayStepSpec,
    input_json: &Value,
    sample_id: &str,
) -> Result<Value, AppError>
where
    E: LiveReplayExecutor,
{
    info!(
        sample_id = %sample_id,
        step_key = step.step_key,
        step_version = step.step_version,
        "live replay warmup step start"
    );
    let started_at = Instant::now();
    let outcome = executor
        .execute_json(step.step_key, step.step_version, input_json)
        .await?;
    let _latency_ms = started_at.elapsed().as_millis() as u64;

    match outcome {
        ExecutionOutcome::Success(attempt) => attempt.parsed_output_json.ok_or_else(|| {
            AppError::Analysis {
                message: format!(
                    "warmup step {}:{} for sample {} returned success without parsed_output_json",
                    step.step_key, step.step_version, sample_id
                ),
                source: None,
            }
        }),
        ExecutionOutcome::SchemaValidationFailed(attempt) => Err(AppError::Analysis {
            message: format!(
                "warmup step {}:{} for sample {} failed schema validation: {}; parsed_output_preview={}",
                step.step_key,
                step.step_version,
                sample_id,
                attempt
                    .schema_validation_error
                    .unwrap_or_else(|| "unknown schema validation error".to_string()),
                json_preview(attempt.parsed_output_json.as_ref())
            ),
            source: None,
        }),
        ExecutionOutcome::OutboundCallFailed { attempt, error } => Err(AppError::Analysis {
            message: format!(
                "warmup step {}:{} for sample {} failed outbound call: {}; raw_response_preview={}",
                step.step_key,
                step.step_version,
                sample_id,
                attempt
                    .outbound_error_message
                    .unwrap_or_else(|| error.to_string()),
                json_preview(attempt.raw_response_json.as_ref())
            ),
            source: Some(Box::new(error)),
        }),
    }
}

async fn execute_reported_step<E>(
    executor: &E,
    step: &ReplayStepSpec,
    sample: &LiveReplaySample,
    dataset: &LiveReplayDataset,
    step_registry: &pa_orchestrator::StepRegistry,
    input_json: &Value,
) -> Result<(ReplayStepRun, Option<Value>), AppError>
where
    E: LiveReplayExecutor,
{
    let resolved_step = step_registry
        .resolve(step.step_key, step.step_version)
        .ok_or_else(|| AppError::Analysis {
            message: format!(
                "missing step registration for live replay step {}:{}",
                step.step_key, step.step_version
            ),
            source: None,
        })?;
    let started_at = Instant::now();
    let outcome = executor
        .execute_json(step.step_key, step.step_version, input_json)
        .await?;
    let latency_ms = started_at.elapsed().as_millis() as u64;

    let step_run = replay_step_run_from_outcome(
        sample,
        dataset,
        step,
        resolved_step.prompt.step_version.as_str(),
        input_json,
        latency_ms,
        outcome,
    );
    let continue_output = if step_run.schema_valid {
        Some(step_run.output_json.clone())
    } else {
        None
    };
    info!(
        sample_id = %sample.sample_id,
        step_key = step.step_key,
        step_version = step.step_version,
        schema_valid = step_run.schema_valid,
        failure_category = ?step_run.failure_category,
        "live replay target step finish"
    );

    Ok((step_run, continue_output))
}

fn replay_step_run_from_outcome(
    sample: &LiveReplaySample,
    dataset: &LiveReplayDataset,
    step: &ReplayStepSpec,
    prompt_version: &str,
    input_json: &Value,
    latency_ms: u64,
    outcome: ExecutionOutcome,
) -> ReplayStepRun {
    match outcome {
        ExecutionOutcome::Success(attempt) => ReplayStepRun {
            sample_id: sample.sample_id.clone(),
            market: dataset.market.clone(),
            timeframe: dataset.timeframe.clone(),
            step_key: step.step_key.to_string(),
            step_version: step.step_version.to_string(),
            prompt_version: prompt_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            input_json: input_json.clone(),
            output_json: attempt.parsed_output_json.unwrap_or(Value::Null),
            raw_response_json: attempt.raw_response_json,
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(latency_ms),
            judge_score: None,
            human_notes: None,
        },
        ExecutionOutcome::SchemaValidationFailed(attempt) => ReplayStepRun {
            sample_id: sample.sample_id.clone(),
            market: dataset.market.clone(),
            timeframe: dataset.timeframe.clone(),
            step_key: step.step_key.to_string(),
            step_version: step.step_version.to_string(),
            prompt_version: prompt_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            input_json: input_json.clone(),
            output_json: attempt.parsed_output_json.unwrap_or(Value::Null),
            raw_response_json: attempt.raw_response_json,
            schema_valid: false,
            schema_validation_error: attempt.schema_validation_error,
            failure_category: Some("schema_validation_failure".to_string()),
            outbound_error_message: None,
            latency_ms: Some(latency_ms),
            judge_score: None,
            human_notes: None,
        },
        ExecutionOutcome::OutboundCallFailed { attempt, .. } => ReplayStepRun {
            sample_id: sample.sample_id.clone(),
            market: dataset.market.clone(),
            timeframe: dataset.timeframe.clone(),
            step_key: step.step_key.to_string(),
            step_version: step.step_version.to_string(),
            prompt_version: prompt_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            input_json: input_json.clone(),
            output_json: attempt.parsed_output_json.unwrap_or(Value::Null),
            raw_response_json: attempt.raw_response_json,
            schema_valid: false,
            schema_validation_error: None,
            failure_category: Some("outbound_failure".to_string()),
            outbound_error_message: attempt.outbound_error_message,
            latency_ms: Some(latency_ms),
            judge_score: None,
            human_notes: None,
        },
    }
}

async fn fetch_sample_rows(
    sample: &LiveReplaySample,
    provider_router: &ProviderRouter,
) -> Result<Vec<CanonicalKlineRow>, AppError> {
    let start_open_time = DateTime::<Utc>::from_timestamp(
        sample.target_bar_open_time.timestamp()
            - ((sample.lookback_15m_bar_count.saturating_sub(1) as i64) * 900),
        0,
    )
    .ok_or_else(|| AppError::Validation {
        message: format!(
            "failed to compute lookback start time for live replay sample {}",
            sample.sample_id
        ),
        source: None,
    })?;
    let provider_rows = provider_router
        .fetch_klines_window_from(
            &sample.provider,
            HistoricalKlineQuery {
                provider_symbol: sample.provider_symbol.clone(),
                timeframe: Timeframe::M15,
                start_open_time: Some(start_open_time),
                end_close_time: Some(sample.target_bar_close_time),
                limit: Some(sample.lookback_15m_bar_count),
            },
        )
        .await?;
    let mut rows = provider_rows
        .into_iter()
        .map(normalize_kline)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|row| CanonicalKlineRow {
            instrument_id: sample.instrument_id,
            timeframe: Timeframe::M15,
            open_time: row.open_time,
            close_time: row.close_time,
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume,
            source_provider: sample.provider.clone(),
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.open_time);

    if rows.len() != sample.lookback_15m_bar_count {
        return Err(AppError::Validation {
            message: format!(
                "live replay sample {} fetched {} rows, but lookback_15m_bar_count expected {}",
                sample.sample_id,
                rows.len(),
                sample.lookback_15m_bar_count
            ),
            source: None,
        });
    }

    if rows.len() < sample.warmup_bar_count + 1 {
        return Err(AppError::Validation {
            message: format!(
                "live replay sample {} fetched {} rows, but needs at least {} rows for warmup + target",
                sample.sample_id,
                rows.len(),
                sample.warmup_bar_count + 1
            ),
            source: None,
        });
    }

    Ok(rows)
}

fn build_shared_pa_state_input(
    sample: &LiveReplaySample,
    row: &CanonicalKlineRow,
    visible_rows: &[CanonicalKlineRow],
) -> Result<Value, AppError> {
    to_json_value(SharedPaStateBarInput {
        instrument_id: sample.instrument_id,
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Closed,
        bar_open_time: row.open_time,
        bar_close_time: row.close_time,
        bar_json: canonical_row_json(row),
        market_context_json: build_market_context_json(sample, row, visible_rows)?,
    })
}

fn build_shared_bar_analysis_input(
    sample: &LiveReplaySample,
    row: &CanonicalKlineRow,
    shared_pa_state_json: &Value,
    historical_pa_states: &[Value],
) -> Result<Value, AppError> {
    to_json_value(SharedBarAnalysisInput {
        instrument_id: sample.instrument_id,
        timeframe: Timeframe::M15,
        bar_open_time: row.open_time,
        bar_close_time: row.close_time,
        bar_state: AnalysisBarState::Closed,
        shared_pa_state_json: shared_pa_state_json.clone(),
        recent_pa_states_json: Value::Array(recent_context_with_target(
            historical_pa_states,
            shared_pa_state_json,
        )),
    })
}

fn build_shared_daily_context_input(
    sample: &LiveReplaySample,
    target_row: &CanonicalKlineRow,
    visible_rows: &[CanonicalKlineRow],
    warmup_pa_states: &[Value],
    target_pa_state_output: &Value,
    warmup_shared_bar_analyses: &[Value],
    target_shared_bar_output: &Value,
) -> Result<Value, AppError> {
    to_json_value(SharedDailyContextInput {
        instrument_id: sample.instrument_id,
        trading_date: target_row.open_time.date_naive(),
        recent_pa_states_json: Value::Array(recent_context_with_target(
            warmup_pa_states,
            target_pa_state_output,
        )),
        recent_shared_bar_analyses_json: Value::Array(recent_context_with_target(
            warmup_shared_bar_analyses,
            target_shared_bar_output,
        )),
        multi_timeframe_structure_json: build_multi_timeframe_structure_json(sample, visible_rows)?,
        market_background_json: json!({
            "market": REQUIRED_MARKET,
            "display_symbol": sample.display_symbol,
            "provider": sample.provider,
            "provider_symbol": sample.provider_symbol,
            "target_bar": canonical_row_json(target_row),
            "lookback_15m_bar_count": sample.lookback_15m_bar_count,
        }),
    })
}

fn build_user_position_input(
    sample: &LiveReplaySample,
    target_row: &CanonicalKlineRow,
    target_pa_state_output: &Value,
    target_shared_bar_output: &Value,
    target_daily_output: &Value,
) -> Result<Value, AppError> {
    to_json_value(ManualUserAnalysisInput {
        user_id: REPLAY_USER_ID,
        instrument_id: sample.instrument_id,
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Closed,
        bar_open_time: Some(target_row.open_time),
        bar_close_time: Some(target_row.close_time),
        trading_date: Some(target_row.open_time.date_naive()),
        positions_json: sample.user_position_json.clone(),
        subscriptions_json: sample.user_subscription_json.clone(),
        shared_bar_analysis_json: target_shared_bar_output.clone(),
        shared_daily_context_json: target_daily_output.clone(),
        shared_pa_state_json: target_pa_state_output.clone(),
    })
}

fn build_market_context_json(
    sample: &LiveReplaySample,
    row: &CanonicalKlineRow,
    visible_rows: &[CanonicalKlineRow],
) -> Result<Value, AppError> {
    Ok(json!({
        "display_symbol": sample.display_symbol,
        "provider": sample.provider,
        "provider_symbol": sample.provider_symbol,
        "target_bar": canonical_row_json(row),
        "multi_timeframe_structure": build_multi_timeframe_structure_json(sample, visible_rows)?,
    }))
}

fn build_multi_timeframe_structure_json(
    sample: &LiveReplaySample,
    visible_rows: &[CanonicalKlineRow],
) -> Result<Value, AppError> {
    let one_hour = aggregate_replay_window_rows(
        visible_rows,
        sample.instrument_id,
        Timeframe::M15,
        Timeframe::H1,
        None,
        None,
    )?;
    let one_day = aggregate_replay_window_rows(
        visible_rows,
        sample.instrument_id,
        Timeframe::M15,
        Timeframe::D1,
        None,
        None,
    )?;

    Ok(json!({
        "1h": aggregated_rows_json(&one_hour),
        "1d": aggregated_rows_json(&one_day),
    }))
}

fn aggregated_rows_json(rows: &[AggregatedKline]) -> Vec<Value> {
    rows.iter().map(aggregated_row_json).collect()
}

fn recent_context_values(values: &[Value]) -> Vec<Value> {
    values.to_vec()
}

fn recent_context_with_target(values: &[Value], target: &Value) -> Vec<Value> {
    let mut recent_values = recent_context_values(values);
    recent_values.push(target.clone());
    recent_values
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

fn to_json_value<T>(value: T) -> Result<Value, AppError>
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|err| AppError::Analysis {
        message: "failed to serialize live replay step input".to_string(),
        source: Some(Box::new(err)),
    })
}

fn json_preview(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "null".to_string();
    };

    let serialized =
        serde_json::to_string(value).unwrap_or_else(|_| "<unserializable-json>".to_string());
    const MAX_CHARS: usize = 1_200;
    if serialized.chars().count() <= MAX_CHARS {
        serialized
    } else {
        let truncated = serialized.chars().take(MAX_CHARS).collect::<String>();
        format!("{truncated}...")
    }
}

fn validate_dataset(dataset: &LiveReplayDataset) -> Result<(), AppError> {
    if dataset.market != REQUIRED_MARKET {
        return Err(validation_error(format!(
            "live replay dataset market must be {REQUIRED_MARKET}, got {}",
            dataset.market
        )));
    }

    if dataset.timeframe != REQUIRED_TIMEFRAME {
        return Err(validation_error(format!(
            "live replay dataset timeframe must be {REQUIRED_TIMEFRAME}, got {}",
            dataset.timeframe
        )));
    }

    if dataset.pipeline_variant != REQUIRED_PIPELINE_VARIANT {
        return Err(validation_error(format!(
            "live replay dataset pipeline_variant must be {REQUIRED_PIPELINE_VARIANT}, got {}",
            dataset.pipeline_variant
        )));
    }

    if dataset.samples.is_empty() {
        return Err(validation_error(
            "live replay dataset samples must be non-empty",
        ));
    }

    let mut sample_ids = std::collections::BTreeSet::new();
    let mut identity_target_bars = std::collections::BTreeSet::new();
    let mut previous_target_bar_open_time: Option<DateTime<Utc>> = None;

    for sample in &dataset.samples {
        if !sample_ids.insert(sample.sample_id.clone()) {
            return Err(validation_error(format!(
                "live replay dataset contains duplicate sample_id {}",
                sample.sample_id
            )));
        }

        if sample.target_bar_close_time <= sample.target_bar_open_time {
            return Err(validation_error(format!(
                "live replay sample {} target_bar_close_time must be after target_bar_open_time",
                sample.sample_id
            )));
        }

        if let Some(previous_open_time) = previous_target_bar_open_time {
            if sample.target_bar_open_time <= previous_open_time {
                return Err(validation_error(format!(
                    "live replay dataset target bar order must be strictly increasing; sample {} is out of order",
                    sample.sample_id
                )));
            }
        }
        previous_target_bar_open_time = Some(sample.target_bar_open_time);

        let identity_target_bar_key = (
            sample.instrument_id,
            sample.provider.clone(),
            sample.provider_symbol.clone(),
            sample.display_symbol.clone(),
            sample.target_bar_open_time,
            sample.target_bar_close_time,
        );
        if !identity_target_bars.insert(identity_target_bar_key) {
            return Err(validation_error(format!(
                "live replay dataset contains duplicate target bar for sample identity at {}",
                sample.target_bar_close_time
            )));
        }

        if sample.warmup_bar_count < MIN_WARMUP_BAR_COUNT {
            return Err(validation_error(format!(
                "live replay sample {} warmup_bar_count must be at least {MIN_WARMUP_BAR_COUNT}, got {}",
                sample.sample_id, sample.warmup_bar_count
            )));
        }

        if sample.lookback_15m_bar_count < sample.warmup_bar_count + 1 {
            return Err(validation_error(format!(
                "live replay sample {} lookback_15m_bar_count must be at least warmup_bar_count + 1 ({}), got {}",
                sample.sample_id,
                sample.warmup_bar_count + 1,
                sample.lookback_15m_bar_count
            )));
        }

        if !sample.user_position_json.is_object() {
            return Err(validation_error(format!(
                "live replay sample {} user_position_json must be a JSON object",
                sample.sample_id
            )));
        }

        if !sample.user_subscription_json.is_object() {
            return Err(validation_error(format!(
                "live replay sample {} user_subscription_json must be a JSON object",
                sample.sample_id
            )));
        }
    }

    Ok(())
}

fn build_live_provider_router(resolved_config: &ResolvedReplayConfig) -> ProviderRouter {
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        &resolved_config.app_config.twelvedata_base_url,
        &resolved_config.app_config.twelvedata_api_key,
    )));
    provider_router
}

fn validation_error(message: impl Into<String>) -> AppError {
    AppError::Validation {
        message: message.into(),
        source: None,
    }
}

fn resolve_input_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let candidate = if path.is_absolute() || path.exists() {
        path.to_path_buf()
    } else {
        workspace_root().join(path)
    };

    stable_canonical_path(candidate)
}

fn stable_canonical_path(path: PathBuf) -> PathBuf {
    match fs::canonicalize(&path) {
        Ok(canonical) => strip_windows_verbatim_prefix(canonical),
        Err(_) if path.is_absolute() => path,
        Err(_) => std::env::current_dir()
            .map(|cwd| cwd.join(&path))
            .unwrap_or(path),
    }
}

fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};

        let mut components = path.components();
        let Some(Component::Prefix(prefix_component)) = components.next() else {
            return path;
        };

        let normalized_prefix = match prefix_component.kind() {
            Prefix::VerbatimDisk(disk) => {
                let drive = char::from(disk).to_string();
                format!("{drive}:")
            }
            Prefix::VerbatimUNC(server, share) => {
                format!(
                    r"\\{}\{}",
                    server.to_string_lossy(),
                    share.to_string_lossy()
                )
            }
            _ => return path,
        };

        let mut normalized = PathBuf::from(normalized_prefix);
        normalized.push(components.as_path());
        normalized
    }

    #[cfg(not(windows))]
    {
        path
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, MutexGuard},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{resolve_input_path, workspace_root};

    static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn resolve_input_path_canonicalizes_workspace_relative_file_across_cwds() {
        let _guard = process_test_lock();
        let target = workspace_root().join("config.example.toml");
        let expected = resolve_input_path(&target);

        let workspace_cwd = CurrentDirGuard::change_to(&workspace_root());
        let from_workspace = resolve_input_path("config.example.toml");
        drop(workspace_cwd);

        let isolated_dir = write_temp_dir();
        let isolated_cwd = CurrentDirGuard::change_to(&isolated_dir);
        let from_other_cwd = resolve_input_path("config.example.toml");
        drop(isolated_cwd);

        assert_eq!(from_workspace, expected);
        assert_eq!(from_other_cwd, expected);
    }

    #[test]
    fn resolve_input_path_keeps_explicit_existing_relative_paths_outside_workspace() {
        let _guard = process_test_lock();
        let external_dir = write_temp_dir();
        let external_file = external_dir.join("external-config.toml");
        fs::write(&external_file, "database_url = 'postgres://example'").unwrap();

        let cwd_guard = CurrentDirGuard::change_to(&external_dir);
        let resolved = resolve_input_path("external-config.toml");
        drop(cwd_guard);

        let expected = resolve_input_path(&external_file);
        assert_eq!(resolved, expected);
    }

    fn process_test_lock() -> MutexGuard<'static, ()> {
        match PROCESS_TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn write_temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pa-app-replay-live-unit-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl CurrentDirGuard {
        fn change_to(path: &Path) -> Self {
            let previous = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { previous }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.previous).unwrap();
        }
    }
}
