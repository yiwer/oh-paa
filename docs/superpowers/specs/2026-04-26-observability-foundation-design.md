Date: 2026-04-26
Project: `oh-paa`
Status: Drafted for review

# Observability Foundation Design

## 1. Overview

`oh-paa` currently exposes only `tracing` text logs. There are no metrics,
no distributed traces, no health endpoints, and no structured runtime view of
the system. Every architectural concern that follows this work — durable
orchestration verification, persistence equivalence testing, provider
resilience, audit and tenancy — depends on being able to *see* what the system
is doing in production. Without an observability foundation, those refactors
are blind.

This design introduces a four-pillar observability layer:

- Prometheus-style metrics, scraped from an HTTP endpoint
- OpenTelemetry distributed traces, exported via OTLP when configured
- Structured logs (JSON) with `trace_id` correlation, replacing the current
  `tracing-fmt` text output
- Liveness and readiness HTTP endpoints

It does so by adding a new workspace crate `pa-observability` that owns
initialization and exposes a domain-semantic API. Business crates do not
write metric name strings — they call typed functions like
`orchestration::task_completed(...)`. This trades a small amount of
boilerplate in `pa-observability` for a single point of control over naming,
labels, and registration.

## 2. Goals

- Ship metrics, traces, structured logs, and health endpoints as a single
  coherent foundation
- Define a complete signal catalog (metrics + key trace spans) covering the
  five observable domains: orchestration, LLM, market, API, infrastructure
- Make business code instrument observability through a typed domain API,
  not free-form metric strings
- Make OTLP trace export optional: absent configuration means silent
  disable, never noisy warnings or failed startup
- Provide hard verification that the foundation actually emits what the
  catalog claims (CI check + end-to-end replay validation)
- Ship a `signals.md` reference document and a CI guard that prevents code
  and documentation from drifting apart

This phase must end with:

- one new crate `pa-observability`
- `/metrics`, `/healthz`, `/readyz` HTTP routes on the existing `pa-api`
  Axum router
- instrumentation calls at every signal listed in section 6
- structured JSON log output with `trace_id` correlation
- one `docs/observability/signals.md` reference and one CI check enforcing
  catalog–code consistency
- one operator validation: a single `replay_analysis` run produces all
  catalog metrics with non-zero counts where applicable

## 3. Non-Goals

This phase does not include:

- continuous profiling (`pprof`, `tokio-console`)
- automated tests for trace span structure (link relationships, attribute
  presence) — these are validated manually
- prebuilt Grafana dashboards or alert rules — the foundation exposes
  signals; dashboards belong to the operator side and iterate independently
- code generation of `signals.md` from source — manual maintenance plus CI
  consistency check is sufficient
- moving away from `tracing` to a different logging facade
- changes to business logic; this work only adds instrumentation at existing
  flow points

## 4. Fixed Constraints

- `tracing` stays as the logging facade. `pa-observability` adds a JSON
  formatter and OTLP layer; existing `tracing::info!`, `span!`, and
  `#[instrument]` calls are unchanged.
- Metrics use the `metrics` facade crate plus the
  `metrics-exporter-prometheus` backend. Business crates depend on
  `pa-observability`, never directly on `metrics-exporter-prometheus`.
