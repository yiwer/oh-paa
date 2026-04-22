# Live Historical Replay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first-class `live historical replay` lane for `crypto + 15m + baseline_a` that fetches real TwelveData history, runs the actual four-step LLM chain, and emits comparable reports for prompt-quality iteration.

**Architecture:** Keep the existing fixture replay path intact and make replay mode explicit. Extend the market SPI with a historical-window fetch contract plus reusable in-memory aggregation so the live runner can build target and warmup context without depending on database persistence. Normalize external legacy configs into the current step-oriented `AppConfig` shape, then reuse `build_step_registry_from_config` and `Executor<OpenAiCompatibleClient>` for real execution.

**Tech Stack:** Rust 2024, Tokio, Reqwest, Serde, Serde JSON, Chrono, TOML, UUID, `jsonschema`, OpenAI-compatible LLM execution, TwelveData, `pa-market` provider SPI.

---

## File Structure Map

- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\src\provider.rs`
  - Add `HistoricalKlineQuery` and provider/router entrypoints for explicit historical-window fetches.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\src\providers\twelvedata.rs`
  - Implement TwelveData `time_series` historical-window fetch with explicit UTC ordering and bounds.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\src\service.rs`
  - Expose pure aggregation helpers so replay can derive `1h` and `1d` structure from fetched `15m` bars in memory.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\src\lib.rs`
  - Export the new query and aggregation helpers.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\tests\historical_window.rs`
  - Cover TwelveData window-contract behavior and replay aggregation behavior.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\Cargo.toml`
  - Add direct dependencies needed by replay config loading and live runner modules.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay.rs`
  - Keep the public replay API, explicit mode dispatch, shared report types, and fixture compatibility.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay_config.rs`
  - Load and normalize external current-shape and legacy-shape configs into replay-ready runtime config.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay_live.rs`
  - Define live dataset models, build historical context windows, run warmup steps, and execute the target chain.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay_score.rs`
  - Compute structural, completeness, and cross-step consistency scores.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\lib.rs`
  - Export the new replay config and live replay entrypoints.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\bin\replay_analysis.rs`
  - Parse `--mode`, `--dataset`, `--config`, and `--variant`, then print JSON reports.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\tests\replay.rs`
  - Preserve fixture replay behavior while asserting new report fields.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\tests\replay_config.rs`
  - Verify current and legacy config normalization.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\tests\live_replay.rs`
  - Cover non-network live replay assembly and scoring with fixture dependencies.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\testdata\analysis_replay\live_crypto_15m.json`
  - Curated first-slice live dataset with closed-bar targets and synthetic user fixtures.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-analysis\src\prompt_specs.rs`
  - Receive the first evidence-driven prompt tightening after live replay is available.
- `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\docs\architecture\phase1-runtime.md`
  - Document the live replay operator flow, dataset contract, and first prompt-iteration loop.

## Decomposition Note

This is one subsystem plan: real historical replay for the existing analysis pipeline. It spans provider access, replay orchestration, runtime config normalization, scoring, and one evidence-driven prompt iteration, but every task serves the same closed loop:

`historical provider window -> replay context builder -> shared/user step execution -> scoring/report -> prompt improvement`

The plan intentionally stays narrow on:

- market: `crypto`
- timeframe: `15m`
- pipeline variant: `baseline_a`
- target bar state: `closed`

That keeps the first real-execution loop small enough to verify end to end before expanding to A-shares, forex, open bars, or model-grid comparisons.

### Task 1: Add Historical Window Provider Support and Replay Aggregation Primitives

**Files:**
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\src\provider.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\src\providers\twelvedata.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\src\service.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\src\lib.rs`
- Create: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-market\tests\historical_window.rs`

- [ ] **Step 1: Write the failing provider-window and aggregation tests**

```rust
use chrono::{DateTime, Utc};
use pa_core::Timeframe;
use pa_market::{
    AggregateCanonicalKlinesRequest, HistoricalKlineQuery, MarketDataProvider,
    provider::providers::TwelveDataProvider,
};

async fn spawn_twelvedata_server(
    state: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
) -> TestServer {
    use axum::{
        Router,
        extract::{Query, State},
        routing::get,
    };

    async fn time_series(
        State(state): State<std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>>,
        Query(query): Query<std::collections::HashMap<String, String>>,
    ) -> axum::Json<serde_json::Value> {
        let mut pairs = query.into_iter().collect::<Vec<_>>();
        pairs.sort_by(|left, right| left.0.cmp(&right.0));
        *state.lock().unwrap() = pairs;

        axum::Json(serde_json::json!({
            "status": "ok",
            "values": [{
                "datetime": "2026-04-20 00:00:00",
                "open": "84000.0",
                "high": "84100.0",
                "low": "83980.0",
                "close": "84080.0",
                "volume": "12.5"
            }]
        }))
    }

    let app = Router::new()
        .route("/time_series", get(time_series))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    TestServer {
        base_url: format!("http://{address}"),
    }
}

fn fixture_canonical_rows_15m(start: &str, count: usize) -> Vec<pa_market::CanonicalKlineRow> {
    let start = DateTime::parse_from_rfc3339(start).unwrap().with_timezone(&Utc);

    (0..count)
        .map(|index| {
            let open_time = start + chrono::Duration::minutes((index as i64) * 15);
            let close_time = open_time + chrono::Duration::minutes(15);
            let base = rust_decimal::Decimal::from(84_000 + (index as i64 * 10));

            pa_market::CanonicalKlineRow {
                instrument_id: uuid::Uuid::nil(),
                timeframe: Timeframe::M15,
                open_time,
                close_time,
                open: base,
                high: base + rust_decimal::Decimal::from(25),
                low: base - rust_decimal::Decimal::from(20),
                close: base + rust_decimal::Decimal::from(10),
                volume: Some(rust_decimal::Decimal::from(100 + index as i64)),
                source_provider: "twelvedata".to_string(),
            }
        })
        .collect()
}

#[derive(Debug)]
struct TestServer {
    base_url: String,
}

impl TestServer {
    fn base_url(&self) -> String {
        self.base_url.clone()
    }
}

#[tokio::test]
async fn twelvedata_fetch_klines_window_uses_explicit_bounds_and_ascending_order() {
    let state = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(String, String)>::new()));
    let server = spawn_twelvedata_server(std::sync::Arc::clone(&state)).await;
    let provider = TwelveDataProvider::new(server.base_url(), "test-key");

    let rows = provider
        .fetch_klines_window(HistoricalKlineQuery {
            provider_symbol: "BTC/USD".to_string(),
            timeframe: Timeframe::M15,
            start_open_time: Some(utc("2026-04-20T00:00:00Z")),
            end_close_time: Some(utc("2026-04-20T06:00:00Z")),
            limit: Some(32),
        })
        .await
        .expect("window request should succeed");

    assert!(!rows.is_empty());
    let query = state.lock().unwrap().clone();
    assert!(query.contains(&("symbol".into(), "BTC/USD".into())));
    assert!(query.contains(&("interval".into(), "15min".into())));
    assert!(query.contains(&("start_date".into(), "2026-04-20T00:00:00+00:00".into())));
    assert!(query.contains(&("end_date".into(), "2026-04-20T06:00:00+00:00".into())));
    assert!(query.contains(&("order".into(), "asc".into())));
    assert!(query.contains(&("timezone".into(), "UTC".into())));
}

