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
