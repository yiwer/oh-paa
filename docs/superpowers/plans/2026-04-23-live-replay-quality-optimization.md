# Live Replay Quality Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the tooling, observability, templates, and first prompt-hardening loop needed to run a disciplined quality-first optimization funnel for `5-sample live replay`, then execute the first candidate rounds and archive the results.

**Architecture:** Extend the existing replay report and live runner so each candidate round is observable, attributable, and archive-friendly. Add a dedicated single-step probe CLI and a secret-free current-shape config template so Stage 1 and Stage 2 screening can run without hand-built ad hoc commands. Then harden the first-step prompt contract with TDD and use the new tooling to run the first funnel rounds toward the two-run `5-sample live replay` quality gate.

**Tech Stack:** Rust 2024, Tokio, Serde, Serde JSON, Reqwest, Chrono, TOML, UUID, Axum test servers, TwelveData, OpenAI-compatible chat completions, existing `pa-app` replay modules, `pa-analysis` prompt specs, Markdown runbook docs.

---

## File Structure Map

- `G:\Rust\oh-paa\crates\pa-app\src\replay.rs`
  - Extend shared report types with candidate metadata and summary fields that fixture and live replay can both emit.
- `G:\Rust\oh-paa\crates\pa-app\src\replay_live.rs`
  - Emit progress logs and live replay summary fields, preserve first-failing-step information, and keep report generation stable for long-running background executions.
- `G:\Rust\oh-paa\crates\pa-app\src\replay_probe.rs`
  - Hold Stage 1 single-step probe models, input loading helpers, and report serialization for one-step candidate checks.
- `G:\Rust\oh-paa\crates\pa-app\src\bin\replay_probe.rs`
  - Provide a CLI entrypoint for single-step and single-sample probe execution.
- `G:\Rust\oh-paa\crates\pa-app\src\lib.rs`
  - Export the new replay probe module.
- `G:\Rust\oh-paa\crates\pa-app\tests\replay.rs`
  - Cover report summary serialization and candidate metadata in fixture replay.
- `G:\Rust\oh-paa\crates\pa-app\tests\live_replay.rs`
  - Cover live replay summary fields, first-failing-step propagation, and probe helpers.
- `G:\Rust\oh-paa\crates\pa-app\tests\probe.rs`
  - Cover CLI parsing and single-step probe output shape.
- `G:\Rust\oh-paa\testdata\analysis_replay\probe_shared_pa_state_input.json`
  - Provide a stable first-step probe input for Stage 1 screening.
- `G:\Rust\oh-paa\crates\pa-analysis\src\prompt_specs.rs`
  - Receive the first-step prompt hardening for `shared_pa_state_bar_v1`.
- `G:\Rust\oh-paa\crates\pa-analysis\tests\task_factory.rs`
  - Verify the tightened `shared_pa_state_bar_v1` instructions remain explicit and schema-aligned.
- `G:\Rust\oh-paa\config.live-replay-quality.example.toml`
  - Provide a secret-free current-shape template for step-level model bindings used during the optimization funnel.
- `G:\Rust\oh-paa\docs\architecture\phase1-runtime.md`
  - Document the stable optimization runbook commands and candidate funnel workflow.
