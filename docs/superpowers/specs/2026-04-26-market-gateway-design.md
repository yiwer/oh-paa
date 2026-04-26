Date: 2026-04-26
Project: `oh-paa`
Status: Drafted for review

# Market Gateway Design

## 1. Overview

`pa-instrument` already models the market dimension fully in the database:
`Market`, `Instrument`, `InstrumentSymbolBinding`, and `ProviderPolicy` (with
market-or-instrument scope). It also exposes a resolver,
`InstrumentRepository::resolve_market_data_context`, returning a
`InstrumentMarketDataContext { market, instrument, policy, bindings }`.

However, `pa-market::service` does not consume any of this. Its workflow
functions (`backfill_canonical_klines`, `derive_open_bar`,
`aggregate_canonical_klines`) take many individually-resolved string
arguments — `BackfillCanonicalKlinesRequest` alone carries 9 fields
including `primary_provider`, `fallback_provider`,
`primary_provider_symbol`, `fallback_provider_symbol`, `market_code`,
`market_timezone`. The callers (`pa-api/admin.rs`, `pa-api/market.rs`,
`pa-api/analysis_runtime.rs`) call `resolve_market_data_context` to get a
fully-populated context and then **manually unpack it back into 4–8 raw
strings** before calling into `pa-market`. The DB-level abstraction is
defeated at the function boundary.

This sub-project introduces `pa_market::MarketGateway`: a thin layer that
takes `&InstrumentMarketDataContext` plus an operation, resolves the right
provider and the right `provider_symbol` from the context internally, and
delegates to the existing `ProviderRouter`. Workflow functions are updated
to accept the context directly. The DB models stop being decorative and
start being load-bearing.

This is sub-project B in the broader refactor sequence. It must land
**before** sub-project A (observability foundation), so that A's metric
catalog can include a `market` label from day one rather than being added
in a follow-up.

## 2. Goals

- Eliminate string-shaped routing arguments (`primary_provider`, etc.)
  from `pa-market::service` workflow signatures
- Introduce `MarketGateway` as the single resolution point for
  "(context, operation) → (provider, provider_symbol, fetch)"
- Make `&InstrumentMarketDataContext` the canonical input shape for any
  market operation
- Keep behavior unchanged: no business-logic changes, all existing tests
  pass with only signature-level updates
- Provide the structural prerequisite for sub-project A to attach
  `market` labels to observability signals

## 3. Non-Goals

- No changes to the `MarketDataProvider` trait
- No changes to `ProviderRouter`'s fallback or fetch logic
- No changes to `ProviderPolicy` schema or fallback semantics
- No introduction of a context cache (per-request resolution is fine)
- No `MarketGateway` trait — single struct, single implementation, YAGNI
- No new routing strategies (cost-based, voting, etc.)
- No observability instrumentation in this sub-project — that is sub-project
  A's responsibility; B only ensures A has access to `&InstrumentMarketDataContext`
  at every relevant call site
- No reclassification of validation/storage error types

## 4. Fixed Constraints

- `pa-market` gains a new dependency on `pa-instrument` (types only — no
  repository or sqlx surface). Direction: `pa-market → pa-instrument`,
  acyclic.
- `MarketGateway` is a struct, not a trait.
- `MarketSessionProfile::from_market(code, tz)` stays — used by tests and
  legacy entry points without a `Market` record.
- All 204 existing tests must pass with only signature-level updates.
- This work happens in a single PR (single sub-project, ~21 call sites
  total).

## 5. Architecture

### 5.1 Crate Layering

```
pa-instrument   (Market / Instrument / Policy / Binding models + DB)
       ↑
   pa-market    (gateway + provider router + workflow)
       ↑
   pa-api / pa-app
```

The new edge `pa-market → pa-instrument` is a one-way type dependency. No
runtime path is added (`pa-instrument`'s repository/sqlx surface is not
imported into `pa-market`).

### 5.2 New Module: `pa-market/src/gateway.rs`

```rust
use pa_instrument::InstrumentMarketDataContext;

pub struct MarketGateway {
    router: ProviderRouter,
}

impl MarketGateway {
    pub fn new(router: ProviderRouter) -> Self;
    pub fn router(&self) -> &ProviderRouter; // transitional; reassess after migration

    pub async fn fetch_klines(
        &self,
        ctx: &InstrumentMarketDataContext,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<RoutedKlines, AppError>;

    pub async fn fetch_klines_window(
        &self,
        ctx: &InstrumentMarketDataContext,
        timeframe: Timeframe,
        start_open_time: Option<DateTime<Utc>>,
        end_close_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<RoutedKlines, AppError>;

    pub async fn fetch_latest_tick(
        &self,
        ctx: &InstrumentMarketDataContext,
    ) -> Result<RoutedTick, AppError>;
}
```

Internal resolution per method:

1. Read `ctx.policy.kline_primary` / `kline_fallback` (or `tick_primary`
   / `tick_fallback` for tick operations) to get provider names.