- Trace export uses `opentelemetry-otlp` over the standard
  `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable. When the variable is
  absent, the OTLP layer is not installed at all (not "installed and
  failing").
- The `/metrics` endpoint is mounted on the existing `pa-api` Axum router
  on the same port as the application API. No separate admin port.
- All existing 204 tests must continue to pass. No business behavior change
  is permitted in this phase.
- Decimal precision, error types, and orchestration semantics are out of
  scope and must not be modified.

## 5. Architecture

### 5.1 New Crate: `pa-observability`

```
crates/pa-observability/
├── Cargo.toml
├── src/
│   ├── lib.rs              # public re-exports + init()
│   ├── init.rs             # subscriber + metrics exporter + OTLP wiring
│   ├── config.rs           # ObservabilityConfig (env-driven)
│   ├── health.rs           # health check trait + composite checker
│   ├── http.rs             # axum::Router for /metrics /healthz /readyz
│   └── domain/
│       ├── mod.rs
│       ├── orchestration.rs
│       ├── llm.rs
│       ├── market.rs
│       ├── api.rs
│       └── infra.rs
```

Each `domain/*.rs` file owns:

- the canonical metric names for that domain (`const` strings, private)
- typed recording functions (e.g., `pub fn task_completed(prompt_key: &str,
  outcome: TaskOutcome, duration: Duration)`)
- enums for label values (e.g., `TaskOutcome::{Success, Failure, DeadLetter}`)
  so business code passes types, not strings

The crate exposes only the domain functions plus `init()` and `router()`.
Metric name strings never leak.

### 5.2 Initialization

`pa-observability::init(config: ObservabilityConfig) -> Result<Guard>`:

1. Build a `tracing_subscriber::Registry` with:
   - `tracing_subscriber::fmt` layer in JSON mode, writing to stdout
   - `tracing_opentelemetry::layer` only when `config.otlp_endpoint`
     resolves to `Some`
2. Install the global metrics recorder via
   `PrometheusBuilder::new().install_recorder()` and store the handle.
3. Pre-register all metric descriptions (using `metrics::describe_*`) so
   the `/metrics` output is self-documenting and helps with discovery.
4. Return a `Guard` that on drop flushes the OTLP exporter (if installed).

`pa-app` replaces its current tracing init with one call to
`pa_observability::init()`. The returned `Guard` is held in `main` for the
process lifetime.

### 5.3 HTTP Surface

`pa-observability::router()` returns an `axum::Router` exposing:

- `GET /metrics` — Prometheus text format from the stored handle
- `GET /healthz` — always 200 with body `ok`
- `GET /readyz` — runs each registered `HealthCheck`, returns 200 only if
  all pass; 503 with a JSON body listing failed checks otherwise

Health checks are pluggable. `pa-app` registers:

- `PgHealthCheck` — runs `SELECT 1` against the orchestration pool with a
  500ms timeout
- `ProviderConfigLoadedCheck` — confirms provider registry was populated
  during startup

`pa-api` merges `pa_observability::router()` into its top-level router via
`Router::merge`, giving these routes the same host and port as the rest of
the API.

### 5.4 Configuration

Environment-driven, parsed once at startup:

| Variable | Default | Effect |
|---|---|---|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | unset | If unset, OTLP layer is not installed. If set, traces export to this endpoint. |
| `OTEL_SERVICE_NAME` | `oh-paa` | Service identifier on exported spans. |
| `LOG_FORMAT` | `json` | `json` or `text`. `text` retained for local dev only. |
| `RUST_LOG` | `info,oh_paa=debug` | Standard `tracing` filter. |

No new TOML config keys; observability config is environment-only to keep
deployment configuration in one place (the env).

### 5.5 Trace Boundary Rule

Asynchronous task consumption does **not** continue the API request trace.
When a task is enqueued via the API:

- the `api.request` span records a `task_id` attribute and ends normally
- the `orchestration.execute_attempt` span starts a new trace and records
  an OpenTelemetry `Link` back to the enqueue span's `SpanContext`

This matches industry practice for queue-based async work and prevents
unbounded trace lifetimes. End-to-end correlation is preserved through the
explicit link plus the shared `task_id` attribute.

## 6. Signal Catalog

All metrics use seconds as the time unit and follow Prometheus naming
conventions (`_total` suffix for counters, `_seconds` suffix for
histograms).

### 6.1 Orchestration Domain

| Metric | Type | Labels | Purpose |
|---|---|---|---|
| `orchestration_tasks_total` | counter | `status`, `prompt_key`, `market` | Task state transitions |
| `orchestration_queue_depth` | gauge | `state` (pending\|claimed\|dead) | Queue health |
| `orchestration_claim_duration_seconds` | histogram | — | `claim_next_pending_task` time, signals Pg row-lock contention |
| `orchestration_task_duration_seconds` | histogram | `prompt_key`, `outcome`, `market` | End-to-end task time |
| `orchestration_attempts_per_task` | histogram | `prompt_key` | Retry distribution |
| `orchestration_dead_letter_total` | counter | `reason` | DLQ entry causes |

### 6.2 LLM Domain

| Metric | Type | Labels | Purpose |
|---|---|---|---|
| `llm_request_duration_seconds` | histogram | `provider`, `model`, `outcome`, `market` | Per-call latency |
| `llm_tokens_total` | counter | `provider`, `model`, `kind` (in\|out) | Cost observability |
| `llm_schema_validation_total` | counter | `prompt_key`, `outcome` | Schema hit rate |
| `llm_retry_total` | counter | `provider`, `reason` (transient\|rate_limited\|schema) | Retry pressure |

### 6.3 Market Domain

| Metric | Type | Labels | Purpose |
|---|---|---|---|
| `market_provider_requests_total` | counter | `provider`, `outcome`, `market` | Provider call volume |
| `market_provider_duration_seconds` | histogram | `provider`, `market` | Provider latency |
| `market_bars_ingested_total` | counter | `market`, `timeframe` | Ingestion volume |

### 6.4 API Domain

| Metric | Type | Labels | Purpose |
|---|---|---|---|
| `http_requests_total` | counter | `route`, `method`, `status` | Request volume |
| `http_request_duration_seconds` | histogram | `route` | Endpoint latency |

### 6.5 Infrastructure

| Metric | Type | Labels | Purpose |
|---|---|---|---|
| `pg_pool_connections` | gauge | `state` (idle\|active) | Pool saturation |
| `process_*` | varied | — | Standard process metrics from `metrics-exporter-prometheus` |

### 6.5.1 The `market` Label

All metrics that carry a `market` label source the value from
`InstrumentMarketDataContext::market.code`, populated by the
`MarketGateway` introduced in sub-project B
(`docs/superpowers/specs/2026-04-26-market-gateway-design.md`).

Instrumentation points that genuinely have no instrument context
(infrastructure metrics, OTLP exporter metrics, LLM calls dispatched
without an instrument) emit the literal value `unknown`. This is a
documented escape hatch, not a workaround for missing plumbing — the CI
catalog check (§7.3) enforces that any newly-added metric carrying the
`market` label appears in `signals.md` with `unknown` documented as a
permitted value.

The label uses the immutable, controlled vocabulary `Market.code`
(e.g., `cn-a`, `continuous-utc`, `fx`), so cardinality is bounded by
the number of supported markets — currently a handful, never user
input.

### 6.6 Trace Spans

Standard span hierarchy (within a single trace):

- `api.request` → `orchestration.enqueue`
- `orchestration.execute_attempt` (root of new trace, linked to enqueue)
  → `llm.call` | `schema.validate` | `repository.save_result`
- `market.provider.fetch` (own trace per fetch, no parent)

Required attributes:

- `task_id` on every orchestration and downstream span
- `prompt_key` on `orchestration.execute_attempt`
- `provider`, `model` on `llm.call`
- `provider` on `market.provider.fetch`

## 7. Testing Strategy

### 7.1 Unit Tests (in `pa-observability`)

For each `domain/*.rs`, snapshot-based tests using
`metrics::debugging::Snapshotter`:

- call the typed recording function
- assert the resulting snapshot contains the expected metric name, type,
  label set, and value

This pins both naming and label cardinality. Adding a new label without
updating the test fails CI.

### 7.2 Integration Tests (in existing crates)

Augment three existing integration tests, one per critical domain:

- `pa-orchestrator`: after running one task to completion, assert
  `orchestration_task_duration_seconds_count >= 1` and the corresponding
  `orchestration_tasks_total{status="completed"}` increment
- `pa-api`: after one HTTP request, assert `http_requests_total` increment
  with the right labels and `http_request_duration_seconds_count >= 1`
- `pa-market`: after one provider fetch, assert
  `market_provider_requests_total` and `market_provider_duration_seconds`
  both moved

### 7.3 Catalog Consistency Check (CI)

A small consistency check, implemented as a `#[test]` inside
`pa-observability` (no new tooling required):

- includes `docs/observability/signals.md` via `include_str!`
- parses out the metric name column
- compares against the set of metric name constants registered in
  `pa-observability::domain::*`
- fails if either side has a name the other doesn't

Running as a regular test means it is covered by the existing
`cargo test` step in CI; no workflow change is needed.

### 7.4 What Is Not Tested

- trace span structure, attribute presence, link relationships — validated
  manually during the operator validation step
- OTLP wire-level export — assumed correct based on upstream library

## 8. Operator Validation (Definition of Done)

The phase is not complete until an operator runs through this checklist
end-to-end on their workstation and records the result in
`docs/observability/runbook.md`:

1. Start the app with `OTEL_EXPORTER_OTLP_ENDPOINT` unset. Confirm:
   - startup log line: `observability initialized: metrics=on, otlp=off`
   - no warnings about missing OTLP configuration
2. `curl /healthz` returns 200; `curl /readyz` returns 200.
3. Stop PostgreSQL. `curl /readyz` returns 503 with a JSON body naming
   `pg` as the failed check. Restart Pg; `/readyz` returns 200 again.
4. Run `cargo run -p pa-app --bin replay_analysis -- <fixture>`. After
   completion, `curl /metrics` shows non-zero values for:
   - `orchestration_task_duration_seconds_count`
   - `llm_request_duration_seconds_count`
   - `llm_tokens_total`
   - `orchestration_tasks_total{status="completed"}`
5. Start the app with `OTEL_EXPORTER_OTLP_ENDPOINT=http://<collector>:4317`
   pointed at any OTLP receiver. Confirm spans arrive (screenshot saved to
   `docs/observability/`).
6. `cargo test` passes (all 204 existing + new tests).
7. `cargo test -p pa-observability` includes the catalog-consistency test
   and it passes.

## 9. Rollout Plan

This is additive, not behavior-changing, so a staged rollout is unnecessary.
The merge plan:

1. Add `pa-observability` crate with init, http, and empty domain modules
2. Wire `pa-app` to call `pa_observability::init()` and `pa-api` to merge
   the router. Confirm `/metrics` is empty but reachable and `/healthz`
   responds.
3. Implement one domain at a time (orchestration → LLM → market → API →
   infra), each as its own commit, each landing instrumentation calls plus
   unit tests plus integration test updates plus signals.md additions.
4. Add the catalog-consistency `#[test]` in `pa-observability`.
5. Run operator validation; record results.

Each step is independently mergeable and preserves a working system.

## 10. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Domain semantic API becomes a maintenance burden as signals grow | Acceptable: better than 14 sites writing free-form metric names. If it grows past ~30 functions per domain, revisit. |
| `metrics` facade and `tracing` compete for attention; engineers forget which to use | Convention: metrics for "rate/count/distribution over time", tracing for "what happened in this one execution". Document in `signals.md`. |
| OTLP layer install order interacts badly with other tracing layers | Install OTLP layer first when present, then JSON fmt layer. Validated by section 8 step 5. |
| Cardinality explosion via unbounded label values (e.g., free-form `prompt_key`) | All label values come from typed enums or controlled vocabulary; raw user strings never reach a label slot. Snapshot tests catch new label additions. |
| `signals.md` and code drift despite CI check | The CI check compares names; if labels drift it won't catch it. Accept this as a known gap; revisit if it bites in practice. |

## 11. Out-of-Scope Future Work

These are deliberately deferred and called out so they are not silently
forgotten:

- Grafana dashboards and alerting rules
- Tail-based trace sampling
- Profiling integration (`tokio-console`, `pprof`)
- Cost/billing data pipeline derived from `llm_tokens_total`
- SLO definitions and burn-rate alerts

These will be addressed after sub-project B (persistence equivalence and
orchestration stress testing) demonstrates which signals operators
actually rely on.