- `G:\Rust\oh-paa\docs\superpowers\archives\`
  - Store candidate reports and findings notes produced during execution.

## Decomposition Note

This plan covers one subsystem: the live replay quality optimization loop for the existing `crypto + 15m + baseline_a` slice. The work is intentionally limited to:

- making replay rounds observable and attributable
- adding a dedicated probe path for Stage 1 and Stage 2 screening
- codifying a secret-free current-shape config template
- hardening the first-step prompt that has already shown instability during real runs
- executing the first disciplined candidate funnel rounds

It does not expand datasets, add new analysis steps, or redesign the pipeline.

### Task 1: Add Candidate Metadata, Failure Summary, and Progress Visibility to Replay Reports

**Files:**
- Modify: `G:\Rust\oh-paa\crates\pa-app\src\replay.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-app\src\replay_live.rs`
- Test: `G:\Rust\oh-paa\crates\pa-app\tests\replay.rs`
- Test: `G:\Rust\oh-paa\crates\pa-app\tests\live_replay.rs`

- [ ] **Step 1: Write the failing fixture replay test for candidate metadata and summary fields**

```rust
#[tokio::test]
async fn replay_runner_records_candidate_metadata_and_failure_summary() {
    let report = pa_app::replay::run_fixture_replay_variant_from_path(
        "testdata/analysis_replay/sample_set.json",
        "baseline_a",
    )
    .await
    .unwrap();

    assert_eq!(report.candidate_id.as_deref(), Some("baseline_a"));
    assert!(report.summary.is_object());
    assert_eq!(
        report.summary["total_step_runs"].as_u64(),
        Some(report.step_runs.len() as u64)
    );
    assert_eq!(report.summary["first_failing_step"], serde_json::Value::Null);
    assert_eq!(
        report.summary["failure_counts_by_category"],
        serde_json::json!({})
    );
}
```

- [ ] **Step 2: Run the fixture replay test to verify the new fields do not exist yet**

Run: `cargo test -p pa-app --test replay replay_runner_records_candidate_metadata_and_failure_summary -- --exact`

Expected: FAIL with unknown field errors for `candidate_id` or `summary`

- [ ] **Step 3: Add the new report fields and summary builder in `replay.rs`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayExperimentReport {
    pub experiment_id: String,
    pub dataset_id: String,
    pub pipeline_variant: String,
    #[serde(default)]
    pub execution_mode: ReplayExecutionMode,
    #[serde(default)]
    pub config_source_path: Option<String>,
    #[serde(default)]
    pub candidate_id: Option<String>,
    pub step_runs: Vec<ReplayStepRun>,
    pub programmatic_scores: serde_json::Map<String, serde_json::Value>,
    #[serde(default = "default_report_summary")]
    pub summary: serde_json::Value,
}

fn default_report_summary() -> serde_json::Value {
    serde_json::json!({
        "total_step_runs": 0,
        "first_failing_step": null,
        "failure_counts_by_category": {}
    })
}

pub(crate) fn build_report_summary(step_runs: &[ReplayStepRun]) -> serde_json::Value {
    let first_failing_step = step_runs
        .iter()
        .find(|run| !run.schema_valid)
        .map(|run| format!("{}:{}:{}", run.sample_id, run.step_key, run.step_version));

    let mut failure_counts = std::collections::BTreeMap::<String, u64>::new();
    for run in step_runs.iter().filter(|run| !run.schema_valid) {
        let key = run
            .failure_category
            .clone()
            .unwrap_or_else(|| "unknown_failure".to_string());
        *failure_counts.entry(key).or_insert(0) += 1;
    }

    serde_json::json!({
        "total_step_runs": step_runs.len(),
        "first_failing_step": first_failing_step,
        "failure_counts_by_category": failure_counts,
    })
}
```

- [ ] **Step 4: Use the summary builder from fixture and live replay constructors**

```rust
let summary = build_report_summary(&step_runs);

Ok(ReplayExperimentReport {
    experiment_id,
    dataset_id: dataset.dataset_id.clone(),
    pipeline_variant: dataset.pipeline_variant.clone(),
    execution_mode,
    config_source_path,
    candidate_id: Some(dataset.pipeline_variant.clone()),
    step_runs,
    programmatic_scores,
    summary,
})
```

- [ ] **Step 5: Add the failing live replay test for first-failing-step propagation**

```rust
#[tokio::test]
async fn live_replay_runner_records_first_failing_step_in_summary() {
    let _guard = process_test_lock();
    let dataset = single_sample_dataset();
    let resolved_config =
        load_replay_config(pa_app::workspace_root().join("config.example.toml")).unwrap();
    let server = test_server_for_values(build_twelvedata_values_json(
        utc("2026-04-18T05:15:00Z"),
        12,
    ))
    .await;
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        server.base_url(),
        "test-key",
    )));
    let executor = TestExecutor::new(vec![
        ExecutionOutcome::OutboundCallFailed {
            attempt: sample_attempt("shared_pa_state_bar", "qwen-plus"),
            error: AppError::Provider {
                message: "socket closed".to_string(),
                source: None,
            },
        },
    ]);

    let error = run_live_replay_with_dependencies(&dataset, &resolved_config, &provider_router, &executor)
        .await
        .expect_err("warmup failure should still surface");

    assert!(error.to_string().contains("shared_pa_state_bar:v1"));
}
```