#[test]
fn aggregate_replay_rows_builds_complete_hourly_bars_in_memory() {
    let rows = fixture_canonical_rows_15m("2026-04-20T00:00:00Z", 8);
    let aggregated = pa_market::aggregate_replay_rows(
        &rows,
        rows[0].instrument_id,
        Timeframe::M15,
        Timeframe::H1,
        Some("crypto"),
        Some("UTC"),
    )
    .expect("aggregation should succeed");

    assert_eq!(aggregated.len(), 2);
    assert!(aggregated.iter().all(|row| row.complete));
    assert_eq!(aggregated[0].child_bar_count, 4);
}

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value).unwrap().with_timezone(&Utc)
}
```

- [ ] **Step 2: Run the new market tests to verify the window contract is missing**

Run: `cargo test -p pa-market --test historical_window`

Expected: FAIL because `HistoricalKlineQuery`, `fetch_klines_window`, and `aggregate_replay_rows` do not exist yet

- [ ] **Step 3: Add `HistoricalKlineQuery` and router/provider entrypoints with a safe default implementation**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalKlineQuery {
    pub provider_symbol: String,
    pub timeframe: Timeframe,
    pub start_open_time: Option<DateTime<Utc>>,
    pub end_close_time: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn fetch_klines(
        &self,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError>;

    async fn fetch_klines_window(
        &self,
        query: HistoricalKlineQuery,
    ) -> Result<Vec<ProviderKline>, AppError> {
        match (query.start_open_time, query.end_close_time, query.limit) {
            (None, None, Some(limit)) => {
                self.fetch_klines(&query.provider_symbol, query.timeframe, limit)
                    .await
            }
            _ => Err(AppError::Validation {
                message: format!(
                    "provider `{}` does not support historical window fetch",
                    self.name()
                ),
                source: None,
            }),
        }
    }
}
```

- [ ] **Step 4: Implement TwelveData historical-window fetch and expose the router helper**

```rust
async fn fetch_klines_window(
    &self,
    query: HistoricalKlineQuery,
) -> Result<Vec<ProviderKline>, AppError> {
    let mut params = vec![
        ("symbol", query.provider_symbol),
        ("interval", Self::timeframe_interval(query.timeframe).to_string()),
        ("order", "asc".to_string()),
        ("timezone", "UTC".to_string()),
        ("apikey", self.api_key.clone()),
    ];

    if let Some(limit) = query.limit {
        params.push(("outputsize", limit.to_string()));
    }
    if let Some(start) = query.start_open_time {
        params.push(("start_date", start.to_rfc3339()));
    }
    if let Some(end) = query.end_close_time {
        params.push(("end_date", end.to_rfc3339()));
    }

    let body = self.get_text("time_series", &params).await?;
    Self::parse_klines_response(&body, query.timeframe)
}

pub async fn fetch_klines_window_from(
    &self,
    provider_name: &str,
    query: HistoricalKlineQuery,
) -> Result<Vec<ProviderKline>, AppError> {
    let provider = self.provider(provider_name).ok_or_else(|| AppError::Validation {
        message: format!("provider `{provider_name}` is not registered"),
        source: None,
    })?;
    provider.fetch_klines_window(query).await
}
```

- [ ] **Step 5: Expose a pure in-memory aggregation helper for replay windows**

```rust
pub fn aggregate_replay_rows(
    rows: &[CanonicalKlineRow],
    instrument_id: Uuid,
    source_timeframe: Timeframe,
    target_timeframe: Timeframe,
    market_code: Option<&str>,
    market_timezone: Option<&str>,
) -> Result<Vec<AggregatedKline>, AppError> {
    let session_profile = MarketSessionProfile::from_market(market_code, market_timezone);
    let mut accepted = rows
        .iter()
        .filter(|row| session_profile.accepts_bar_open(source_timeframe, row.open_time))
        .cloned()
        .collect::<Vec<_>>();
    accepted.sort_by_key(|row| row.open_time);

    aggregate_rows(
        &accepted,
        instrument_id,
        source_timeframe,
        target_timeframe,
        &session_profile,
    )
}
```

- [ ] **Step 6: Run the market suite covering the new replay primitives**

Run: `cargo test -p pa-market --test historical_window`

Expected: PASS with explicit query-param assertions for TwelveData and complete in-memory aggregation coverage for replay windows

- [ ] **Step 7: Commit the provider-window slice**

```bash
git add crates/pa-market/src/provider.rs crates/pa-market/src/providers/twelvedata.rs crates/pa-market/src/service.rs crates/pa-market/src/lib.rs crates/pa-market/tests/historical_window.rs
git commit -m "feat: add historical window provider support for replay"
```

### Task 2: Refactor Replay Contracts Around Explicit Execution Modes

**Files:**
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\lib.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\tests\replay.rs`

- [ ] **Step 1: Write the failing replay test for explicit execution-mode metadata**

```rust
#[tokio::test]
async fn fixture_replay_report_marks_execution_mode_and_keeps_fixture_metadata() {
    let report = pa_app::replay::run_fixture_replay_variant_from_path(
        "testdata/analysis_replay/sample_set.json",
        "baseline_a",
    )
    .await
    .unwrap();

    assert_eq!(report.execution_mode, pa_app::replay::ReplayExecutionMode::Fixture);
    assert!(report.config_source_path.is_none());
    assert!(report.step_runs.iter().all(|run| run.raw_response_json.is_none()));
}
```

- [ ] **Step 2: Run the replay test to verify the new mode contract is missing**

Run: `cargo test -p pa-app fixture_replay_report_marks_execution_mode_and_keeps_fixture_metadata -- --exact`

Expected: FAIL because `ReplayExecutionMode`, `run_fixture_replay_variant_from_path`, and the new report fields do not exist yet

- [ ] **Step 3: Extend the shared replay report and step-run models**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayExecutionMode {
    Fixture,
    LiveHistorical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayExperimentReport {
    pub experiment_id: String,
    pub dataset_id: String,
    pub pipeline_variant: String,
    pub execution_mode: ReplayExecutionMode,
    pub config_source_path: Option<String>,
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
    pub raw_response_json: Option<Value>,
    pub schema_valid: bool,
    pub schema_validation_error: Option<String>,
    pub failure_category: Option<String>,
    pub outbound_error_message: Option<String>,
    pub latency_ms: Option<u64>,
    pub judge_score: Option<f64>,
    pub human_notes: Option<String>,
}
```

- [ ] **Step 4: Split the public entrypoints into fixture and live dispatch**

```rust
pub async fn run_fixture_replay_variant_from_path(
    path: impl AsRef<Path>,
    pipeline_variant: &str,
) -> Result<ReplayExperimentReport, AppError> {
    let dataset = load_replay_dataset(path)?;
    let step_runs = execute_fixture_variant(&dataset, pipeline_variant)?;
    Ok(ReplayExperimentReport {
        experiment_id: build_experiment_id(&dataset.dataset_id, pipeline_variant, &step_runs)?,
        dataset_id: dataset.dataset_id,
        pipeline_variant: pipeline_variant.to_string(),
        execution_mode: ReplayExecutionMode::Fixture,
        config_source_path: None,
        step_runs,
        programmatic_scores: score_step_runs(&step_runs),
    })
}

pub async fn run_replay_variant_from_path(
    path: impl AsRef<Path>,
    pipeline_variant: &str,
) -> Result<ReplayExperimentReport, AppError> {
    run_fixture_replay_variant_from_path(path, pipeline_variant).await
}
```

- [ ] **Step 5: Run the replay tests to confirm fixture behavior stays stable**

Run: `cargo test -p pa-app --test replay`

Expected: PASS with old fixture assertions still green and the new `execution_mode` metadata present

- [ ] **Step 6: Commit the replay-contract refactor**

```bash
git add crates/pa-app/src/replay.rs crates/pa-app/src/lib.rs crates/pa-app/tests/replay.rs
git commit -m "refactor: add explicit replay execution modes"
```

### Task 3: Add External Replay Config Normalization for Current and Legacy Shapes

