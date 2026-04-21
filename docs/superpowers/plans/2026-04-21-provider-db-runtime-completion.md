# Provider / DB Runtime Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current market-data skeleton into a runnable end-to-end path that can fetch real provider data, persist canonical K-lines in PostgreSQL, aggregate bars, and expose operator and market read APIs for full-flow testing.

**Architecture:** Keep provider-specific HTTP transport inside `pa-market::provider::providers`, move market persistence and read logic behind repository traits with PostgreSQL implementations, and wire a runtime `MarketRuntime` into `pa-api`/`pa-app` so admin backfill and market display endpoints use the same production path. Keep analysis orchestration isolated; this phase focuses on the provider -> canonical market data -> read API path needed for real-system verification.

**Tech Stack:** Rust 2024, Tokio, Axum, SQLx + PostgreSQL, Reqwest, Serde, Chrono, UUID, Rust Decimal, Tracing.

---

## File Structure Map

- `E:\rust-app\oh-paa\Cargo.toml`
  - Add any missing workspace features needed for SQLx migrations/macros and decimal mapping.
- `E:\rust-app\oh-paa\crates\pa-market\src\provider.rs`
  - Extend provider wiring to support live HTTP-backed implementations.
- `E:\rust-app\oh-paa\crates\pa-market\src\providers\eastmoney.rs`
  - Replace unwired placeholder methods with real HTTP transport.
- `E:\rust-app\oh-paa\crates\pa-market\src\providers\twelvedata.rs`
  - Replace unwired placeholder methods with real HTTP transport using API key auth.
- `E:\rust-app\oh-paa\crates\pa-market\src\repository.rs`
  - Add PostgreSQL-backed canonical K-line storage and read/query APIs.
- `E:\rust-app\oh-paa\crates\pa-market\src\service.rs`
  - Add read/aggregation services in addition to backfill.
- `E:\rust-app\oh-paa\crates\pa-instrument\src\repository.rs`
  - Add market/instrument/binding/policy queries needed by runtime APIs.
- `E:\rust-app\oh-paa\crates\pa-api\src\admin.rs`
  - Add operator endpoints for backfill and runtime checks.
- `E:\rust-app\oh-paa\crates\pa-api\src\market.rs`
  - Add canonical K-line and aggregated K-line read endpoints.
- `E:\rust-app\oh-paa\crates\pa-api\src\router.rs`
  - Add production app state for market runtime dependencies.
- `E:\rust-app\oh-paa\crates\pa-app\src\main.rs`
  - Create database pool, run migrations, register providers, and build shared app state.
- `E:\rust-app\oh-paa\crates\pa-api\tests\smoke.rs`
  - Replace placeholder assertions for `/admin` and `/market` with real route behavior checks.
- `E:\rust-app\oh-paa\crates\pa-market\tests\provider_http.rs`
  - Add transport-level tests with mock HTTP servers or controlled local endpoints.
- `E:\rust-app\oh-paa\crates\pa-market\tests\pg_repository.rs`
  - Add PostgreSQL-backed persistence/read tests.
- `E:\rust-app\oh-paa\docs\architecture\provider-db-e2e-test-plan.md`
  - Keep execution checklist aligned with runtime implementation.

## Task 1: Wire Real Provider HTTP Transport

**Files:**
- Modify: `E:\rust-app\oh-paa\Cargo.toml`
- Modify: `E:\rust-app\oh-paa\crates\pa-market\src\provider.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-market\src\providers\eastmoney.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-market\src\providers\twelvedata.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-market\tests\provider_http.rs`

- [ ] Add failing integration tests for live-transport provider methods using controlled HTTP fixtures.
- [ ] Run the new provider transport tests to verify they fail because provider methods still return unwired errors.
- [ ] Implement configurable HTTP-backed `EastMoneyProvider` and `TwelveDataProvider`.
- [ ] Add provider healthcheck behavior that distinguishes transport failures from parse/config failures.
- [ ] Run `cargo test -p pa-market --test provider_http`.
- [ ] Commit with message: `feat: wire live eastmoney and twelvedata transport`

## Task 2: Add PostgreSQL Canonical K-line Persistence and Reads

**Files:**
- Modify: `E:\rust-app\oh-paa\Cargo.toml`
- Modify: `E:\rust-app\oh-paa\crates\pa-market\src\repository.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-market\src\service.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-market\tests\pg_repository.rs`