- [ ] **Step 6: Add progress logs around warmup and target execution in `replay_live.rs`**

```rust
tracing::info!(
    sample_id = %sample.sample_id,
    warmup_bar_count = sample.warmup_bar_count,
    target_bar_open_time = %sample.target_bar_open_time,
    "live replay sample started"
);

tracing::info!(
    sample_id = %sample.sample_id,
    warmup_index = warmup_offset,
    step = "shared_pa_state_bar",
    "live replay warmup step started"
);

tracing::info!(
    sample_id = %sample.sample_id,
    step = step.step_key,
    latency_ms,
    schema_valid = step_run.schema_valid,
    "live replay target step finished"
);
```

- [ ] **Step 7: Run replay tests to verify report metadata and live replay logging changes are green**

Run: `cargo test -p pa-app --test replay --test live_replay`

Expected: PASS with the new report fields serialized and the live replay tests still green

- [ ] **Step 8: Commit**

```bash
git add crates/pa-app/src/replay.rs crates/pa-app/src/replay_live.rs crates/pa-app/tests/replay.rs crates/pa-app/tests/live_replay.rs
git commit -m "feat: add replay candidate summary metadata"
```

### Task 2: Add a Dedicated `replay_probe` CLI for Single-Step and Single-Sample Screening

**Files:**
- Create: `G:\Rust\oh-paa\crates\pa-app\src\replay_probe.rs`
- Create: `G:\Rust\oh-paa\crates\pa-app\src\bin\replay_probe.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-app\src\lib.rs`
- Create: `G:\Rust\oh-paa\crates\pa-app\tests\probe.rs`
- Create: `G:\Rust\oh-paa\testdata\analysis_replay\probe_shared_pa_state_input.json`

- [ ] **Step 1: Write the failing probe test for CLI parsing and result shape**

```rust
#[test]
fn replay_probe_cli_parses_step_and_input_path() {
    let args = pa_app::replay_probe::parse_probe_cli_args([
        "replay_probe",
        "--config",
        "config.live-replay-quality.example.toml",
        "--step",
        "shared_pa_state_bar:v1",
        "--input",
        "testdata/analysis_replay/probe_shared_pa_state_input.json",
    ])
    .expect("probe args should parse");

    assert_eq!(args.step_key, "shared_pa_state_bar");
    assert_eq!(args.step_version, "v1");
    assert_eq!(
        args.input_path,
        "testdata/analysis_replay/probe_shared_pa_state_input.json"
    );
}

#[tokio::test]
async fn replay_probe_records_schema_and_failure_fields() {
    let result = pa_app::replay_probe::ProbeResult {
        step_key: "shared_pa_state_bar".to_string(),
        step_version: "v1".to_string(),
        llm_provider: "dashscope".to_string(),
        model: "qwen3.6-plus".to_string(),
        schema_valid: true,
        failure_category: None,
        schema_validation_error: None,
        outbound_error_message: None,
        output_json: serde_json::json!({"bar_identity": {}}),
        raw_response_json: Some(serde_json::json!({"choices": []})),
    };

    assert_eq!(result.failure_category, None);
    assert!(result.output_json.is_object());
}
```

- [ ] **Step 2: Run the probe tests to verify the module and types do not exist yet**

Run: `cargo test -p pa-app --test probe`

Expected: FAIL because `replay_probe` does not exist