2. Call `ctx.binding_for_provider(name)` to get each `provider_symbol`.
3. Delegate to the existing `ProviderRouter::fetch_*_with_fallback_source`
   methods.
4. Return the raw `RoutedKlines` / `RoutedTick`.

When `policy.kline_fallback` (or `tick_fallback`) is `None`, the gateway
calls only the primary; primary failure surfaces directly without a
fallback attempt. This requires a single-provider dispatch path. The
chosen mechanism: promote the existing private
`ProviderRouter::fetch_klines_from` and `fetch_latest_tick_from` methods
to `pub(crate)`, and have the gateway pick between the single-provider
and fallback variants based on whether `policy.*_fallback` is `Some`.
Public `with_fallback_source` methods on `ProviderRouter` remain
unchanged.

### 5.3 Error Semantics

- **Missing binding** for a provider named in policy →
  `AppError::Validation` (sourced from `binding_for_provider`'s existing
  behavior).
- **Provider not registered** in router → `AppError::Validation`
  (sourced from existing `ProviderRouter::fetch_*_from`).
- **Policy resolution failure** is upstream of the gateway and is the
  responsibility of `InstrumentRepository::resolve_market_data_context`;
  the gateway never sees a malformed context.
- All other errors propagate unchanged from `ProviderRouter` and the
  underlying `MarketDataProvider`.

### 5.4 Workflow Function Signature Changes

`pa-market/src/service.rs`:

**`backfill_canonical_klines`**
```rust
pub async fn backfill_canonical_klines(
    gateway: &MarketGateway,
    repository: &dyn CanonicalKlineRepository,
    ctx: &InstrumentMarketDataContext,
    timeframe: Timeframe,
    limit: usize,
) -> Result<(), AppError>;
```
`BackfillCanonicalKlinesRequest` is removed. Internal session profile is
derived from `ctx.market` via `MarketSessionProfile::from_market_record`.

**`derive_open_bar`**
```rust
pub async fn derive_open_bar(
    gateway: &MarketGateway,
    repository: &dyn CanonicalKlineRepository,
    ctx: &InstrumentMarketDataContext,
    timeframe: Timeframe,
) -> Result<Option<DerivedOpenBar>, AppError>;
```
`DeriveOpenBarRequest` is removed.

**`aggregate_canonical_klines`**
```rust
pub async fn aggregate_canonical_klines(
    repository: &dyn CanonicalKlineRepository,
    ctx: &InstrumentMarketDataContext,
    request: AggregateCanonicalKlinesRequest,
) -> Result<Vec<AggregatedKline>, AppError>;
```
`AggregateCanonicalKlinesRequest` is shrunk: `instrument_id`,
`market_code`, `market_timezone` are removed; `source_timeframe`,
`target_timeframe`, `start_open_time`, `end_open_time`, `limit` remain.
This function does not take a gateway (no provider involvement).

**`aggregate_replay_window_rows`** (free helper)
```rust
pub fn aggregate_replay_window_rows(
    rows: &[CanonicalKlineRow],
    ctx: &InstrumentMarketDataContext,
    source_timeframe: Timeframe,
    target_timeframe: Timeframe,
) -> Result<Vec<AggregatedKline>, AppError>;
```
`market_code` / `market_timezone` string params are dropped. `instrument_id`
is now `ctx.instrument.id`.

**`list_canonical_klines`**
Unchanged. Pure repository query with no market or provider concern.

### 5.5 Session Profile Convenience Constructor

In `pa-market/src/session.rs`:
```rust
impl MarketSessionProfile {
    pub fn from_market_record(market: &pa_instrument::Market) -> Self;
}
```
Coexists with the existing `from_market(code, tz)`. The latter is **not**
removed — it is still used by tests and the rare entry points that have
no `Market` record (e.g., `replay_window_rows` invoked with synthetic
fixtures).

### 5.6 Test Fixture Helper

In `pa-instrument/src/repository.rs` (or a new `fixtures.rs`):
```rust
#[cfg(any(test, feature = "test-fixtures"))]
impl InstrumentMarketDataContext {
    pub fn fixture(/* minimal builder API */) -> Self;
}
```
Provides a hand-rolled `InstrumentMarketDataContext` for `pa-market` tests
that need a context but cannot reach a database. Exact builder shape is an
implementation detail decided during the implementation plan.

## 6. Migration Plan

Single PR. File-by-file:

| File | Change |
|---|---|
| `pa-market/Cargo.toml` | Add `pa-instrument = { path = "../pa-instrument" }` |
| `pa-market/src/gateway.rs` | New module with `MarketGateway` |
| `pa-market/src/lib.rs` | Re-export `MarketGateway`; adjust `pub use` for removed/renamed types |
| `pa-market/src/session.rs` | Add `from_market_record` |
| `pa-market/src/service.rs` | 4 functions reshape; remove 2 request structs; shrink 1 |
| `pa-instrument/src/repository.rs` | Add `InstrumentMarketDataContext::fixture` for test reuse |
| `pa-market/tests/backfill_idempotent.rs` | 4 call updates; build context fixtures |
| `pa-market/tests/open_bar_runtime.rs` | 2 call updates |
| `pa-market/tests/session_aggregation.rs` | 3 call updates |
| `pa-market/tests/gateway.rs` | New file: 5 unit tests (see §7.1) |
| `pa-market/tests/provider_router.rs` | Unchanged (tests `ProviderRouter` directly) |
| `pa-api/src/admin.rs` | 1 site: pass `&ctx` instead of unpacked strings |
| `pa-api/src/market.rs` | 4 sites |
| `pa-api/src/analysis_runtime.rs` | 6 sites |
| `pa-app/src/main.rs` | Wrap `ProviderRouter` in `MarketGateway` at startup; thread the gateway to handlers |

