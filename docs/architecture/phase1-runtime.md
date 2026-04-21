# Phase 1 Runtime Notes

## Scope

Phase 1 exposes a minimal Axum runtime that groups routes under `/admin`, `/market`,
`/analysis`, and `/user`, plus `/healthz` for process-level health checks. The grouped
routes are placeholders only and intentionally do not claim full product behavior yet.

## Startup

`pa-app` initializes tracing, loads `config.toml` via `pa_core::config::load()`, builds
the shared API state, binds the configured TCP listener, and serves the Axum router.

## Operator Notes

- The process expects a `config.toml` in the current working directory.
- `/healthz` is the only endpoint that should be treated as operationally meaningful in
  Phase 1.
- The grouped routes confirm router wiring and ownership boundaries while downstream
  services, storage, and provider integrations are still being layered in.

## Phase 2 Worker Notes

- Shared bar analysis, shared daily context, and manual user analysis now enqueue durable
  orchestration tasks before any LLM execution happens.
- The API and background worker currently share an in-memory orchestration repository, which
  keeps the Phase 2 pipeline runnable without introducing production database wiring yet.
- Workers execute strictly from persisted snapshot JSON and write attempts, results, or
  dead letters back to orchestration storage.
- Closed-bar tasks deduplicate by task identity while open-bar tasks remain repeatable by
  design.
- Prompt registration is code-defined at startup, and the current runtime uses a fixture LLM
  transport so the orchestration loop can be exercised before a production model client lands.