- [ ] **Step 3: Add the probe args, result type, and execution helper**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeCliArgs {
    pub config_path: String,
    pub step_key: String,
    pub step_version: String,
    pub input_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeResult {
    pub step_key: String,
    pub step_version: String,
    pub llm_provider: String,
    pub model: String,
    pub schema_valid: bool,
    pub failure_category: Option<String>,
    pub schema_validation_error: Option<String>,
    pub outbound_error_message: Option<String>,
    pub output_json: serde_json::Value,
    pub raw_response_json: Option<serde_json::Value>,
}

pub async fn run_probe_from_path(
    config_path: impl AsRef<std::path::Path>,
    step_key: &str,
    step_version: &str,
    input_path: impl AsRef<std::path::Path>,
) -> Result<ProbeResult, pa_core::AppError> {
    let resolved_config = crate::replay_config::load_replay_config(config_path)?;
    let executor = crate::build_worker_executor_from_config(&resolved_config.app_config)?;
    let input_json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(input_path.as_ref()).map_err(|source| pa_core::AppError::Storage {
            message: format!("failed to read probe input from {}", input_path.as_ref().display()),
            source: Some(Box::new(source)),
        })?,
    )
    .map_err(|source| pa_core::AppError::Validation {
        message: "failed to parse probe input JSON".to_string(),
        source: Some(Box::new(source)),
    })?;

    let outcome = executor.execute_json(step_key, step_version, &input_json).await?;
    Ok(match outcome {
        pa_orchestrator::ExecutionOutcome::Success(attempt) => ProbeResult {
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            schema_valid: true,
            failure_category: None,
            schema_validation_error: None,
            outbound_error_message: None,
            output_json: attempt.parsed_output_json.unwrap_or(serde_json::Value::Null),
            raw_response_json: attempt.raw_response_json,
        },
        pa_orchestrator::ExecutionOutcome::SchemaValidationFailed(attempt) => ProbeResult {
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            schema_valid: false,
            failure_category: Some("schema_validation".to_string()),
            schema_validation_error: attempt.schema_validation_error,
            outbound_error_message: None,
            output_json: attempt.parsed_output_json.unwrap_or(serde_json::Value::Null),
            raw_response_json: attempt.raw_response_json,
        },
        pa_orchestrator::ExecutionOutcome::OutboundCallFailed { attempt, .. } => ProbeResult {
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            schema_valid: false,
            failure_category: Some("transport".to_string()),
            schema_validation_error: None,
            outbound_error_message: attempt.outbound_error_message,
            output_json: attempt.parsed_output_json.unwrap_or(serde_json::Value::Null),
            raw_response_json: attempt.raw_response_json,
        },
    })
}
```

- [ ] **Step 4: Add the CLI entrypoint and export the module**

```rust
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = pa_app::replay_probe::parse_probe_cli_args(std::env::args())?;
    let result = pa_app::replay_probe::run_probe_from_path(
        args.config_path,
        &args.step_key,
        &args.step_version,
        args.input_path,
    )
    .await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