**Files:**
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\Cargo.toml`
- Create: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay_config.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\lib.rs`
- Create: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\tests\replay_config.rs`

- [ ] **Step 1: Write the failing config-normalization tests for both known legacy sources**

```rust
fn write_temp_toml(raw: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "replay-config-{}-{}.toml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, raw).unwrap();
    path
}

#[test]
fn load_replay_config_normalizes_pa_analyze_server_shape() {
    let path = write_temp_toml(r#"
[providers.twelvedata]
base_url = "https://api.twelvedata.com"
api_key = "demo-key"

[llm.qwen]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "dashscope-demo"
model = "deepseek-v3.2"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
"#);

    let resolved = pa_app::replay_config::load_replay_config(&path).unwrap();

    assert_eq!(resolved.app_config.twelvedata_api_key, "demo-key");
    assert_eq!(
        resolved.app_config.llm.execution_profiles["baseline_a_default"].model,
        "deepseek-v3.2"
    );
    assert_eq!(
        resolved.app_config.llm.step_bindings["shared_pa_state_bar_v1"].execution_profile,
        "baseline_a_default"
    );
}

#[test]
fn load_replay_config_normalizes_stock_everyday_shape() {
    let path = write_temp_toml(r#"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "demo-key"

[llm]
base_url = "https://api.deepseek.com"
api_key = "deepseek-demo"
model = "deepseek-reasoner"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
"#);

    let resolved = pa_app::replay_config::load_replay_config(&path).unwrap();
    assert_eq!(resolved.app_config.llm.providers["default"].base_url, "https://api.deepseek.com");
    assert_eq!(resolved.app_config.llm.execution_profiles["baseline_a_default"].provider, "default");
}
```

- [ ] **Step 2: Run the config-normalization tests to verify the loader does not exist yet**

Run: `cargo test -p pa-app --test replay_config`

Expected: FAIL because `replay_config` is not implemented and `pa-app` is missing direct `toml`/`chrono` support for the new module

- [ ] **Step 3: Implement a replay-only config loader that accepts current shape first, then known legacy shapes**

```rust
pub struct ResolvedReplayConfig {
    pub source_path: PathBuf,
    pub app_config: AppConfig,
}

pub fn load_replay_config(path: impl AsRef<Path>) -> Result<ResolvedReplayConfig, AppError> {
    let path = path.as_ref().to_path_buf();

    if let Ok(app_config) = AppConfig::load_from_path(&path) {
        return Ok(ResolvedReplayConfig {
            source_path: path,
            app_config,
        });
    }

    let raw = std::fs::read_to_string(&path).map_err(|source| AppError::Storage {
        message: format!("failed to read replay config from {}", path.display()),
        source: Some(Box::new(source)),
    })?;

    parse_pa_analyze_server_config(&raw)
        .or_else(|_| parse_stock_everyday_config(&raw))
        .map(|app_config| ResolvedReplayConfig { source_path: path, app_config })
}
```

- [ ] **Step 4: Synthesize a baseline replay binding map instead of copying secrets into repo config**

```rust
fn baseline_a_step_bindings(profile_key: &str) -> std::collections::BTreeMap<String, LlmStepBindingConfig> {
    std::collections::BTreeMap::from([
        ("shared_pa_state_bar_v1".to_string(), LlmStepBindingConfig { execution_profile: profile_key.to_string() }),
        ("shared_bar_analysis_v2".to_string(), LlmStepBindingConfig { execution_profile: profile_key.to_string() }),
        ("shared_daily_context_v2".to_string(), LlmStepBindingConfig { execution_profile: profile_key.to_string() }),
        ("user_position_advice_v2".to_string(), LlmStepBindingConfig { execution_profile: profile_key.to_string() }),
    ])
}
```

- [ ] **Step 5: Run the replay-config tests to verify current and legacy sources normalize to a reusable `AppConfig`**

Run: `cargo test -p pa-app --test replay_config`

Expected: PASS with coverage for current `oh-paa` config shape, `pa-analyze-server` shape, and `stock-everyday` shape

- [ ] **Step 6: Commit the replay-config adapter**

```bash
git add crates/pa-app/Cargo.toml crates/pa-app/src/replay_config.rs crates/pa-app/src/lib.rs crates/pa-app/tests/replay_config.rs
git commit -m "feat: add external replay config normalization"
```

### Task 4: Add the Live Replay Dataset Contract and Validation Rules

**Files:**
- Create: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay_live.rs`
- Create: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\tests\live_replay.rs`
- Create: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\testdata\analysis_replay\live_crypto_15m.json`

- [ ] **Step 1: Write the failing live-dataset loader test**

```rust
#[test]
fn load_live_dataset_reads_target_bar_and_user_fixture() {
    let dataset = pa_app::replay_live::load_live_replay_dataset(
        "testdata/analysis_replay/live_crypto_15m.json",
    )
    .unwrap();

    assert_eq!(dataset.dataset_id, "live_crypto_15m_v1");
    assert_eq!(dataset.market, "crypto");
    assert_eq!(dataset.timeframe, "15m");
    assert_eq!(dataset.pipeline_variant, "baseline_a");
    assert_eq!(dataset.samples.len(), 5);
    assert_eq!(dataset.samples[0].provider, "twelvedata");
    assert_eq!(dataset.samples[0].provider_symbol, "BTC/USD");
    assert_eq!(dataset.samples[0].warmup_bar_count, 8);
    assert!(dataset.samples[0].user_position_json.is_object());
}
```

- [ ] **Step 2: Run the new live-dataset test to verify the loader and file do not exist**

Run: `cargo test -p pa-app load_live_dataset_reads_target_bar_and_user_fixture -- --exact`

Expected: FAIL because `replay_live` and `live_crypto_15m.json` do not exist yet

- [ ] **Step 3: Define the live dataset schema and strict validation rules**

```rust
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
    pub lookback_15m_bars: usize,
    pub warmup_bar_count: usize,
    pub user_position_json: Value,
    pub user_subscription_json: Value,
}

fn validate_live_dataset(dataset: &LiveReplayDataset) -> Result<(), AppError> {
    if dataset.market != "crypto" || dataset.timeframe != "15m" || dataset.pipeline_variant != "baseline_a" {
        return Err(AppError::Validation {
            message: "first live replay slice must be crypto + 15m + baseline_a".to_string(),
            source: None,
        });
    }
    if dataset.samples.iter().any(|sample| sample.warmup_bar_count < 8) {
        return Err(AppError::Validation {
            message: "live replay samples must provide at least 8 warmup bars".to_string(),
            source: None,
        });
    }
    Ok(())
}
```

- [ ] **Step 4: Add the curated crypto-15m dataset file**

```json
{
  "dataset_id": "live_crypto_15m_v1",
  "market": "crypto",
  "timeframe": "15m",
  "pipeline_variant": "baseline_a",
  "samples": [
    {
      "sample_id": "btc-usd-2026-04-17-08-00-breakout",
      "instrument_id": "22222222-2222-2222-2222-222222222202",
      "provider": "twelvedata",
      "provider_symbol": "BTC/USD",
      "display_symbol": "BTC/USD",
      "target_bar_open_time": "2026-04-17T08:00:00Z",
      "target_bar_close_time": "2026-04-17T08:15:00Z",
      "lookback_15m_bars": 192,
      "warmup_bar_count": 8,
      "user_position_json": { "side": "long", "size": 0.25, "avg_price": 83950.0 },
      "user_subscription_json": { "risk_mode": "standard" }
    },
    {
      "sample_id": "btc-usd-2026-04-18-10-15-failed-breakout",
      "instrument_id": "22222222-2222-2222-2222-222222222202",
      "provider": "twelvedata",
      "provider_symbol": "BTC/USD",
      "display_symbol": "BTC/USD",
      "target_bar_open_time": "2026-04-18T10:15:00Z",
      "target_bar_close_time": "2026-04-18T10:30:00Z",
      "lookback_15m_bars": 192,
      "warmup_bar_count": 8,
      "user_position_json": { "side": "long", "size": 0.25, "avg_price": 84210.0 },
      "user_subscription_json": { "risk_mode": "standard" }
    },
    {
      "sample_id": "btc-usd-2026-04-19-03-00-range-rejection",
      "instrument_id": "22222222-2222-2222-2222-222222222202",
      "provider": "twelvedata",
      "provider_symbol": "BTC/USD",
      "display_symbol": "BTC/USD",
      "target_bar_open_time": "2026-04-19T03:00:00Z",
      "target_bar_close_time": "2026-04-19T03:15:00Z",
      "lookback_15m_bars": 192,
      "warmup_bar_count": 8,
      "user_position_json": { "side": "flat", "size": 0, "avg_price": null },
      "user_subscription_json": { "risk_mode": "standard" }
    },
    {
      "sample_id": "btc-usd-2026-04-20-12-00-compression-expansion",
      "instrument_id": "22222222-2222-2222-2222-222222222202",
      "provider": "twelvedata",
      "provider_symbol": "BTC/USD",
      "display_symbol": "BTC/USD",
      "target_bar_open_time": "2026-04-20T12:00:00Z",
      "target_bar_close_time": "2026-04-20T12:15:00Z",
      "lookback_15m_bars": 192,
      "warmup_bar_count": 8,
      "user_position_json": { "side": "long", "size": 0.15, "avg_price": 84820.0 },
      "user_subscription_json": { "risk_mode": "tight" }
    },
    {
      "sample_id": "btc-usd-2026-04-21-17-00-shock-follow-through",
      "instrument_id": "22222222-2222-2222-2222-222222222202",
      "provider": "twelvedata",
      "provider_symbol": "BTC/USD",
      "display_symbol": "BTC/USD",
      "target_bar_open_time": "2026-04-21T17:00:00Z",
      "target_bar_close_time": "2026-04-21T17:15:00Z",
      "lookback_15m_bars": 192,
      "warmup_bar_count": 8,
      "user_position_json": { "side": "long", "size": 0.10, "avg_price": 85240.0 },
      "user_subscription_json": { "risk_mode": "standard" }
    }
  ]
}
```

- [ ] **Step 5: Run the live-dataset test to verify the contract is loadable and validated**

Run: `cargo test -p pa-app --test live_replay load_live_dataset_reads_target_bar_and_user_fixture -- --exact`

Expected: PASS with strict validation of market, timeframe, variant, provider, and warmup-bar requirements

- [ ] **Step 6: Commit the live-dataset contract**

```bash
git add crates/pa-app/src/replay_live.rs crates/pa-app/tests/live_replay.rs testdata/analysis_replay/live_crypto_15m.json
git commit -m "feat: add live crypto replay dataset contract"
```

### Task 5: Implement the Live Historical Replay Runner and Warmup Context Builder

**Files:**
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay_live.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\lib.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\tests\live_replay.rs`

- [ ] **Step 1: Write the failing non-network live-runner test**

```rust
fn fixture_live_dependencies(
    sample: pa_app::replay_live::LiveReplaySample,
) -> FixtureLiveDependencies<QueuedLlmClient> {
    let config = pa_core::AppConfig::load_from_path(
        pa_app::workspace_root().join("config.example.toml"),
    )
    .unwrap();
    let resolved_config = pa_app::replay_config::ResolvedReplayConfig {
        source_path: std::path::PathBuf::from("fixture"),
        app_config: config.clone(),
    };

    let mut providers = pa_market::provider::ProviderMap::default();
    providers.insert(
        "twelvedata",
        std::sync::Arc::new(StaticHistoricalProvider::new(
            &sample.provider_symbol,
            sample.target_bar_open_time - chrono::Duration::minutes(15 * 32),
            32,
        )),
    );
    let provider_router = pa_market::ProviderRouter::new(providers);

    let executor = pa_orchestrator::Executor::new(
        pa_app::build_step_registry_from_config(&config).unwrap(),
        QueuedLlmClient::new(std::collections::VecDeque::from([
            serde_json::json!({
                "bar_identity": { "ticker": "BTC/USD", "timeframe": "15m" },
                "decision_tree_state": {
                    "trend_context": { "state": "up" },
                    "location_context": { "state": "breakout_zone" },
                    "signal_quality": { "state": "good" },
                    "confirmation_state": { "state": "confirmed" },
                    "invalidation_conditions": { "state": "lose_breakout" },
                    "bias_balance": { "state": "bulls_in_control" }
                },
                "support_resistance_map": { "support": 84000.0, "resistance": 84500.0 },
                "signal_assessment": { "signal_quality": "good" }
            }),
            serde_json::json!({
                "bar_identity": { "ticker": "BTC/USD" },
                "bar_summary": { "summary": "warmup bar" },
                "market_story": { "story": "buyers holding range high" },
                "bullish_case": { "path": "continuation" },
                "bearish_case": { "path": "failed break" },
                "two_sided_balance": { "state": "bullish" },
                "key_levels": { "breakout_level": 84500.0 },
                "signal_bar_verdict": { "verdict": "bullish_signal_bar" },
                "continuation_path": { "trigger": "hold_above_84500" },
                "reversal_path": { "trigger": "close_back_below_84320" },
                "invalidation_map": { "bullish_invalidation": 84320.0 },
                "follow_through_checkpoints": { "next_objective": 84950.0 }
            }),
            serde_json::json!({
                "bar_identity": { "ticker": "BTC/USD", "timeframe": "15m" },
                "decision_tree_state": {
                    "trend_context": { "state": "up" },
                    "location_context": { "state": "breakout_zone" },
                    "signal_quality": { "state": "good" },
                    "confirmation_state": { "state": "confirmed" },
                    "invalidation_conditions": { "state": "lose_breakout" },
                    "bias_balance": { "state": "bulls_in_control" }
                },
                "support_resistance_map": { "support": 84000.0, "resistance": 84500.0 },
                "signal_assessment": { "signal_quality": "good" }
            }),
            serde_json::json!({
                "bar_identity": { "ticker": "BTC/USD" },
                "bar_summary": { "summary": "target bar" },
                "market_story": { "story": "acceptance above range high" },
                "bullish_case": { "path": "continuation" },
                "bearish_case": { "path": "failed break" },
                "two_sided_balance": { "state": "bullish_but_extended" },
                "key_levels": { "breakout_level": 84500.0 },
                "signal_bar_verdict": { "verdict": "bullish_signal_bar" },
                "continuation_path": { "trigger": "hold_above_84500" },
                "reversal_path": { "trigger": "close_back_below_84320" },
                "invalidation_map": { "bullish_invalidation": 84320.0 },
                "follow_through_checkpoints": { "next_objective": 84950.0 }
            }),
            serde_json::json!({
                "context_identity": { "ticker": "BTC/USD", "trading_date": "2026-04-17" },
                "market_background": { "state": "risk_on_with_crypto_strength" },
                "dominant_structure": { "state": "daily_uptrend" },
                "intraday_vs_higher_timeframe_state": { "state": "intraday_supports_daily" },
                "key_support_levels": { "levels": [84320.0] },
                "key_resistance_levels": { "levels": [84950.0] },
                "signal_bars": { "primary": "15m_breakout_bar" },
                "candle_pattern_map": { "pattern": "range_expansion" },
                "decision_tree_nodes": {
                    "trend_context": { "state": "up" },
                    "location_context": { "state": "breakout_zone" },
                    "signal_quality": { "state": "credible" },
                    "confirmation_state": { "state": "accepted_above_break" },
                    "invalidation_conditions": { "state": "lose_84320" },
                    "path_of_least_resistance": { "state": "higher" }
                },
                "liquidity_context": { "state": "thin_above_range" },
                "scenario_map": { "base_case": "continuation_then_pause" },
                "risk_notes": { "note": "watch late breakout fatigue" },
                "session_playbook": { "playbook": "buy_dips_above_breakout" }
            }),
            serde_json::json!({
                "position_state": { "state": "in_profit_long" },
                "market_read_through": { "summary": "shared context remains supportive for longs" },
                "bullish_path_for_user": { "action": "hold_core_and_let_it_work" },
                "bearish_path_for_user": { "action": "trim_if_price_accepts_back_inside_range" },
                "hold_reduce_exit_conditions": { "hold_if": "price_stays_above_84500" },
                "risk_control_levels": { "stop_reference": 84320.0 },
                "invalidations": { "primary": "15m_close_below_84320" },
                "action_candidates": { "next_step": "trail_stop_under_breakout" }
            }),
        ])),
    );

    FixtureLiveDependencies {
        resolved_config,
        provider_router,
        executor,
    }
}

struct FixtureLiveDependencies<C> {
    resolved_config: pa_app::replay_config::ResolvedReplayConfig,
    provider_router: pa_market::ProviderRouter,
    executor: pa_orchestrator::Executor<C>,
}

#[derive(Debug, Clone)]
struct QueuedLlmClient {
    outputs: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<serde_json::Value>>>,
}

impl QueuedLlmClient {
    fn new(outputs: std::collections::VecDeque<serde_json::Value>) -> Self {
        Self {
            outputs: std::sync::Arc::new(std::sync::Mutex::new(outputs)),
        }
    }
}

#[async_trait::async_trait]
impl pa_orchestrator::LlmClient for QueuedLlmClient {
    async fn generate_json(
        &self,
        request: &pa_orchestrator::LlmRequest,
    ) -> pa_orchestrator::LlmCallEnvelope {
        let output = self.outputs.lock().unwrap().pop_front().unwrap();
        pa_orchestrator::LlmCallEnvelope::Success(pa_orchestrator::LlmSuccessEnvelope {
            llm_provider: request.provider.clone(),
            model: request.model.clone(),
            request_payload_json: serde_json::json!({
                "provider": request.provider.clone(),
                "model": request.model.clone(),
                "input_json": request.input_json.clone(),
            }),
            raw_response_json: output.clone(),
            parsed_output_json: output,
        })
    }
}

#[derive(Debug)]
struct StaticHistoricalProvider {
    symbol: String,
    bars: Vec<pa_market::ProviderKline>,
}

impl StaticHistoricalProvider {
    fn new(symbol: &str, start: DateTime<Utc>, count: usize) -> Self {
        let bars = (0..count)
            .map(|index| {
                let open_time = start + chrono::Duration::minutes(index as i64 * 15);
                let close_time = open_time + chrono::Duration::minutes(15);
                pa_market::ProviderKline {
                    open_time,
                    close_time,
                    open: rust_decimal::Decimal::from(84_000 + index as i64 * 10),
                    high: rust_decimal::Decimal::from(84_020 + index as i64 * 10),
                    low: rust_decimal::Decimal::from(83_980 + index as i64 * 10),
                    close: rust_decimal::Decimal::from(84_010 + index as i64 * 10),
                    volume: Some(rust_decimal::Decimal::from(100 + index as i64)),
                }
            })
            .collect();

        Self {
            symbol: symbol.to_string(),
            bars,
        }
    }
}

#[async_trait::async_trait]
impl pa_market::MarketDataProvider for StaticHistoricalProvider {
    fn name(&self) -> &'static str {
        "twelvedata"
    }

    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<pa_market::ProviderKline>, pa_core::AppError> {
        Ok(self.bars.clone())
    }

    async fn fetch_klines_window(
        &self,
        query: pa_market::HistoricalKlineQuery,
    ) -> Result<Vec<pa_market::ProviderKline>, pa_core::AppError> {
        assert_eq!(query.provider_symbol, self.symbol);
        Ok(self.bars.clone())
    }

    async fn fetch_latest_tick(
        &self,
        _provider_symbol: &str,
    ) -> Result<pa_market::ProviderTick, pa_core::AppError> {
        panic!("tick access is not required for closed-bar live replay tests")
    }

    async fn healthcheck(&self) -> Result<(), pa_core::AppError> {
        Ok(())
    }
}

#[tokio::test]
async fn live_runner_builds_warmup_context_and_executes_the_target_chain() {
    let mut dataset = pa_app::replay_live::load_live_replay_dataset(
        "testdata/analysis_replay/live_crypto_15m.json",
    )
    .unwrap();
    let mut sample = dataset.samples[0].clone();
    sample.warmup_bar_count = 1;
    dataset.samples = vec![sample.clone()];

    let dependencies = fixture_live_dependencies(sample.clone());
    let report = pa_app::replay_live::run_live_replay_with_dependencies(
        &dataset,
        &dependencies.resolved_config,
        dependencies.provider_router,
        dependencies.executor,
    )
    .await
    .unwrap();

    assert_eq!(report.execution_mode, pa_app::replay::ReplayExecutionMode::LiveHistorical);
    assert_eq!(report.step_runs.len(), 6);
    assert_eq!(report.step_runs[0].step_key, "shared_pa_state_bar");
    assert!(report.step_runs.iter().all(|run| run.raw_response_json.is_some()));
    assert!(report.step_runs.iter().all(|run| run.schema_valid));
}
```

- [ ] **Step 2: Run the live-runner test to verify the runner is still missing**

Run: `cargo test -p pa-app live_runner_builds_warmup_context_and_executes_the_target_chain -- --exact`

Expected: FAIL because there is no live runner, no dependency bundle, and no historical context assembly

- [ ] **Step 3: Build a historical context bundle from fetched `15m` bars**

```rust
fn normalize_live_rows(
    instrument_id: Uuid,
    provider_name: &str,
    bars: Vec<ProviderKline>,
) -> Result<Vec<CanonicalKlineRow>, AppError> {
    bars.into_iter()
        .map(pa_market::normalize_kline)
        .map(|row| {
            row.map(|row| CanonicalKlineRow {
                instrument_id,
                timeframe: Timeframe::M15,
                open_time: row.open_time,
                close_time: row.close_time,
                open: row.open,
                high: row.high,
                low: row.low,
                close: row.close,
                volume: row.volume,
                source_provider: provider_name.to_string(),
            })
        })
        .collect()
}

struct LiveSampleContext {
    target_bar: CanonicalKlineRow,
    trailing_15m_bars: Vec<CanonicalKlineRow>,
    trailing_1h_bars: Vec<AggregatedKline>,
    trailing_1d_bars: Vec<AggregatedKline>,
    trading_date: chrono::NaiveDate,
}

async fn build_live_sample_context(
    router: &ProviderRouter,
    sample: &LiveReplaySample,
) -> Result<LiveSampleContext, AppError> {
    let bars = router
        .fetch_klines_window_from(
            &sample.provider,
            HistoricalKlineQuery {
                provider_symbol: sample.provider_symbol.clone(),
                timeframe: Timeframe::M15,
                start_open_time: Some(sample.target_bar_open_time - chrono::Duration::minutes((sample.lookback_15m_bars as i64) * 15)),
                end_close_time: Some(sample.target_bar_close_time),
                limit: Some(sample.lookback_15m_bars + 16),
            },
        )
        .await?;

    let canonical = normalize_live_rows(sample.instrument_id, &sample.provider, bars)?;
    let target_bar = canonical
        .iter()
        .find(|row| row.open_time == sample.target_bar_open_time && row.close_time == sample.target_bar_close_time)
        .cloned()
        .ok_or_else(|| AppError::Analysis {
            message: format!("missing target closed bar for sample {}", sample.sample_id),
            source: None,
        })?;

    Ok(LiveSampleContext {
        target_bar,
        trailing_15m_bars: canonical.clone(),
        trailing_1h_bars: pa_market::aggregate_replay_rows(&canonical, sample.instrument_id, Timeframe::M15, Timeframe::H1, Some("crypto"), Some("UTC"))?,
        trailing_1d_bars: pa_market::aggregate_replay_rows(&canonical, sample.instrument_id, Timeframe::M15, Timeframe::D1, Some("crypto"), Some("UTC"))?,
        trading_date: sample.target_bar_open_time.date_naive(),
    })
}
```

- [ ] **Step 4: Execute warmup PA-state/shared-bar steps before the target bar**

```rust
async fn execute_live_step<C: pa_orchestrator::LlmClient>(
    executor: &Executor<C>,
    sample: &LiveReplaySample,
    step_key: &str,
    step_version: &str,
    input_json: &Value,
) -> Result<ReplayStepRun, AppError> {
    let outcome = executor.execute_json(step_key, step_version, input_json).await?;

    match outcome {
        pa_orchestrator::ExecutionOutcome::Success(attempt) => Ok(ReplayStepRun {
            sample_id: sample.sample_id.clone(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            prompt_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            input_json: input_json.clone(),
            output_json: attempt.parsed_output_json.unwrap_or(Value::Null),
            raw_response_json: attempt.raw_response_json,
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(0),
            judge_score: None,
            human_notes: None,
        }),
        pa_orchestrator::ExecutionOutcome::SchemaValidationFailed(attempt) => Ok(ReplayStepRun {
            sample_id: sample.sample_id.clone(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            prompt_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            input_json: input_json.clone(),
            output_json: attempt.parsed_output_json.unwrap_or(Value::Null),
            raw_response_json: attempt.raw_response_json,
            schema_valid: false,
            schema_validation_error: attempt.schema_validation_error,
            failure_category: Some("schema_validation_failed".to_string()),
            outbound_error_message: None,
            latency_ms: Some(0),
            judge_score: None,
            human_notes: None,
        }),
        pa_orchestrator::ExecutionOutcome::OutboundCallFailed { attempt, error } => Ok(ReplayStepRun {
            sample_id: sample.sample_id.clone(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            prompt_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            input_json: input_json.clone(),
            output_json: Value::Null,
            raw_response_json: attempt.raw_response_json,
            schema_valid: false,
            schema_validation_error: None,
            failure_category: Some("outbound_call_failed".to_string()),
            outbound_error_message: Some(error.to_string()),
            latency_ms: Some(0),
            judge_score: None,
            human_notes: None,
        }),
    }
}

fn build_shared_pa_state_input(
    sample: &LiveReplaySample,
    context: &LiveSampleContext,
    bar: &CanonicalKlineRow,
) -> Value {
    serde_json::to_value(pa_analysis::SharedPaStateBarInput {
        instrument_id: sample.instrument_id,
        timeframe: Timeframe::M15,
        bar_state: pa_orchestrator::AnalysisBarState::Closed,
        bar_open_time: bar.open_time,
        bar_close_time: bar.close_time,
        bar_json: serde_json::json!({
            "kind": "canonical_closed_bar",
            "open_time": bar.open_time,
            "close_time": bar.close_time,
            "open": bar.open,
            "high": bar.high,
            "low": bar.low,
            "close": bar.close,
            "volume": bar.volume,
            "source_provider": bar.source_provider,
        }),
        market_context_json: serde_json::json!({
            "market": { "market_code": "crypto", "timezone": "UTC" },
            "hourly_structure": context.trailing_1h_bars,
            "daily_structure": context.trailing_1d_bars,
        }),
    })
    .unwrap()
}

fn build_shared_bar_input(
    sample: &LiveReplaySample,
    bar: &CanonicalKlineRow,
    recent_pa_states: Vec<Value>,
) -> Value {
    serde_json::json!({
        "instrument_id": sample.instrument_id,
        "timeframe": "15m",
        "bar_open_time": bar.open_time,
        "bar_close_time": bar.close_time,
        "bar_state": "closed",
        "shared_pa_state_json": recent_pa_states.last().cloned().unwrap_or(Value::Null),
        "recent_pa_states_json": recent_pa_states,
    })
}

async fn build_warmup_context<C: pa_orchestrator::LlmClient>(
    executor: &Executor<C>,
    sample: &LiveReplaySample,
    context: &LiveSampleContext,
) -> Result<(Vec<Value>, Vec<Value>, Vec<ReplayStepRun>), AppError> {
    let warmup_bars = context
        .trailing_15m_bars
        .iter()
        .filter(|bar| bar.close_time <= sample.target_bar_close_time)
        .cloned()
        .collect::<Vec<_>>();

    let mut recent_pa_states = Vec::new();
    let mut recent_shared_bars = Vec::new();
    let mut warmup_runs = Vec::new();

    for bar in warmup_bars.iter().rev().skip(1).take(sample.warmup_bar_count).rev() {
        let pa_input = build_shared_pa_state_input(sample, context, bar);
        let pa_run = execute_live_step(executor, sample, "shared_pa_state_bar", "v1", &pa_input).await?;
        recent_pa_states.push(pa_run.output_json.clone());
        warmup_runs.push(pa_run);

        let bar_input = build_shared_bar_input(sample, bar, recent_pa_states.clone());
        let bar_run = execute_live_step(executor, sample, "shared_bar_analysis", "v2", &bar_input).await?;
        recent_shared_bars.push(bar_run.output_json.clone());
        warmup_runs.push(bar_run);
    }

    Ok((recent_pa_states, recent_shared_bars, warmup_runs))
}
```

- [ ] **Step 5: Execute the four target steps with real upstream dependency wiring and failure capture**

```rust
fn recent_pa_states_with_target(recent: &[Value], target: &Value) -> Vec<Value> {
    let mut values = recent.to_vec();
    values.push(target.clone());
    values
}

fn recent_shared_bars_with_target(recent: &[Value], target: &Value) -> Vec<Value> {
    let mut values = recent.to_vec();
    values.push(target.clone());
    values
}

fn build_shared_daily_input(
    sample: &LiveReplaySample,
    context: &LiveSampleContext,
    recent_pa_states: Vec<Value>,
    recent_shared_bars: Vec<Value>,
) -> Value {
    serde_json::json!({
        "instrument_id": sample.instrument_id,
        "trading_date": context.trading_date,
        "recent_pa_states_json": recent_pa_states,
        "recent_shared_bar_analyses_json": recent_shared_bars,
        "multi_timeframe_structure_json": {
            "1h": context.trailing_1h_bars,
            "1d": context.trailing_1d_bars,
        },
        "market_background_json": {
            "market": "crypto",
            "session_kind": "continuous_utc",
            "volatility_state": "derived_from_recent_15m_window"
        }
    })
}

fn build_user_position_input(
    sample: &LiveReplaySample,
    context: &LiveSampleContext,
    pa_state_json: &Value,
    shared_bar_json: &Value,
    shared_daily_json: &Value,
) -> Value {
    serde_json::json!({
        "user_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1",
        "instrument_id": sample.instrument_id,
        "timeframe": "15m",
        "bar_state": "closed",
        "bar_open_time": context.target_bar.open_time,
        "bar_close_time": context.target_bar.close_time,
        "trading_date": context.trading_date,
        "positions_json": sample.user_position_json,
        "subscriptions_json": sample.user_subscription_json,
        "shared_pa_state_json": pa_state_json,
        "shared_bar_analysis_json": shared_bar_json,
        "shared_daily_context_json": shared_daily_json,
    })
}

let pa_input = build_shared_pa_state_input(sample, &context, &context.target_bar);
let pa_run = execute_live_step(executor, sample, "shared_pa_state_bar", "v1", &pa_input).await?;

let bar_input = build_shared_bar_input(sample, &context.target_bar, recent_pa_states_with_target(&recent_pa_states, &pa_run.output_json));
let bar_run = execute_live_step(executor, sample, "shared_bar_analysis", "v2", &bar_input).await?;

let daily_input = build_shared_daily_input(
    sample,
    &context,
    recent_pa_states_with_target(&recent_pa_states, &pa_run.output_json),
    recent_shared_bars_with_target(&recent_shared_bars, &bar_run.output_json),
);
let daily_run = execute_live_step(executor, sample, "shared_daily_context", "v2", &daily_input).await?;

let user_input = build_user_position_input(sample, &context, &pa_run.output_json, &bar_run.output_json, &daily_run.output_json);
let user_run = execute_live_step(executor, sample, "user_position_advice", "v2", &user_input).await?;
```

- [ ] **Step 6: Wire the public live-replay entrypoint through the real config and real executor**

```rust
fn build_provider_router(config: &AppConfig) -> ProviderRouter {
    let mut providers = pa_market::provider::ProviderMap::default();
    providers.insert(
        "twelvedata",
        std::sync::Arc::new(pa_market::provider::providers::TwelveDataProvider::new(
            &config.twelvedata_base_url,
            &config.twelvedata_api_key,
        )),
    );
    ProviderRouter::new(providers)
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
                "live dataset variant mismatch: dataset={}, requested={pipeline_variant}",
                dataset.pipeline_variant
            ),
            source: None,
        });
    }

    let resolved_config = load_replay_config(config_path)?;
    let provider_router = build_provider_router(&resolved_config.app_config);
    let executor = build_worker_executor_from_config(&resolved_config.app_config)?;

    run_live_replay_with_dependencies(&dataset, &resolved_config, provider_router, executor).await
}
```

- [ ] **Step 7: Run the live-runner tests to verify non-network assembly and target-chain execution**

Run: `cargo test -p pa-app --test live_replay`

Expected: PASS with fixture-backed coverage for context assembly, warmup generation, target-step execution, and report failure capture

- [ ] **Step 8: Commit the live historical runner**

```bash
git add crates/pa-app/src/replay.rs crates/pa-app/src/replay_live.rs crates/pa-app/src/lib.rs crates/pa-app/tests/live_replay.rs
git commit -m "feat: add live historical replay runner"
```

### Task 6: Add Programmatic Scoring and the Operator CLI

**Files:**
- Create: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay_score.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\replay_live.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\src\bin\replay_analysis.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-app\tests\live_replay.rs`