- [ ] Add failing repository tests for PostgreSQL upsert and canonical read behavior.
- [ ] Run the new repository tests to verify they fail because only the in-memory repository exists.
- [ ] Implement `PgCanonicalKlineRepository` with idempotent upsert semantics.
- [ ] Add read methods for canonical K-lines by instrument, timeframe, and time window.
- [ ] Run `cargo test -p pa-market --test pg_repository`.
- [ ] Commit with message: `feat: add postgres canonical kline repository`

## Task 3: Add Aggregation and Gap-Aware Market Reads

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-market\src\service.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-market\src\repository.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-market\tests\aggregation.rs`

- [ ] Add failing tests for `15m -> 1h` aggregation, including a missing-child-bar case.
- [ ] Run the aggregation tests to verify they fail before implementation.
- [ ] Implement aggregation service logic over canonical child bars.
- [ ] Make aggregation explicitly fail or mark incomplete when required child bars are missing.
- [ ] Run `cargo test -p pa-market --test aggregation`.
- [ ] Commit with message: `feat: add gap-aware kline aggregation`

## Task 4: Add Instrument / Binding / Policy Queries Needed by Runtime

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-instrument\src\repository.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-instrument\src\service.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-instrument\tests\runtime_queries.rs`

- [ ] Add failing tests for loading instruments, provider symbol bindings, and resolved provider policy by `instrument_id`.
- [ ] Run the new instrument runtime-query tests to verify they fail.
- [ ] Implement PostgreSQL-backed runtime query methods needed by backfill and market read APIs.
- [ ] Run `cargo test -p pa-instrument --test runtime_queries`.
- [ ] Commit with message: `feat: add runtime instrument and provider lookups`

## Task 5: Replace Placeholder Admin and Market APIs

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\admin.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\market.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\router.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\lib.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-api\tests\smoke.rs`

- [ ] Add failing smoke tests for:
- [ ] `POST /admin/market/backfill`
- [ ] `GET /market/canonical`
- [ ] `GET /market/aggregated`
- [ ] Run `cargo test -p pa-api --test smoke` to verify current placeholder behavior fails the new expectations.
- [ ] Implement admin backfill endpoint and market read endpoints against the real runtime services.
- [ ] Update shared app state to carry market runtime dependencies.
- [ ] Run `cargo test -p pa-api --test smoke`.
- [ ] Commit with message: `feat: add admin backfill and market data endpoints`

## Task 6: Wire Production Runtime in `pa-app`

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-app\src\main.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-core\src\config.rs`
- Modify: `E:\rust-app\oh-paa\docs\architecture\phase1-runtime.md`

- [ ] Add a failing runtime-oriented compile or smoke test expectation for database-backed app construction if needed.
- [ ] Replace the in-memory-only app boot path with:
- [ ] configuration loading
- [ ] PostgreSQL pool creation
- [ ] SQLx migration execution
- [ ] provider registration
- [ ] production app state construction
- [ ] Keep analysis orchestration compiling while making the market-data path production-backed.
- [ ] Run `cargo check --workspace`.
- [ ] Commit with message: `feat: wire database and provider runtime`

## Task 7: Execute the Full Verification Path

**Files:**
- Modify: `E:\rust-app\oh-paa\docs\architecture\provider-db-e2e-test-plan.md`

- [ ] Update the E2E checklist to reflect the final endpoint names and runtime commands.
- [ ] Run `cargo fmt --all`.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Run `cargo test --workspace`.
- [ ] Document the exact operator inputs still needed for live verification:
- [ ] TwelveData API key
- [ ] PostgreSQL connection string
- [ ] one configured instrument and symbol binding per live-provider path
- [ ] Commit with message: `docs: finalize provider and db e2e verification plan`

## Verification Checklist

Run these commands before claiming the implementation is ready for live end-to-end execution:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected:

- formatting succeeds
- clippy reports no warnings
- all workspace tests pass

## Spec Coverage Self-Review

- live provider fetch path: covered by Task 1
- PostgreSQL canonical storage: covered by Task 2
- K-line aggregation: covered by Task 3
- instrument/provider binding runtime lookup: covered by Task 4
- operator backfill + display-facing market APIs: covered by Task 5
- production runtime wiring: covered by Task 6
- end-to-end execution checklist: covered by Task 7

## Placeholder Scan

- no `TODO`, `TBD`, or implied follow-up placeholders remain in task steps
- each task names exact files and verification commands

## Type Consistency Review

- provider transport remains in `pa-market`
- persistence and read services remain behind repository traits
- runtime app state is extended rather than split into unrelated top-level routers
- admin and market APIs both consume the same market runtime dependencies