```

- [ ] **Step 5: Add the stable probe input fixture**

```json
{
  "instrument_id": "22222222-2222-2222-2222-222222222202",
  "timeframe": "15m",
  "bar_state": "closed",
  "bar_open_time": "2026-04-18T06:15:00Z",
  "bar_close_time": "2026-04-18T06:30:00Z",
  "bar_json": {
    "kind": "canonical_closed_bar",
    "instrument_id": "22222222-2222-2222-2222-222222222202",
    "timeframe": "15m",
    "open_time": "2026-04-18T06:15:00Z",
    "close_time": "2026-04-18T06:30:00Z",
    "open": "77201.26",
    "high": "77215.3",
    "low": "77091",
    "close": "77121.26",
    "volume": null,
    "source_provider": "twelvedata"
  },
  "market_context_json": {
    "display_symbol": "BTC/USD",
    "provider": "twelvedata",
    "provider_symbol": "BTC/USD",
    "target_bar": {
      "kind": "canonical_closed_bar",
      "instrument_id": "22222222-2222-2222-2222-222222222202",
      "timeframe": "15m",
      "open_time": "2026-04-18T06:15:00Z",
      "close_time": "2026-04-18T06:30:00Z",
      "open": "77201.26",
      "high": "77215.3",
      "low": "77091",
      "close": "77121.26",
      "volume": null,
      "source_provider": "twelvedata"
    },
    "multi_timeframe_structure": {
      "1h": [
        {
          "kind": "aggregated_closed_bar",
          "instrument_id": "22222222-2222-2222-2222-222222222202",
          "source_timeframe": "15m",
          "timeframe": "1h",
          "open_time": "2026-04-18T06:00:00Z",
          "close_time": "2026-04-18T07:00:00Z",
          "open": "77201.26",
          "high": "77215.3",
          "low": "77091",
          "close": "77121.26",
          "volume": null,
          "source_provider": "twelvedata",
          "child_bar_count": 1,
          "expected_child_bar_count": 4,
          "complete": false
        }
      ],
      "1d": [
        {
          "kind": "aggregated_closed_bar",
          "instrument_id": "22222222-2222-2222-2222-222222222202",
          "source_timeframe": "15m",
          "timeframe": "1d",
          "open_time": "2026-04-18T00:00:00Z",
          "close_time": "2026-04-19T00:00:00Z",
          "open": "77201.26",
          "high": "77215.3",
          "low": "77091",
          "close": "77121.26",
          "volume": null,
          "source_provider": "twelvedata",
          "child_bar_count": 1,
          "expected_child_bar_count": 96,
          "complete": false
        }
      ]
    }
  }
}
```

- [ ] **Step 6: Run the probe tests and a help-path smoke command**

Run: `cargo test -p pa-app --test probe`

Expected: PASS

Run: `cargo run -p pa-app --bin replay_probe -- --config config.live-replay-quality.example.toml --step shared_pa_state_bar:v1 --input testdata/analysis_replay/probe_shared_pa_state_input.json`

Expected: validation or provider error if the template config still uses replacement keys, but the CLI should parse and execute through the probe path

- [ ] **Step 7: Commit**

```bash
git add crates/pa-app/src/replay_probe.rs crates/pa-app/src/bin/replay_probe.rs crates/pa-app/src/lib.rs crates/pa-app/tests/probe.rs testdata/analysis_replay/probe_shared_pa_state_input.json
git commit -m "feat: add replay probe cli"
```

### Task 3: Add a Secret-Free Current-Shape Config Template and Stable Optimization Runbook

**Files:**
- Create: `G:\Rust\oh-paa\config.live-replay-quality.example.toml`
- Modify: `G:\Rust\oh-paa\docs\architecture\phase1-runtime.md`

- [ ] **Step 1: Add the secret-free current-shape config template**

```toml
database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "replace-with-real-key"

[llm.providers.deepseek]
base_url = "https://api.deepseek.com"
api_key = "replace-with-real-key"
openai_api_style = "chat_completions"

[llm.providers.dashscope]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "replace-with-real-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.pa_state_extract_fast]
provider = "dashscope"
model = "qwen3.6-plus"
max_tokens = 4096
max_retries = 2
per_call_timeout_secs = 180
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.execution_profiles.shared_bar_reasoner]
provider = "dashscope"
model = "qwen3-max"
max_tokens = 8192
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.execution_profiles.user_position_reasoner]
provider = "deepseek"
model = "deepseek-v3.2"
max_tokens = 4096
max_retries = 2
per_call_timeout_secs = 300
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "pa_state_extract_fast"

[llm.step_bindings.shared_bar_analysis_v2]
execution_profile = "shared_bar_reasoner"

[llm.step_bindings.shared_daily_context_v2]
execution_profile = "shared_bar_reasoner"

[llm.step_bindings.user_position_advice_v2]
execution_profile = "user_position_reasoner"
```

- [ ] **Step 2: Extend the runtime doc with exact funnel commands**

```md
## Quality Optimization Funnel

- Stage 1 single-step probe:
  - `cargo run -p pa-app --bin replay_probe -- --config <config.toml> --step shared_pa_state_bar:v1 --input <input.json>`
- Stage 2 single-sample chain:
  - `cargo run -p pa-app --bin replay_analysis -- --mode live --dataset <single-sample.json> --config <config.toml> --variant baseline_a`
