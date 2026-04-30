use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use pa_app::replay::{
    ReplayDataset, ReplayExecutionMode, ReplayExperimentReport, load_replay_dataset,
    parse_replay_cli_args, run_fixture_replay_variant_from_path, run_replay_variant_from_path,
};

static TEMP_DATASET_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn replay_runner_records_variant_step_outputs_and_scores() {
    let report = run_fixture_replay_variant_from_path(
        "testdata/analysis_replay/sample_set.json",
        "baseline_a",
    )
    .await
    .unwrap();
    let second_report = run_fixture_replay_variant_from_path(
        "testdata/analysis_replay/sample_set.json",
        "baseline_a",
    )
    .await
    .unwrap();
    let wrapper_report =
        run_replay_variant_from_path("testdata/analysis_replay/sample_set.json", "baseline_a")
            .await
            .unwrap();

    assert_eq!(report.experiment_id, second_report.experiment_id);
    assert_eq!(report.experiment_id, wrapper_report.experiment_id);
    assert_eq!(report.dataset_id, "sample_set");
    assert_eq!(report.pipeline_variant, "baseline_a");
    assert_eq!(report.execution_mode, ReplayExecutionMode::Fixture);
    assert_eq!(report.config_source_path, None);
    assert_eq!(report.step_runs.len(), 12);
    assert_eq!(
        report.programmatic_scores["total_step_runs"].as_u64(),
        Some(12)
    );
    assert_eq!(
        report.programmatic_scores["valid_step_runs"].as_u64(),
        Some(12)
    );
    assert_eq!(
        report.programmatic_scores["schema_hit_rate"].as_f64(),
        Some(1.0)
    );
    assert_eq!(
        report.programmatic_scores["latency_coverage"].as_f64(),
        Some(1.0)
    );
    assert!(
        report
            .programmatic_scores
            .contains_key("decision_tree_completeness")
    );
    assert!(
        report
            .programmatic_scores
            .contains_key("cross_step_consistency_rate")
    );

    let first = &report.step_runs[0];
    assert_eq!(first.sample_id, "crypto-btc-15m-breakout");
    assert_eq!(first.step_key, "shared_pa_state_bar");
    assert_eq!(first.step_version, "v1");
    assert_eq!(first.prompt_version, "v1");
    assert_eq!(first.llm_provider, "deepseek");
    assert_eq!(first.model, "deepseek-v4-flash");
    assert!(first.schema_valid);
    assert!(first.schema_validation_error.is_none());
    assert_eq!(first.latency_ms, Some(210));
    assert_eq!(first.raw_response_json, None);
    assert_eq!(first.failure_category, None);
    assert_eq!(first.outbound_error_message, None);
    assert!(first.input_json.is_object());
    assert!(first.output_json.is_object());
}

#[tokio::test]
async fn fixture_replay_report_includes_candidate_metadata_and_empty_failure_summary() {
    let report = run_fixture_replay_variant_from_path(
        "testdata/analysis_replay/sample_set.json",
        "baseline_a",
    )
    .await
    .expect("fixture replay should succeed");

    assert_eq!(report.candidate_id.as_deref(), Some("baseline_a"));
    assert_eq!(report.summary["total_step_runs"].as_u64(), Some(12));
    assert!(report.summary["first_failing_step"].is_null());
    assert_eq!(
        report.summary["failure_counts_by_category"].as_object(),
        Some(&serde_json::Map::new())
    );
}

#[tokio::test]
async fn replay_runner_rejects_missing_required_steps() {
    let mut dataset = sample_dataset();
    dataset.samples[0]
        .variants
        .get_mut("baseline_a")
        .unwrap()
        .steps
        .remove(2);
    let path = write_temp_dataset(&dataset);

    let error = run_replay_variant_from_path(&path, "baseline_a")
        .await
        .expect_err("missing required step should fail");

    let message = error.to_string();
    assert!(message.contains("fixture length mismatch"));
}

#[tokio::test]
async fn replay_runner_excludes_missing_latency_from_average_and_reports_coverage() {
    let mut dataset = sample_dataset();
    dataset.samples[0]
        .variants
        .get_mut("baseline_a")
        .unwrap()
        .steps[0]
        .latency_ms = None;
    let path = write_temp_dataset(&dataset);

    let report = run_replay_variant_from_path(&path, "baseline_a")
        .await
        .expect("dataset with partial latency should still replay");

    assert_eq!(report.step_runs[0].latency_ms, None);
    assert_eq!(
        report.programmatic_scores["latency_coverage"].as_f64(),
        Some(11.0 / 12.0)
    );
    assert!(
        report.programmatic_scores["avg_latency_ms"]
            .as_f64()
            .unwrap()
            > 0.0
    );
}