- [ ] **Step 1: Write the failing score-and-cli tests**

```rust
fn sample_live_step_runs() -> Vec<pa_app::replay::ReplayStepRun> {
    vec![
        pa_app::replay::ReplayStepRun {
            sample_id: "sample-1".to_string(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_pa_state_bar".to_string(),
            step_version: "v1".to_string(),
            prompt_version: "v1".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "deepseek-v3.2".to_string(),
            input_json: serde_json::json!({}),
            output_json: serde_json::json!({
                "decision_tree_state": {},
                "support_resistance_map": {},
                "signal_assessment": {}
            }),
            raw_response_json: Some(serde_json::json!({ "ok": true })),
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(200),
            judge_score: None,
            human_notes: None,
        },
        pa_app::replay::ReplayStepRun {
            sample_id: "sample-1".to_string(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_bar_analysis".to_string(),
            step_version: "v2".to_string(),
            prompt_version: "v2".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "deepseek-v3.2".to_string(),
            input_json: serde_json::json!({}),
            output_json: serde_json::json!({
                "bullish_case": {},
                "bearish_case": {},
                "two_sided_balance": {}
            }),
            raw_response_json: Some(serde_json::json!({ "ok": true })),
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(250),
            judge_score: None,
            human_notes: None,
        },
    ]
}

#[test]
fn score_step_runs_reports_completeness_and_consistency_metrics() {
    let scores = pa_app::replay_score::score_step_runs(&sample_live_step_runs());

    assert!(scores.contains_key("decision_tree_completeness"));
    assert!(scores.contains_key("key_level_completeness"));
    assert!(scores.contains_key("signal_bar_completeness"));
    assert!(scores.contains_key("bull_bear_dual_path_completeness"));
    assert!(scores.contains_key("cross_step_consistency_rate"));
}

#[test]
fn replay_cli_parser_requires_config_for_live_mode() {
    let error = pa_app::replay::parse_replay_cli_args([
        "replay_analysis",
        "--mode",
        "live",
        "--dataset",
        "testdata/analysis_replay/live_crypto_15m.json",
        "--variant",
        "baseline_a",
    ])
    .expect_err("live mode without config must fail");

    assert!(error.to_string().contains("--config"));
}
```