- Stage 3 five-sample gate:
  - `cargo run -p pa-app --bin replay_analysis -- --mode live --dataset testdata/analysis_replay/live_crypto_15m.json --config <config.toml> --variant baseline_a`
- Archive convention:
  - `docs/superpowers/archives/<date>-<candidate>-report.json`
  - `docs/superpowers/archives/<date>-<candidate>-findings.md`
```

- [ ] **Step 3: Run a config-template parse check and docs review**

Run: `cargo test -p pa-app --test replay_config load_replay_config_keeps_current_oh_paa_shape_unchanged -- --exact`

Expected: PASS, confirming the current-shape template remains aligned with `AppConfig`

Run: `git diff -- docs/architecture/phase1-runtime.md config.live-replay-quality.example.toml`

Expected: only the new runbook section and the secret-free template

- [ ] **Step 4: Commit**

```bash
git add config.live-replay-quality.example.toml docs/architecture/phase1-runtime.md
git commit -m "docs: add live replay quality runbook template"
```

### Task 4: Harden `shared_pa_state_bar_v1` Against JSON Drift Using TDD

**Files:**
- Modify: `G:\Rust\oh-paa\crates\pa-analysis\src\prompt_specs.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-analysis\tests\task_factory.rs`

- [ ] **Step 1: Write the failing prompt-contract test for stricter JSON-only first-step instructions**

```rust
#[test]
fn shared_pa_state_prompt_v1_emphasizes_no_alias_no_trailing_text_and_object_only_shape() {
    let prompt = pa_analysis::shared_pa_state_bar_prompt_v1();
    let instructions = prompt.developer_instructions.join("\n");

    assert!(instructions.contains("Do not use alias keys"));
    assert!(instructions.contains("Do not append explanatory text after the closing JSON brace"));
    assert!(instructions.contains("All numeric-looking values must still remain inside JSON fields"));
    assert!(instructions.contains("If a section is uncertain, emit an object with uncertainty fields instead of prose"));
}
```

- [ ] **Step 2: Run the prompt-contract test to verify the stricter language is absent**

Run: `cargo test -p pa-analysis shared_pa_state_prompt_v1_emphasizes_no_alias_no_trailing_text_and_object_only_shape -- --exact`

Expected: FAIL because the new guardrail strings are not present yet

- [ ] **Step 3: Add the minimal prompt hardening in `prompt_specs.rs`**

```rust
"Do not use alias keys, near-match keys, or commentary wrappers. Use only the required schema keys exactly as written."
    .to_string(),
"Do not append explanatory text before or after the JSON object. The first character must be `{` and the final character must be `}`."
    .to_string(),
"Every required section must remain a JSON object. If confidence is low, emit an object that carries uncertainty fields instead of prose or omitted keys."
    .to_string(),
"Do not encode analysis as free text paragraphs inside a top-level field. Keep all reasoning inside structured JSON fields."
    .to_string(),