#[tokio::test]
async fn replay_runner_experiment_id_depends_only_on_selected_variant() {
    let baseline_report =
        run_replay_variant_from_path("testdata/analysis_replay/sample_set.json", "baseline_a")
            .await
            .unwrap();
    let mut dataset = sample_dataset();

    for sample in &mut dataset.samples {
        sample.variants.insert(
            "candidate_b".to_string(),
            sample.variants["baseline_a"].clone(),
        );
    }
    let path = write_temp_dataset(&dataset);
    let candidate_report = run_replay_variant_from_path(&path, "baseline_a")
        .await
        .expect("unrelated variant should not affect baseline id");

    assert_eq!(baseline_report.step_runs, candidate_report.step_runs);
    assert_eq!(
        baseline_report.experiment_id,
        candidate_report.experiment_id
    );
}

#[test]
fn replay_report_deserializes_legacy_fixture_report_without_execution_mode() {
    let legacy_report_json = serde_json::json!({
        "experiment_id": "legacy-id",
        "dataset_id": "sample_set",
        "pipeline_variant": "baseline_a",
        "step_runs": [],
        "programmatic_scores": {}
    });

    let report: ReplayExperimentReport =
        serde_json::from_value(legacy_report_json).expect("legacy report JSON should deserialize");

    assert_eq!(report.execution_mode, ReplayExecutionMode::Fixture);
    assert_eq!(report.config_source_path, None);
    assert_eq!(report.candidate_id, None);
    assert_eq!(report.summary["total_step_runs"].as_u64(), Some(0));
    assert!(report.summary["first_failing_step"].is_null());
    assert_eq!(
        report.summary["failure_counts_by_category"].as_object(),
        Some(&serde_json::Map::new())
    );
}

#[test]
fn replay_report_deserializes_legacy_non_empty_step_runs_with_derived_summary() {
    let legacy_report_json = serde_json::json!({
        "experiment_id": "legacy-id",
        "dataset_id": "sample_set",
        "pipeline_variant": "baseline_a",
        "step_runs": [
            {
                "sample_id": "sample-1",
                "market": "crypto",
                "timeframe": "15m",
                "step_key": "shared_pa_state_bar",
                "step_version": "v1",
                "prompt_version": "v1",
                "llm_provider": "fixture",
                "model": "fixture-live",
                "input_json": {},
                "output_json": {},
                "raw_response_json": null,
                "schema_valid": false,
                "schema_validation_error": "missing required field",
                "failure_category": null,
                "outbound_error_message": null,
                "latency_ms": 25,
                "judge_score": null,
                "human_notes": null
            }
        ],
        "programmatic_scores": {}
    });

    let report: ReplayExperimentReport =
        serde_json::from_value(legacy_report_json).expect("legacy report JSON should deserialize");

    assert_eq!(report.summary["total_step_runs"].as_u64(), Some(1));
    assert_eq!(
        report.summary["failure_counts_by_category"]["schema_validation_failure"].as_u64(),
        Some(1)
    );
    assert_eq!(
        report.summary["first_failing_step"]["step_key"].as_str(),
        Some("shared_pa_state_bar")
    );
    assert_eq!(
        report.summary["first_failing_step"]["failure_category"].as_str(),
        Some("schema_validation_failure")
    );
}

#[test]
fn replay_analysis_binary_emits_startup_log_to_stderr() {
    let binary = env!("CARGO_BIN_EXE_replay_analysis");
    let output = Command::new(binary)
        .args([
            "--dataset",
            "testdata/analysis_replay/sample_set.json",
            "--variant",
            "baseline_a",
        ])
        .output()
        .expect("replay_analysis binary should execute");

    assert!(
        output.status.success(),
        "expected replay_analysis success, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("replay_analysis starting"),
        "expected startup log in stderr, got: {stderr}"
    );
}

#[test]
fn replay_cli_parser_defaults_to_fixture_mode_without_config() {
    let args = parse_replay_cli_args([
        "replay_analysis",
        "--dataset",
        "testdata/analysis_replay/sample_set.json",
        "--variant",
        "baseline_a",
    ])
    .expect("fixture mode should not require --config");

    assert_eq!(args.mode, ReplayExecutionMode::Fixture);
    assert_eq!(
        args.dataset_path,
        "testdata/analysis_replay/sample_set.json"
    );
    assert_eq!(args.config_path, None);
    assert_eq!(args.variant, "baseline_a");
}

fn sample_dataset() -> ReplayDataset {
    load_replay_dataset("testdata/analysis_replay/sample_set.json").unwrap()
}

fn write_temp_dataset(dataset: &ReplayDataset) -> std::path::PathBuf {
    let sequence = TEMP_DATASET_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "pa-app-replay-{}-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        sequence
    ));
    fs::write(&path, serde_json::to_vec_pretty(dataset).unwrap()).unwrap();
    path
}
