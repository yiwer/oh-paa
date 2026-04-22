# Phase 1 Runtime Notes

## Scope

The runtime now exposes Axum routes under `/admin`, `/market`, `/analysis`, and `/user`,
plus `/healthz` for process-level health checks. `/analysis` and `/user` still drive the
Phase 2 orchestration flow, while `/admin` and `/market` are now backed by real provider,
instrument-policy, and PostgreSQL market-data dependencies.

## Startup

`pa-app` initializes tracing, loads `config.toml` via `pa_core::config::load()`, opens a
PostgreSQL pool, runs SQL migrations from `migrations/`, registers `eastmoney` and
`twelvedata` providers, builds the shared API state, binds the configured TCP listener,
and serves the Axum router.

## Operator Notes

- The process expects a `config.toml` in the current working directory.
- `/healthz` remains the process-level probe endpoint.
- `POST /admin/market/backfill` resolves instrument policy and provider bindings from the
  database, fetches canonical K-lines through the provider router, and upserts closed bars
  into PostgreSQL.
- `GET /market/canonical` reads canonical K-lines from PostgreSQL.
- `GET /market/aggregated` aggregates canonical bars into higher timeframes and marks
  incomplete buckets explicitly.
- `config.example.toml` documents the expected runtime configuration shape; local
  `config.toml` should stay uncommitted.

## Phase 2 Worker Notes

- Shared bar analysis, shared daily context, and manual user analysis now enqueue durable
  orchestration tasks before any LLM execution happens.
- The API and background worker currently share an in-memory orchestration repository, which
  keeps the Phase 2 pipeline runnable while the market-data path is already production-backed.
- Workers execute strictly from persisted snapshot JSON and write attempts, results, or
  dead letters back to orchestration storage.
- Closed-bar tasks deduplicate by task identity while open-bar tasks remain repeatable by
  design.
- Prompt registration is code-defined at startup, and the app runtime now binds each analysis
  step to real OpenAI-compatible provider/model profiles from configuration.

## Replay Evaluation Notes

- `cargo run -p pa-app --bin replay_analysis -- testdata/analysis_replay/sample_set.json baseline_a`
  replays historical A-share, crypto, and forex fixtures through the layered analysis pipeline.
- Replay reports log a deterministic experiment id, dataset id, pipeline variant, per-step
  provider/model metadata, step outputs, schema validation status, and aggregate programmatic
  scores such as `schema_hit_rate` and `latency_coverage`.
- The first replay implementation is intentionally offline and deterministic: it validates
  pre-baked variant outputs against the real registered step schemas so prompt/flow experiments
  can be compared without requiring live LLM calls.

## Live Historical Replay Operator Flow

- Live mode command shape:
  - `cargo run -p pa-app --bin replay_analysis -- --mode live --dataset <dataset.json> --config <config.toml> --variant baseline_a`
- Replay mode contracts:
  - `execution_mode` must be `LiveHistorical`.
  - `config_source_path` must be present.
  - `raw_response_json` and `latency_ms` are recorded per target step.
- Data contract expectations:
  - dataset market/timeframe/variant remain constrained to `crypto + 15m + baseline_a`.
  - lookback depth must satisfy `lookback_15m_bar_count >= warmup_bar_count + 1`.
  - warmup bars and target bars are strict closed bars for this first slice.

## Quality Optimization Funnel

- Config source for funnel runs:
  - Use `config.live-replay-quality.example.toml` as the template for a local uncommitted config file passed with `--config`.
  - Do not run the funnel with `config.example.toml`.
- Stage 1 single-step probe:
  - `cargo run -p pa-app --bin replay_probe -- --config <config.toml> --step shared_pa_state_bar:v1 --input testdata/analysis_replay/probe_shared_pa_state_input.json`
  - Gate rule: run repeated checks on the same step and input; only advance when outputs remain structurally stable (valid JSON with required object skeleton preserved across runs).
- Stage 2 single-sample chain:
  - `cargo run -p pa-app --bin replay_analysis -- --mode live --dataset <single-sample.json> --config <config.toml> --variant baseline_a`
- Stage 3 five-sample gate:
  - `cargo run -p pa-app --bin replay_analysis -- --mode live --dataset testdata/analysis_replay/live_crypto_15m.json --config <config.toml> --variant baseline_a`
  - Final success rule: the same candidate must pass two consecutive Stage 3 `live_crypto_15m.json` runs.
- Archive conventions:
  - `docs/superpowers/archives/<date>-<candidate>-report.json`
  - `docs/superpowers/archives/<date>-<candidate>-findings.md`
  - `docs/superpowers/archives/<date>-<candidate>-run.log`

## Prompt Iteration Findings (2026-04-22)

- Shared pipeline prompt hardening:
  - `shared_pa_state_bar_v1` now enforces full top-level schema skeleton, full `decision_tree_state` subtree, and object-only `evidence_log`.
  - `shared_bar_analysis_v2` now enforces canonical key names (`bullish_case`, `bearish_case`, etc.) and rejects ad-hoc aliases.
  - `shared_daily_context_v2` now enforces object-only `decision_tree_nodes`, object-only `signal_bars`, and object-only `path_of_least_resistance`.
  - `user_position_advice_v2` now enforces required top-level keys (`position_state`, `market_read_through`, etc.) and rejects alias keys like `user_position`.
- OpenAI-compatible robustness updates:
  - `developer` role instructions are folded into `system` content for broader provider compatibility.
  - Non-JSON-schema execution profiles now default to `response_format = json_object` to reduce malformed JSON outputs.
  - JSON parsing now tolerates fenced JSON and wrapper text before strict schema validation.
- Legacy config normalization hardening:
  - `pa-analyze-server` legacy shape now tolerates missing retry/timeout legacy keys by applying conservative defaults.

## Model Slice Result (Single-Sample Live Replay)

- Verified successful full target chain (`shared_pa_state_bar -> shared_bar_analysis -> shared_daily_context -> user_position_advice`) on real provider data and real LLM calls.
- Best validated slice so far:
  - provider: `dashscope`
  - model: `qwen-plus`
  - per-step tokens:
    - `shared_pa_state_bar`: 4096
    - `shared_bar_analysis`: 4096
    - `shared_daily_context`: 8192
    - `user_position_advice`: 4096
- Single-sample report quality (latest validated run):
  - `schema_hit_rate = 1.0`
  - `decision_tree_completeness = 1.0`
  - `key_level_completeness = 1.0`
  - `signal_bar_completeness = 1.0`
  - `bull_bear_dual_path_completeness = 1.0`
  - `cross_step_consistency_rate = 1.0`

## Expansion Note

- Full multi-sample `live_crypto_15m.json` runs are significantly slower because warmup bars execute real LLM calls.
- Operationally, prompt iteration should use single-sample live replay for tight loops, then run expanded sample sets for periodic regression sweeps.
