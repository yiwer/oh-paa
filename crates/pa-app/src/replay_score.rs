use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::replay::ReplayStepRun;

pub fn score_step_runs(step_runs: &[ReplayStepRun]) -> Map<String, Value> {
    let total_step_runs = step_runs.len() as u64;
    let valid_step_runs = step_runs.iter().filter(|run| run.schema_valid).count() as u64;
    let timed_step_runs = step_runs
        .iter()
        .filter_map(|run| run.latency_ms.map(|latency| latency as f64))
        .collect::<Vec<_>>();

    Map::from_iter([
        ("total_step_runs".to_string(), Value::from(total_step_runs)),
        ("valid_step_runs".to_string(), Value::from(valid_step_runs)),
        (
            "schema_hit_rate".to_string(),
            Value::from(ratio(valid_step_runs as usize, total_step_runs as usize)),
        ),
        (
            "avg_latency_ms".to_string(),
            Value::from(avg_latency_ms(&timed_step_runs)),
        ),
        (
            "latency_coverage".to_string(),
            Value::from(ratio(timed_step_runs.len(), step_runs.len())),
        ),
        (
            "decision_tree_completeness".to_string(),
            Value::from(required_path_score(
                step_runs,
                "shared_pa_state_bar",
                &[
                    "decision_tree_state",
                    "support_resistance_map",
                    "signal_assessment",
                ],
            )),
        ),
        (
            "key_level_completeness".to_string(),
            Value::from(required_path_score(
                step_runs,
                "shared_daily_context",
                &["key_support_levels", "key_resistance_levels"],
            )),
        ),
        (
            "signal_bar_completeness".to_string(),
            Value::from(required_path_score(
                step_runs,
                "shared_daily_context",
                &["signal_bars", "candle_pattern_map"],
            )),
        ),
        (
            "bull_bear_dual_path_completeness".to_string(),
            Value::from(required_path_score(
                step_runs,
                "shared_bar_analysis",
                &["bullish_case", "bearish_case", "two_sided_balance"],
            )),
        ),
        (
            "cross_step_consistency_rate".to_string(),
            Value::from(cross_step_consistency_rate(step_runs)),
        ),
    ])
}

fn required_path_score(
    step_runs: &[ReplayStepRun],
    step_key: &str,
    required_paths: &[&str],
) -> f64 {
    let relevant_runs = step_runs
        .iter()
        .filter(|run| run.step_key == step_key)
        .collect::<Vec<_>>();
    if relevant_runs.is_empty() || required_paths.is_empty() {
        return 0.0;
    }

    let hits = relevant_runs
        .iter()
        .flat_map(|run| {
            required_paths
                .iter()
                .map(move |path| run.schema_valid && has_required_path(&run.output_json, path))
        })
        .filter(|present| *present)
        .count();

    ratio(hits, relevant_runs.len() * required_paths.len())
}

fn cross_step_consistency_rate(step_runs: &[ReplayStepRun]) -> f64 {
    let mut runs_by_sample = BTreeMap::<&str, BTreeMap<&str, &ReplayStepRun>>::new();
    for run in step_runs {
        runs_by_sample
            .entry(run.sample_id.as_str())
            .or_default()
            .insert(run.step_key.as_str(), run);
    }

    let mut consistent_samples = 0usize;
    let eligible_samples = runs_by_sample.len();

    for sample_runs in runs_by_sample.values() {
        let Some(pa_state) = sample_runs.get("shared_pa_state_bar") else {
            continue;
        };
        let Some(shared_bar) = sample_runs.get("shared_bar_analysis") else {
            continue;
        };
        let Some(shared_daily) = sample_runs.get("shared_daily_context") else {
            continue;
        };

        let pa_state_complete = pa_state.schema_valid
            && has_required_path(&pa_state.output_json, "decision_tree_state")
            && has_required_path(&pa_state.output_json, "support_resistance_map");
        let shared_bar_complete = shared_bar.schema_valid
            && has_required_path(&shared_bar.output_json, "bullish_case")
            && has_required_path(&shared_bar.output_json, "bearish_case");
        let shared_daily_complete = shared_daily.schema_valid
            && has_required_path(&shared_daily.output_json, "decision_tree_nodes")
            && has_required_path(&shared_daily.output_json, "signal_bars");

        if pa_state_complete && shared_bar_complete && shared_daily_complete {
            consistent_samples += 1;
        }
    }

    ratio(consistent_samples, eligible_samples)
}

fn has_required_path(value: &Value, key: &str) -> bool {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .is_some_and(|field| !field.is_null())
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn avg_latency_ms(latencies: &[f64]) -> f64 {
    if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<f64>() / latencies.len() as f64
    }
}