- [ ] **Step 2: Run the score-and-cli tests to verify the scorer module and parser do not exist**

Run: `cargo test -p pa-app --test live_replay`

Expected: FAIL because `replay_score` and `parse_replay_cli_args` are not implemented

- [ ] **Step 3: Implement structural, completeness, and consistency scores**

```rust
pub fn score_step_runs(step_runs: &[ReplayStepRun]) -> Map<String, Value> {
    let total_step_runs = step_runs.len() as u64;
    let valid_step_runs = step_runs.iter().filter(|run| run.schema_valid).count() as u64;
    let schema_hit_rate = ratio(valid_step_runs, total_step_runs);
    let latency_coverage = ratio(
        step_runs.iter().filter(|run| run.latency_ms.is_some()).count() as u64,
        total_step_runs,
    );

    Map::from_iter([
        ("total_step_runs".to_string(), Value::from(total_step_runs)),
        ("valid_step_runs".to_string(), Value::from(valid_step_runs)),
        ("schema_hit_rate".to_string(), Value::from(schema_hit_rate)),
        ("latency_coverage".to_string(), Value::from(latency_coverage)),
        ("avg_latency_ms".to_string(), Value::from(avg_latency_ms(step_runs))),
        ("decision_tree_completeness".to_string(), Value::from(required_path_score(step_runs, "shared_pa_state_bar", &["decision_tree_state", "support_resistance_map", "signal_assessment"]))),
        ("key_level_completeness".to_string(), Value::from(required_path_score(step_runs, "shared_daily_context", &["key_support_levels", "key_resistance_levels"]))),
        ("signal_bar_completeness".to_string(), Value::from(required_path_score(step_runs, "shared_daily_context", &["signal_bars", "candle_pattern_map"]))),
        ("bull_bear_dual_path_completeness".to_string(), Value::from(required_path_score(step_runs, "shared_bar_analysis", &["bullish_case", "bearish_case", "two_sided_balance"]))),
        ("cross_step_consistency_rate".to_string(), Value::from(cross_step_consistency_rate(step_runs))),
    ])
}
```

