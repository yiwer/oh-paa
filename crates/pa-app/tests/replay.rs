use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use pa_app::replay::{ReplayDataset, load_replay_dataset, run_replay_variant_from_path};

static TEMP_DATASET_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn replay_runner_records_variant_step_outputs_and_scores() {
    let report =
        run_replay_variant_from_path("testdata/analysis_replay/sample_set.json", "baseline_a")
            .await
            .unwrap();
    let second_report =
        run_replay_variant_from_path("testdata/analysis_replay/sample_set.json", "baseline_a")
            .await
            .unwrap();

    assert_eq!(report.experiment_id, second_report.experiment_id);
    assert_eq!(report.dataset_id, "sample_set");
    assert_eq!(report.pipeline_variant, "baseline_a");
    assert_eq!(report.step_runs.len(), 12);
    assert_eq!(report.programmatic_scores["total_step_runs"].as_u64(), Some(12));
    assert_eq!(report.programmatic_scores["valid_step_runs"].as_u64(), Some(12));
    assert_eq!(
        report.programmatic_scores["schema_hit_rate"].as_f64(),
        Some(1.0)
    );
    assert_eq!(
        report.programmatic_scores["latency_coverage"].as_f64(),
        Some(1.0)
    );

    let first = &report.step_runs[0];
    assert_eq!(first.sample_id, "crypto-btc-15m-breakout");
    assert_eq!(first.step_key, "shared_pa_state_bar");
    assert_eq!(first.step_version, "v1");
    assert_eq!(first.prompt_version, "v1");
    assert_eq!(first.llm_provider, "dashscope");
    assert_eq!(first.model, "qwen-plus");
    assert!(first.schema_valid);
    assert!(first.schema_validation_error.is_none());
    assert_eq!(first.latency_ms, Some(210));
    assert!(first.input_json.is_object());
    assert!(first.output_json.is_object());
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
    assert!(report.programmatic_scores["avg_latency_ms"].as_f64().unwrap() > 0.0);
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
    assert_eq!(baseline_report.experiment_id, candidate_report.experiment_id);
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