```

- [ ] **Step 4: Run the focused prompt-contract test and the existing PA-analysis regression suite**

Run: `cargo test -p pa-analysis shared_pa_state_prompt_v1_emphasizes_no_alias_no_trailing_text_and_object_only_shape -- --exact`

Expected: PASS

Run: `cargo test -p pa-analysis`

Expected: PASS with the stricter first-step prompt contract and no schema regressions

- [ ] **Step 5: Commit**

```bash
git add crates/pa-analysis/src/prompt_specs.rs crates/pa-analysis/tests/task_factory.rs
git commit -m "feat: harden shared pa state prompt contract"
```

### Task 5: Execute the First Candidate Funnel Rounds and Archive the Evidence

**Files:**
- Modify: `G:\Rust\oh-paa\docs\superpowers\archives\2026-04-23-live-replay-findings.md`
- Create: `G:\Rust\oh-paa\docs\superpowers\archives\2026-04-23-candidate-baseline-findings.md`
- Create: `G:\Rust\oh-paa\docs\superpowers\archives\2026-04-23-candidate-qwen36plus-qwenmax-v1-findings.md`

- [ ] **Step 1: Materialize a real local config outside the repo from the example template**

Run:

```powershell
Copy-Item G:\Rust\oh-paa\config.live-replay-quality.example.toml $env:TEMP\oh-paa-quality-current.toml
notepad $env:TEMP\oh-paa-quality-current.toml
```

Expected: a local current-shape config with real API keys and the first candidate step bindings

- [ ] **Step 2: Build a one-sample dataset for Stage 2 screening**

Run:

```powershell
$dataset = Get-Content G:\Rust\oh-paa\testdata\analysis_replay\live_crypto_15m.json -Raw | ConvertFrom-Json
$dataset.samples = @($dataset.samples[0])
$dataset | ConvertTo-Json -Depth 10 | Set-Content $env:TEMP\live_crypto_15m_single.json
```

Expected: `$env:TEMP\live_crypto_15m_single.json` containing only one sample

- [ ] **Step 3: Run the Stage 1 probe on the first-step candidate**

Run:

```powershell
cargo run -p pa-app --bin replay_probe -- --config $env:TEMP\oh-paa-quality-current.toml --step shared_pa_state_bar:v1 --input G:\Rust\oh-paa\testdata\analysis_replay\probe_shared_pa_state_input.json
```

Expected: JSON output with `schema_valid`, `failure_category`, `output_json`, and `raw_response_json`

- [ ] **Step 4: Run the Stage 2 single-sample chain**

Run:

```powershell
cargo run -p pa-app --bin replay_analysis -- --mode live --dataset $env:TEMP\live_crypto_15m_single.json --config $env:TEMP\oh-paa-quality-current.toml --variant baseline_a
```

Expected: a full report JSON or a first-failing-step error that can be attributed to one step

- [ ] **Step 5: Run the Stage 3 five-sample gate and archive stdout/stderr separately**

Run:

```powershell
$stdout = "G:\Rust\oh-paa\docs\superpowers\archives\2026-04-23-candidate-baseline-report.json"
$stderr = "G:\Rust\oh-paa\docs\superpowers\archives\2026-04-23-candidate-baseline-run.log"
cargo run -p pa-app --bin replay_analysis -- --mode live --dataset G:\Rust\oh-paa\testdata\analysis_replay\live_crypto_15m.json --config $env:TEMP\oh-paa-quality-current.toml --variant baseline_a 1> $stdout 2> $stderr
```

Expected: a stable JSON report in stdout and a run log with progress events in stderr

- [ ] **Step 6: Record the result in the findings note and decide the single next variable**

Use this note body:

```md
## Candidate baseline_a

- Config: `qwen3.6-plus -> qwen3-max -> qwen3-max -> deepseek-v3.2`
- Stage 1 result:
  - pass/fail:
  - failure category:
- Stage 2 result:
  - pass/fail:
  - first failing step:
- Stage 3 result:
  - schema_hit_rate:
  - cross_step_consistency_rate:
  - decision_tree_completeness:
  - key_level_completeness:
  - signal_bar_completeness:
  - bull_bear_dual_path_completeness:
- Next single variable to change:
```

- [ ] **Step 7: Commit the archive note only after redacting secrets from any pasted config fragments**

```bash
git add docs/superpowers/archives/2026-04-23-live-replay-findings.md docs/superpowers/archives/2026-04-23-candidate-baseline-findings.md
git commit -m "docs: archive first live replay quality candidate"
```

## Self-Review

- Spec coverage:
  - model funnel and candidate pools: covered by Tasks 2, 3, and 5
  - live replay observability and attributable failures: covered by Task 1
  - first-step prompt hardening: covered by Task 4
  - repeatable archive artifacts and runbook: covered by Tasks 3 and 5
  - two-run `5-sample` gate: the tooling and first-round archive path are covered here; the second confirming run happens after a candidate passes Task 5 Stage 3
- Placeholder scan:
  - no `TODO`, `TBD`, or “implement later” markers
  - every code-changing step contains concrete code
  - every run step contains exact commands
- Type consistency:
  - `candidate_id`, `summary`, `ProbeCliArgs`, and `ProbeResult` are named consistently across tasks