- [ ] **Step 4: Add the CLI parser and explicit mode dispatch**

```rust
pub struct ReplayCliArgs {
    pub mode: ReplayExecutionMode,
    pub dataset_path: String,
    pub config_path: Option<String>,
    pub variant: String,
}

fn value_after(values: &[String], flag: &str) -> Option<String> {
    values
        .iter()
        .position(|value| value == flag)
        .and_then(|index| values.get(index + 1))
        .cloned()
}

pub fn parse_replay_cli_args<I, S>(args: I) -> Result<ReplayCliArgs, anyhow::Error>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let values = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let mode = value_after(&values, "--mode").unwrap_or_else(|| "fixture".to_string());
    let dataset_path = value_after(&values, "--dataset").ok_or_else(|| anyhow::anyhow!("missing --dataset"))?;
    let variant = value_after(&values, "--variant").ok_or_else(|| anyhow::anyhow!("missing --variant"))?;
    let config_path = value_after(&values, "--config");

    let mode = match mode.as_str() {
        "fixture" => ReplayExecutionMode::Fixture,
        "live" => ReplayExecutionMode::LiveHistorical,
        other => return Err(anyhow::anyhow!("unsupported --mode {other}")),
    };

    if mode == ReplayExecutionMode::LiveHistorical && config_path.is_none() {
        return Err(anyhow::anyhow!("--config is required when --mode live"));
    }

    Ok(ReplayCliArgs { mode, dataset_path, config_path, variant })
}
```

