use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use jsonschema::Validator;
use pa_core::{AppConfig, AppError};
use pa_orchestrator::sha256_json;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{build_step_registry_from_config, replay_score, workspace_root};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReplayExecutionMode {
    #[default]
    Fixture,
    LiveHistorical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayExperimentReport {
    pub experiment_id: String,
    pub dataset_id: String,
    pub pipeline_variant: String,
    #[serde(default)]
    pub candidate_id: Option<String>,
    #[serde(default)]
    pub execution_mode: ReplayExecutionMode,
    #[serde(default)]
    pub config_source_path: Option<String>,
    pub step_runs: Vec<ReplayStepRun>,
    pub programmatic_scores: Map<String, Value>,
    #[serde(default)]
    pub summary: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayStepRun {
    pub sample_id: String,
    pub market: String,
    pub timeframe: String,
    pub step_key: String,
    pub step_version: String,
    pub prompt_version: String,
    pub llm_provider: String,
    pub model: String,
    pub input_json: Value,
    pub output_json: Value,
    pub raw_response_json: Option<Value>,
    pub schema_valid: bool,
    pub schema_validation_error: Option<String>,
    pub failure_category: Option<String>,
    pub outbound_error_message: Option<String>,
    pub latency_ms: Option<u64>,
    pub judge_score: Option<f64>,
    pub human_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayDataset {
    pub dataset_id: String,
    pub samples: Vec<ReplaySample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySample {
    pub sample_id: String,
    pub market: String,
    pub timeframe: String,
    pub shared_pa_state_input: Value,
    pub shared_bar_analysis_input: Value,
    pub shared_daily_context_input: Value,
    pub user_position_advice_input: Value,
    pub variants: BTreeMap<String, ReplayVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayVariant {
    pub steps: Vec<ReplayVariantStepFixture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayVariantStepFixture {
    pub step_key: String,
    pub step_version: String,
    pub output_json: Value,
    pub latency_ms: Option<u64>,
    pub judge_score: Option<f64>,
    pub human_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayCliArgs {
    pub mode: ReplayExecutionMode,
    pub dataset_path: String,
    pub config_path: Option<String>,
    pub variant: String,
}

pub async fn run_replay_variant_from_path(
    path: impl AsRef<Path>,
    pipeline_variant: &str,
) -> Result<ReplayExperimentReport, AppError> {
    run_fixture_replay_variant_from_path(path, pipeline_variant).await
}

pub async fn run_fixture_replay_variant_from_path(
    path: impl AsRef<Path>,
    pipeline_variant: &str,
) -> Result<ReplayExperimentReport, AppError> {
    let dataset = load_replay_dataset(path)?;
    let step_runs = execute_variant(&dataset, pipeline_variant)?;
    let programmatic_scores = score_step_runs(&step_runs);
    let summary = build_replay_summary(&step_runs);
    let execution_mode = ReplayExecutionMode::Fixture;
    let config_source_path = None;
    let experiment_id = build_experiment_id(
        &dataset.dataset_id,
        pipeline_variant,
        &step_runs,
        execution_mode.clone(),
        config_source_path.as_deref(),
    )?;

    Ok(ReplayExperimentReport {
        experiment_id,
        dataset_id: dataset.dataset_id,
        pipeline_variant: pipeline_variant.to_string(),
        candidate_id: Some(pipeline_variant.to_string()),
        execution_mode,
        config_source_path,
        step_runs,
        programmatic_scores,
        summary,
    })
}

pub fn parse_replay_cli_args<I, S>(args: I) -> Result<ReplayCliArgs, anyhow::Error>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let values = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let mode_value = match value_after(&values, "--mode") {
        Some(mode) => mode,
        None if has_flag(&values, "--mode") => return Err(anyhow!("missing --mode")),
        None => "fixture".to_string(),
    };
    let dataset_path =
        value_after(&values, "--dataset").ok_or_else(|| anyhow!("missing --dataset"))?;
    let variant = value_after(&values, "--variant").ok_or_else(|| anyhow!("missing --variant"))?;
    let config_path = value_after(&values, "--config");

    let mode = match mode_value.as_str() {
        "fixture" => ReplayExecutionMode::Fixture,
        "live" => ReplayExecutionMode::LiveHistorical,
        other => return Err(anyhow!("unsupported --mode {other}")),
    };

    if mode == ReplayExecutionMode::LiveHistorical && config_path.is_none() {
        return Err(anyhow!("--config is required when --mode live"));
    }

    Ok(ReplayCliArgs {
        mode,
        dataset_path,
        config_path,
        variant,
    })
}

pub fn load_replay_dataset(path: impl AsRef<Path>) -> Result<ReplayDataset, AppError> {
    let resolved_path = resolve_input_path(path);
    let raw = fs::read_to_string(&resolved_path).map_err(|err| AppError::Storage {
        message: format!(
            "failed to read replay dataset from {}",
            resolved_path.display()
        ),
        source: Some(Box::new(err)),
    })?;

    serde_json::from_str(&raw).map_err(|err| AppError::Validation {
        message: format!(
            "failed to deserialize replay dataset from {}",
            resolved_path.display()
        ),
        source: Some(Box::new(err)),
    })
}

fn execute_variant(
    dataset: &ReplayDataset,
    pipeline_variant: &str,
) -> Result<Vec<ReplayStepRun>, AppError> {
    let config = load_example_config()?;
    let registry = build_step_registry_from_config(&config)?;
    let mut step_runs = Vec::new();
    let expected_steps = expected_variant_steps(pipeline_variant)?;

    for sample in &dataset.samples {
        let variant = sample
            .variants
            .get(pipeline_variant)
            .ok_or_else(|| AppError::Analysis {
                message: format!(
                    "sample {} does not define pipeline variant {}",
                    sample.sample_id, pipeline_variant
                ),
                source: None,
            })?;
        validate_variant_steps(&sample.sample_id, &variant.steps, expected_steps)?;

        for step in &variant.steps {
            let resolved = registry
                .resolve(&step.step_key, &step.step_version)
                .ok_or_else(|| AppError::Analysis {
                    message: format!(
                        "missing step registration for replay step {}:{}",
                        step.step_key, step.step_version
                    ),
                    source: None,
                })?;
            let validator = validator_for_step(resolved.step.output_json_schema.clone())?;
            let schema_validation_error = validator
                .iter_errors(&step.output_json)
                .next()
                .map(|err| err.to_string());
            let schema_valid = schema_validation_error.is_none();

            step_runs.push(ReplayStepRun {
                sample_id: sample.sample_id.clone(),
                market: sample.market.clone(),
                timeframe: sample.timeframe.clone(),
                step_key: step.step_key.clone(),
                step_version: step.step_version.clone(),
                prompt_version: resolved.prompt.step_version.clone(),
                llm_provider: resolved.profile.provider.clone(),
                model: resolved.profile.model.clone(),
                input_json: sample_input_for_step(sample, &step.step_key)?,
                output_json: step.output_json.clone(),
                raw_response_json: None,
                schema_valid,
                schema_validation_error,
                failure_category: None,
                outbound_error_message: None,
                latency_ms: step.latency_ms,
                judge_score: step.judge_score,
                human_notes: step.human_notes.clone(),
            });
        }
    }

    Ok(step_runs)
}

fn sample_input_for_step(sample: &ReplaySample, step_key: &str) -> Result<Value, AppError> {
    match step_key {
        "shared_pa_state_bar" => Ok(sample.shared_pa_state_input.clone()),
        "shared_bar_analysis" => Ok(sample.shared_bar_analysis_input.clone()),
        "shared_daily_context" => Ok(sample.shared_daily_context_input.clone()),
        "user_position_advice" => Ok(sample.user_position_advice_input.clone()),
        _ => Err(AppError::Analysis {
            message: format!("unsupported replay step key {step_key}"),
            source: None,
        }),
    }
}

fn load_example_config() -> Result<AppConfig, AppError> {
    let path = workspace_root().join("config.example.toml");
    AppConfig::load_from_path(&path).map_err(|err| AppError::Validation {
        message: format!("failed to load replay config from {}", path.display()),
        source: Some(Box::new(err)),
    })
}

fn validator_for_step(schema: Value) -> Result<Validator, AppError> {
    jsonschema::validator_for(&schema).map_err(|err| AppError::Analysis {
        message: format!("invalid replay step schema: {err}"),
        source: None,
    })
}

fn resolve_input_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() || path.exists() {
        return path.to_path_buf();
    }

    workspace_root().join(path)
}

fn value_after(values: &[String], flag: &str) -> Option<String> {
    values
        .iter()
        .position(|value| value == flag)
        .and_then(|index| values.get(index + 1))
        .filter(|value| !value.starts_with("--"))
        .cloned()
}

fn has_flag(values: &[String], flag: &str) -> bool {
    values.iter().any(|value| value == flag)
}

pub(crate) fn build_experiment_id(
    dataset_id: &str,
    pipeline_variant: &str,
    step_runs: &[ReplayStepRun],
    execution_mode: ReplayExecutionMode,
    config_source_path: Option<&str>,
) -> Result<String, AppError> {
    sha256_json(&serde_json::json!({
        "dataset_id": dataset_id,
        "pipeline_variant": pipeline_variant,
        "execution_mode": execution_mode,
        "config_source_path": config_source_path,
        "step_runs": step_runs,
    }))
}

fn expected_variant_steps(
    pipeline_variant: &str,
) -> Result<&'static [(&'static str, &'static str)], AppError> {
    match pipeline_variant {
        "baseline_a" => Ok(&[
            ("shared_pa_state_bar", "v1"),
            ("shared_bar_analysis", "v2"),
            ("shared_daily_context", "v2"),
            ("user_position_advice", "v2"),
        ]),
        _ => Err(AppError::Analysis {
            message: format!("unsupported replay pipeline variant {pipeline_variant}"),
            source: None,
        }),
    }
}

fn validate_variant_steps(
    sample_id: &str,
    actual_steps: &[ReplayVariantStepFixture],
    expected_steps: &[(&str, &str)],
) -> Result<(), AppError> {
    if actual_steps.len() != expected_steps.len() {
        return Err(AppError::Analysis {
            message: format!(
                "sample {sample_id} variant fixture length mismatch: expected {} steps, got {}",
                expected_steps.len(),
                actual_steps.len()
            ),
            source: None,
        });
    }

    let actual_keys = actual_steps
        .iter()
        .map(|step| (step.step_key.as_str(), step.step_version.as_str()))
        .collect::<Vec<_>>();
    let unique_keys = actual_keys.iter().copied().collect::<BTreeSet<_>>();
    if unique_keys.len() != actual_keys.len() {
        return Err(AppError::Analysis {
            message: format!("sample {sample_id} contains duplicate replay step fixtures"),
            source: None,
        });
    }

    for (index, (expected_key, expected_version)) in expected_steps.iter().enumerate() {
        let actual = actual_keys[index];
        if actual != (*expected_key, *expected_version) {
            return Err(AppError::Analysis {
                message: format!(
                    "sample {sample_id} replay step order mismatch at index {index}: expected {}:{}, got {}:{}",
                    expected_key, expected_version, actual.0, actual.1
                ),
                source: None,
            });
        }
    }

    Ok(())
}

pub(crate) fn score_step_runs(step_runs: &[ReplayStepRun]) -> Map<String, Value> {
    replay_score::score_step_runs(step_runs)
}

pub(crate) fn build_replay_summary(step_runs: &[ReplayStepRun]) -> Map<String, Value> {
    let mut summary = Map::new();
    summary.insert(
        "total_step_runs".to_string(),
        Value::from(step_runs.len() as u64),
    );

    let first_failing_step = step_runs.iter().find(|run| {
        !run.schema_valid
            || run.failure_category.is_some()
            || run.schema_validation_error.is_some()
            || run.outbound_error_message.is_some()
    });
    let first_failing_step = match first_failing_step {
        Some(run) => serde_json::json!({
            "sample_id": run.sample_id,
            "step_key": run.step_key,
            "step_version": run.step_version,
            "failure_category": run.failure_category,
            "schema_validation_error": run.schema_validation_error,
            "outbound_error_message": run.outbound_error_message,
        }),
        None => Value::Null,
    };
    summary.insert("first_failing_step".to_string(), first_failing_step);

    let mut failure_counts_by_category: BTreeMap<String, u64> = BTreeMap::new();
    for run in step_runs {
        let category = run
            .failure_category
            .clone()
            .or_else(|| {
                if run.schema_validation_error.is_some() || !run.schema_valid {
                    Some("schema_validation_failure".to_string())
                } else if run.outbound_error_message.is_some() {
                    Some("outbound_failure".to_string())
                } else {
                    None
                }
            });
        if let Some(category) = category {
            *failure_counts_by_category.entry(category).or_insert(0) += 1;
        }
    }
    summary.insert(
        "failure_counts_by_category".to_string(),
        serde_json::to_value(failure_counts_by_category).unwrap_or(Value::Null),
    );

    summary
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ReplayExecutionMode, ReplayStepRun, build_experiment_id};

    #[test]
    fn build_experiment_id_changes_when_execution_context_changes() {
        let step_runs = vec![ReplayStepRun {
            sample_id: "sample-1".to_string(),
            market: "crypto-btc".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_pa_state_bar".to_string(),
            step_version: "v1".to_string(),
            prompt_version: "v1".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "qwen-plus".to_string(),
            input_json: json!({"input": "value"}),
            output_json: json!({"output": "value"}),
            raw_response_json: None,
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(200),
            judge_score: Some(0.9),
            human_notes: None,
        }];

        let fixture_id = build_experiment_id(
            "sample_set",
            "baseline_a",
            &step_runs,
            ReplayExecutionMode::Fixture,
            None,
        )
        .expect("fixture experiment id should hash");
        let live_id = build_experiment_id(
            "sample_set",
            "baseline_a",
            &step_runs,
            ReplayExecutionMode::LiveHistorical,
            Some("config/live.toml"),
        )
        .expect("live experiment id should hash");
        let live_with_other_config_id = build_experiment_id(
            "sample_set",
            "baseline_a",
            &step_runs,
            ReplayExecutionMode::LiveHistorical,
            Some("config/live-alt.toml"),
        )
        .expect("alternate config experiment id should hash");

        assert_ne!(fixture_id, live_id);
        assert_ne!(live_id, live_with_other_config_id);
    }
}
