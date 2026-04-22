use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use jsonschema::Validator;
use pa_core::{AppConfig, AppError};
use pa_orchestrator::sha256_json;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{build_step_registry_from_config, workspace_root};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayExperimentReport {
    pub experiment_id: String,
    pub dataset_id: String,
    pub pipeline_variant: String,
    pub step_runs: Vec<ReplayStepRun>,
    pub programmatic_scores: Map<String, Value>,
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
    pub schema_valid: bool,
    pub schema_validation_error: Option<String>,
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

pub async fn run_replay_variant_from_path(
    path: impl AsRef<Path>,
    pipeline_variant: &str,
) -> Result<ReplayExperimentReport, AppError> {
    let dataset = load_replay_dataset(path)?;
    let step_runs = execute_variant(&dataset, pipeline_variant)?;
    let programmatic_scores = score_step_runs(&step_runs);
    let experiment_id = build_experiment_id(&dataset.dataset_id, pipeline_variant, &step_runs)?;

    Ok(ReplayExperimentReport {
        experiment_id,
        dataset_id: dataset.dataset_id,
        pipeline_variant: pipeline_variant.to_string(),
        step_runs,
        programmatic_scores,
    })
}

pub fn load_replay_dataset(path: impl AsRef<Path>) -> Result<ReplayDataset, AppError> {
    let resolved_path = resolve_input_path(path);
    let raw = fs::read_to_string(&resolved_path).map_err(|err| AppError::Storage {
        message: format!("failed to read replay dataset from {}", resolved_path.display()),
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
                schema_valid,
                schema_validation_error,
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

fn build_experiment_id(
    dataset_id: &str,
    pipeline_variant: &str,
    step_runs: &[ReplayStepRun],
) -> Result<String, AppError> {
    sha256_json(&serde_json::json!({
        "dataset_id": dataset_id,
        "pipeline_variant": pipeline_variant,
        "step_runs": step_runs,
    }))
}

fn expected_variant_steps(pipeline_variant: &str) -> Result<&'static [(&'static str, &'static str)], AppError> {
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

fn score_step_runs(step_runs: &[ReplayStepRun]) -> Map<String, Value> {
    let total_step_runs = step_runs.len() as u64;
    let valid_step_runs = step_runs.iter().filter(|run| run.schema_valid).count() as u64;
    let schema_hit_rate = if total_step_runs == 0 {
        0.0
    } else {
        valid_step_runs as f64 / total_step_runs as f64
    };
    let timed_step_runs = step_runs
        .iter()
        .filter_map(|run| run.latency_ms.map(|latency| latency as f64))
        .collect::<Vec<_>>();
    let avg_latency_ms = if timed_step_runs.is_empty() {
        0.0
    } else {
        timed_step_runs.iter().sum::<f64>() / timed_step_runs.len() as f64
    };
    let latency_coverage = if total_step_runs == 0 {
        0.0
    } else {
        timed_step_runs.len() as f64 / total_step_runs as f64
    };

    Map::from_iter([
        ("total_step_runs".to_string(), Value::from(total_step_runs)),
        ("valid_step_runs".to_string(), Value::from(valid_step_runs)),
        ("schema_hit_rate".to_string(), Value::from(schema_hit_rate)),
        ("avg_latency_ms".to_string(), Value::from(avg_latency_ms)),
        ("latency_coverage".to_string(), Value::from(latency_coverage)),
    ])
}