- [ ] **Step 5: Update the binary to print comparable JSON for both replay modes**

```rust
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = pa_app::replay::parse_replay_cli_args(std::env::args())?;
    let report = match args.mode {
        pa_app::replay::ReplayExecutionMode::Fixture => {
            pa_app::replay::run_fixture_replay_variant_from_path(args.dataset_path, &args.variant).await?
        }
        pa_app::replay::ReplayExecutionMode::LiveHistorical => {
            pa_app::replay::run_live_historical_replay_from_path(
                args.dataset_path,
                args.config_path.expect("live mode validated config"),
                &args.variant,
            )
            .await?
        }
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
```

- [ ] **Step 6: Run replay tests plus fixture and live CLI smokes**

Run: `cargo test -p pa-app --test replay --test replay_config --test live_replay`

Expected: PASS with scoring keys present, live-mode parser validation, and mode-specific report generation

- [ ] **Step 7: Commit the scoring and CLI slice**

```bash
git add crates/pa-app/src/replay_score.rs crates/pa-app/src/replay.rs crates/pa-app/src/replay_live.rs crates/pa-app/src/bin/replay_analysis.rs crates/pa-app/tests/live_replay.rs
git commit -m "feat: add live replay scoring and cli"
```

### Task 7: Run Real Historical Replay, Tighten the First Prompt, and Document the Loop