Approximate diff: ~11 production call sites (1 in `admin.rs` + 4 in
`market.rs` + 6 in `analysis_runtime.rs`) + ~9 test call sites.

## 7. Testing Strategy

### 7.1 Gateway Unit Tests (new file `pa-market/tests/gateway.rs`)

Use a fake `MarketDataProvider` injected into `ProviderRouter`. Construct
`InstrumentMarketDataContext` by hand using the new fixture helper.

Five cases:
1. Primary provider returns data → result `provider_name == primary`
2. Primary returns empty → fallback hits → result `provider_name == fallback`
3. Policy names a provider that is not registered in the router → `Validation`
4. Policy names a provider that has no `InstrumentSymbolBinding` →
   `Validation`
5. Policy with no `kline_fallback` + primary fails → error returned, no
   silent degradation

### 7.2 Existing Workflow Integration Tests

`backfill_idempotent.rs`, `open_bar_runtime.rs`, `session_aggregation.rs`:
update call sites to the new signature. **No new behavioral assertions**
— behavior is unchanged.

### 7.3 What Is Not Tested

- Network behavior of real providers (existing `provider_http.rs`
  coverage is sufficient)
- Performance characteristics of the resolution layer (single in-memory
  hash lookup; not worth measuring)

## 8. Definition of Done

1. `cargo test` passes — all 204 existing tests plus the 5 new gateway tests
2. `pa-app` starts and existing HTTP behavior is unchanged
   (`pa-api/tests/smoke.rs` and `pa-app/tests/live_replay.rs` pass)
3. No `*Request` struct in `pa-market::service` contains
   `primary_provider`, `fallback_provider`, `primary_provider_symbol`,
   `fallback_provider_symbol`, `market_code`, or `market_timezone` fields
4. `rg "primary_provider_symbol|fallback_provider_symbol" crates/` in
   non-fixture business code returns no results
5. The observability foundation spec
   (`docs/superpowers/specs/2026-04-26-observability-foundation-design.md`)
   has been updated in this same PR to add a `market` label to:
   `market_provider_requests_total`, `market_provider_duration_seconds`,
   `orchestration_tasks_total`, `orchestration_task_duration_seconds`,
   `llm_request_duration_seconds`. Label semantics: source from
   `ctx.market.code` where available; `unknown` otherwise — documented
   in the catalog text.

## 9. Impact on Sub-Project A (Observability Foundation)

This sub-project lands **before** sub-project A. The only artifact A's
spec needs is a catalog table edit (item 5 in §8). The observability
crate, instrumentation calls, and CI catalog check do not need to be
aware of `MarketGateway` directly — they just receive `&InstrumentMarketDataContext`
at every relevant call site, which this sub-project guarantees.

The `unknown` fallback for `market` label values applies to instrumentation
points that genuinely have no instrument context (LLM calls dispatched
from contexts without an instrument, infrastructure metrics, etc.). This
is an explicit, documented escape hatch — not a workaround for missing
plumbing.

## 10. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| `pa-market → pa-instrument` dependency tightens crate coupling | Types only, no repo/sqlx import. If reversal is ever needed, lift `MarketDataContext` into `pa-core`. |
| `binding_for_provider` returning `Validation` may misclassify config errors | Keep current semantics. Error model rework is out of scope and would touch policy/binding/provider together. |
| Callers can still bypass `MarketGateway` via `MarketGateway::router()` accessor | Acceptable during migration. After all sites migrated, a follow-up PR can make `router()` private. |
| `aggregate_canonical_klines` takes `ctx` but only reads `ctx.market` | Acceptable. Consistent input shape beats a one-field shortcut; revisit only if `ctx` proves heavyweight. |
| Hand-built `InstrumentMarketDataContext` fixtures in tests are tedious | Add `InstrumentMarketDataContext::fixture` builder in `pa-instrument`. |
| The `market` label fallback `unknown` could mask gaps in instrumentation | The CI catalog check (sub-project A) will catch newly added metrics that lack the `market` label; the value being `unknown` is operationally visible and flagged in the runbook. |

## 11. Out-of-Scope Future Work

- Making `MarketGateway::router()` private (follow-up PR after migration)
- Caching `InstrumentMarketDataContext` resolution within a request
- Cost-based or voting provider routing
- Reclassifying provider/policy/binding error types
- Promoting `MarketGateway` to a trait (only if a second implementation
  appears)