**Files:**
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\crates\pa-analysis\src\prompt_specs.rs`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\docs\architecture\phase1-runtime.md`
- Modify: `E:\rust-app\oh-paa\.worktrees\analysis-pipeline-optimization\docs\superpowers\plans\2026-04-22-live-historical-replay-implementation.md`

- [ ] **Step 1: Run the targeted replay tests and the full workspace suite**

Run: `cargo test -p pa-market --test historical_window && cargo test -p pa-app --test replay --test replay_config --test live_replay && cargo test --workspace`

Expected: PASS before the first real historical execution is attempted

- [ ] **Step 2: Run the first real live replay with the approved historical slice**

Run: `cargo run -p pa-app --bin replay_analysis -- --mode live --dataset testdata/analysis_replay/live_crypto_15m.json --config E:\rust-app\pa-analyze-server\config.toml --variant baseline_a`

Expected: PASS with a JSON report containing `execution_mode = "live_historical"`, `config_source_path`, real `raw_response_json`, per-step `latency_ms`, and completeness/consistency scores

- [ ] **Step 3: Tighten the first optimization target in `shared_pa_state_bar_prompt_v1`**

```rust
vec![
    "Always populate `decision_tree_state.trend_context`, `decision_tree_state.location_context`, `decision_tree_state.signal_quality`, `decision_tree_state.confirmation_state`, `decision_tree_state.invalidation_conditions`, and `decision_tree_state.bias_balance`.",
    "Always populate `support_resistance_map` with levels derived from the supplied bar and higher-timeframe context; if confidence is low, keep the field and say the level is approximate instead of omitting it.",
    "Always populate `signal_assessment` and explain both bullish and bearish evidence before resolving the final bias.",
    "Never replace required structured fields with free-form prose outside the JSON schema.",
]
```

- [ ] **Step 4: Re-run the PA-analysis tests and the same live replay after the prompt update**

Run: `cargo test -p pa-analysis && cargo run -p pa-app --bin replay_analysis -- --mode live --dataset testdata/analysis_replay/live_crypto_15m.json --config E:\rust-app\pa-analyze-server\config.toml --variant baseline_a`

Expected: PASS with schema validity unchanged from the first live report and `decision_tree_completeness` not lower than the first live run

- [ ] **Step 5: Document the operator flow and the first prompt-iteration comparison**

```markdown
## Live Historical Replay

- Mode split:
  - `fixture`: deterministic fixture validation
  - `live_historical`: real provider data plus real LLM execution
- First validated slice: `crypto + 15m + baseline_a + closed bars only`
- Required command:
  - `cargo run -p pa-app --bin replay_analysis -- --mode live --dataset testdata/analysis_replay/live_crypto_15m.json --config E:\rust-app\pa-analyze-server\config.toml --variant baseline_a`
- First prompt-iteration target: `shared_pa_state_bar_v1`
- Compare the pre- and post-prompt reports on:
  - `schema_hit_rate`
  - `decision_tree_completeness`
  - `bull_bear_dual_path_completeness`
  - `cross_step_consistency_rate`
```

- [x] **Step 6: Mark completed checkboxes in this plan only after the live replay and prompt rerun succeed**

```markdown
- [x] **Step 1: Run the targeted replay tests and the full workspace suite**
- [x] **Step 2: Run the first real live replay with the approved historical slice**
- [x] **Step 3: Tighten the first optimization target in `shared_pa_state_bar_prompt_v1`**
- [x] **Step 4: Re-run the PA-analysis tests and the same live replay after the prompt update**
- [x] **Step 5: Document the operator flow and the first prompt-iteration comparison**
```

- [x] **Step 7: Commit the live replay implementation and first prompt iteration**

```bash
git add crates/pa-analysis/src/prompt_specs.rs docs/architecture/phase1-runtime.md docs/superpowers/plans/2026-04-22-live-historical-replay-implementation.md
git commit -m "feat: add live historical replay and first prompt iteration"
```

Status: completed on `2026-04-22` (`2a61bca`) and merged into `master`.

## Task 7 Execution Log (2026-04-22)

### Completed Evidence

- Pre-live verification command set passed:
  - `cargo test -p pa-market --test historical_window`
  - `cargo test -p pa-app --test replay --test replay_config --test live_replay`
  - `cargo test --workspace`
- Live replay runtime and config hardening completed during real-call iteration:
  - legacy `pa-analyze-server` config normalization now accepts minimal `llm.<provider>` blocks by applying default retry/timeout values when absent.
  - openai-compatible client no longer emits unsupported `developer` role to DashScope-compatible endpoints.
  - non-schema profiles default to `response_format = json_object` for stronger structured-output compliance.
  - live replay now captures per-step latency and richer warmup failure previews.
- Prompt iteration hardening completed for all four runtime steps:
  - `shared_pa_state_bar_v1`
  - `shared_bar_analysis_v2`
  - `shared_daily_context_v2`
  - `user_position_advice_v2`

### Live Replay Outcome Snapshot

- Full target chain success has been validated in real live mode on the first-sample slice (real TwelveData fetch + real LLM calls):
  - `execution_mode = LiveHistorical`
  - target steps executed: 4/4
  - schema-valid steps: 4/4
  - score snapshot:
    - `schema_hit_rate = 1.0`
    - `decision_tree_completeness = 1.0`
    - `key_level_completeness = 1.0`
    - `signal_bar_completeness = 1.0`
    - `bull_bear_dual_path_completeness = 1.0`
    - `cross_step_consistency_rate = 1.0`

### Operational Recommendation

- Use single-sample live replay for rapid prompt iteration loops (cost/latency controlled, high feedback speed).
- Schedule multi-sample full-slice runs as periodic regression sweeps because warmup chains trigger substantial additional live LLM calls.
