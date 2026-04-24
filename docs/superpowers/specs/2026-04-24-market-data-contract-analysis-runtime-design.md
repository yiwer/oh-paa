Date: 2026-04-24
Project: `oh-paa`
Status: Drafted for review

# Market Data Contract For Analysis Runtime Design

## 1. Overview

`oh-paa` already has a mostly working market-data path:

- provider fetch through `eastmoney` and `twelvedata`
- canonical closed-bar persistence in PostgreSQL
- higher-timeframe aggregation from canonical bars
- open-bar derivation from closed bars plus latest tick
- market APIs for canonical, aggregated, tick, and open-bar views
- analysis input assembly that consumes market data in `analysis_runtime`

The current weakness is not that the market path is missing. The weakness is that the contract
between market data and `analysis_runtime` is still implicit.

Today:

- closed-bar inputs are trustworthy because they come from canonical persistence or its aggregates
- open-bar inputs are derived correctly, but their semantics are not formalized as a contract
- latest tick is still a provider snapshot rather than a persisted truth layer
- `analysis_runtime` still carries too much responsibility for interpreting which market view is
  valid in each situation

This design defines a contract-first boundary between market data and `analysis_runtime`. It does
not introduce a new tick persistence model in this phase. Instead, it makes the current system
explicit:

- canonical data is the closed-bar truth layer
- aggregated data is a derived closed-bar truth view when complete
- open-bar is a derived analysis input
- tick is a background snapshot, not an analysis truth source

The result should be a stable market-data contract that can support both automated regression and a
real-environment operator flow without changing market semantics again later.

## 2. Goals

This phase exists to make the market-data path to `analysis_runtime` explicit, stable, and
verifiable.

The design goals are:

- define a small set of approved market contract objects consumed by `analysis_runtime`
- enforce that closed-bar analysis inputs come only from canonical persistence or complete
  aggregation results
- enforce that open-bar analysis inputs come only from explicit derived open-bar views
- limit latest tick usage to background and observability contexts
- define clear error and downgrade behavior for missing or unavailable market views
- make the contract testable in local automation and provable in a real PostgreSQL plus real
  provider runtime flow

This phase must end with:

- one documented contract boundary between market data and `analysis_runtime`
- one narrow adapter layer that materializes contract objects for analysis input assembly
- automated verification for market contracts and analysis input assembly rules
- one operator runbook that validates the same contract on the real runtime path

## 3. Non-Goals

This phase does not include:

- durable tick persistence
- a new tick subscription system
- replacing in-memory orchestration storage
- expanding the scope to full manual user analysis end-to-end
- redesigning provider implementations
- redesigning prompt schemas
- redesigning replay scoring
- making aggregation session-calendar-aware beyond current `complete=false` safety behavior

If future work adds tick persistence, it should reuse the same contract semantics rather than force a
new analysis-side interface.

## 4. Fixed Constraints

The following constraints are fixed for this phase:

- canonical closed bars remain the primary market truth layer
- only closed bars are persisted as canonical truth in PostgreSQL
- incomplete aggregated bars may be queryable but are not valid closed-bar analysis inputs
- derived open bars are valid analysis inputs only when explicitly marked as derived
- latest tick remains a snapshot view in this phase and must not be treated as persisted truth
- provider raw payloads must not bypass the contract layer and enter analysis inputs directly
- market background data may degrade for tick and open-bar views, but closed-bar analysis inputs may
  not silently degrade

## 5. Recommended Approach

### 5.1 Selected strategy

Use a contract-first design:

1. keep existing provider, persistence, aggregation, and open-bar mechanisms largely intact
2. define a small approved set of market contract objects
3. introduce a narrow market contract adapter between market services and `analysis_runtime`
4. make `analysis_runtime` consume only contract objects rather than mixed market semantics
5. verify the contract through both automated regression and a real-runtime operator flow

This is preferred over a test-first-only approach because the core problem is semantic drift, not
just missing tests. It is also preferred over immediate tick persistence work because the current
runtime can already be stabilized without expanding the data model.

### 5.2 Rejected strategies

Do not start by adding more integration tests without first fixing the contract boundary. That would
lock current ambiguity into test cases.

Do not make tick a first-class persisted truth layer in this phase. That would enlarge scope before
the analysis-side contract is stable.

Do not let `analysis_runtime` continue acting as an implicit market semantics interpreter. That keeps
the boundary too weak to verify cleanly.

## 6. High-Level Architecture

The design keeps the existing crate layout and inserts one explicit contract boundary.

### 6.1 Provider acquisition layer

Current home:

- `crates/pa-market/src/provider.rs`
- `crates/pa-market/src/providers/*`

Responsibilities:

- fetch provider K-lines
- fetch latest provider tick
- attach provider identity

This layer does not define analysis-valid market semantics.

### 6.2 Market truth layer

Current home:

- `crates/pa-market/src/repository.rs`
- `crates/pa-market/src/service.rs`

Responsibilities:

- backfill canonical closed bars
- read canonical bars
- aggregate canonical bars into higher timeframes

This remains the closed-bar truth path.

### 6.3 Market derived-views layer

Current home:

- `crates/pa-market/src/service.rs`

Responsibilities:

- derive open bars from canonical closed bars plus latest tick
- expose latest tick as a snapshot view

This layer may produce analysis-usable views, but they are explicitly derived or snapshot-only.

### 6.4 Market contract adapter

Recommended home:

- a thin adapter near `crates/pa-api/src/analysis_runtime.rs`
- or a dedicated file such as `crates/pa-api/src/market_contract.rs`

Responsibilities:

- materialize approved market contract objects
- normalize field names and object kinds
- enforce which market views are allowed for which analysis scenario
- convert lower-level market failures into contract-level errors

This is the main new boundary introduced by the design.

### 6.5 Analysis input assembly

Current home:

- `crates/pa-api/src/analysis_runtime.rs`

Responsibilities after this design:

- request market contract objects
- select the right contract object based on `timeframe` and `bar_state`
- assemble shared-analysis input and market background from those contracts

It should no longer decide market validity rules ad hoc.

## 7. Approved Contract Objects

Only four market contract objects are approved for this phase.

### 7.1 `canonical_closed_bar`

Purpose:

- closed `15m` truth input for shared analysis

Required fields:

- `kind = canonical_closed_bar`
- `instrument_id`
- `timeframe`
- `open_time`
- `close_time`
- `open`
- `high`
- `low`
- `close`
- `volume`
- `source_provider`

Rules:

- source must be `canonical_klines`
- bar must already be closed
- this is a first-class analysis truth object

### 7.2 `aggregated_closed_bar`

Purpose:

- closed higher-timeframe truth input for shared analysis

Required fields:

- `kind = aggregated_closed_bar`
- `instrument_id`
- `source_timeframe`
- `timeframe`
- `open_time`
- `close_time`
- `open`
- `high`
- `low`
- `close`
- `volume`
- `child_bar_count`
- `expected_child_bar_count`
- `complete`
- `source_provider`

Rules:

- source must be canonical aggregation
- only `complete = true` rows are valid closed-bar analysis inputs
- `complete = false` rows may remain queryable for operators and APIs but may not be promoted into
  closed-bar shared-analysis inputs

### 7.3 `derived_open_bar`

Purpose:

- open-bar analysis input

Required fields:

- `kind = derived_open_bar`
- `instrument_id`
- `source_timeframe`
- `timeframe`
- `open_time`
- `close_time`
- `latest_tick_time`
- `open`
- `high`
- `low`
- `close`
- `child_bar_count`
- `source_provider`

Rules:

- source must be canonical closed bars plus latest tick
- this object must remain explicitly derived
- it is valid for open-bar analysis input, but it is not a persisted truth row

### 7.4 `latest_tick_snapshot`

Purpose:

- market background and observability context only

Required fields:

- `kind = latest_tick_snapshot`
- `instrument_id`
- `tick_time`
- `price`
- `size`
- `source_provider`
- `market_open`

Rules:

- source is the latest provider tick path in this phase
- this object is not a bar substitute
- it may appear in `market_background_json` or market-facing APIs, but it is not a valid primary
  analysis bar input

## 8. Consumption Rules

The analysis-side contract rules are strict.

### 8.1 Closed-bar analysis

Closed-bar shared analysis may consume only:

- `canonical_closed_bar` for `15m`
- `aggregated_closed_bar` with `complete = true` for `1h` and `1d`

Closed-bar analysis must never consume:

- provider raw K-line payloads
- incomplete aggregated bars
- latest tick snapshots
- derived open bars

### 8.2 Open-bar analysis

Open-bar shared analysis may consume only:

- `derived_open_bar`

Open-bar analysis must never consume:

- a synthetic bar assembled directly inside `analysis_runtime`
- raw provider payloads
- latest tick alone as a surrogate bar

### 8.3 Market background

`market_background_json` may include:

- `latest_tick_snapshot`
- currently derived open-bar views for `15m`, `1h`, and `1d`
- market metadata such as market code, timezone, and session kind

These background attachments are informative. They do not replace the primary bar input selected for
analysis.

## 9. Error Semantics

Contract errors must be explicit and grouped by meaning.

### 9.1 `contract_missing`

Definition:

- the requested contract object does not exist in the truth path

Examples:

- missing canonical closed bar for a requested `15m` input
- missing complete aggregated bar for a requested `1h` or `1d` closed-bar input

Handling:

- closed-bar analysis fails immediately
- the error must identify contract kind, instrument, timeframe, and requested bar boundary

### 9.2 `contract_unavailable`

Definition:

- the contract could not be materialized because a dependency was temporarily unavailable

Examples:

- latest tick fetch failed
- open-bar derivation failed because the provider snapshot path failed

Handling:

- `latest_tick_snapshot` may be returned as `null`
- `derived_open_bar` may be returned as `null` in market background contexts
- the underlying cause remains diagnosable, but the upper layer sees a contract-level availability
  failure

### 9.3 `contract_invalid`

Definition:

- the requested or produced object violates contract rules

Examples:

- attempting to use `complete = false` aggregated data as a closed-bar analysis input
- producing an open-bar object with invalid time boundaries

Handling:

- reject immediately
- do not silently downgrade

### 9.4 `provider_failed`

Definition:

- provider fetch or parse failed and no valid fallback resolved the request

Handling:

- market APIs and runbooks should preserve enough provider-level evidence for operators
- `analysis_runtime` should still depend on the contract adapter and receive a contract-classified
  failure rather than provider-specific raw payload semantics

## 10. Data Flow

The contract introduces two approved analysis-side flows.

### 10.1 Closed-bar path

`provider -> normalize -> canonical_klines -> canonical or complete aggregation -> contract adapter -> analysis_runtime`

This is the only approved source for closed-bar analysis inputs.

### 10.2 Open-bar and background path

`provider tick -> latest_tick_snapshot -> open-bar derivation -> contract adapter -> analysis_runtime`

In this flow:

- `derived_open_bar` may serve as the primary bar input for open-bar analysis
- `latest_tick_snapshot` remains background-only

## 11. Verification Strategy

Verification is a first-class deliverable in this phase and must run in two tracks.

### 11.1 Automated regression track

The automated track should prove contract correctness without relying on external runtime
availability.

Required coverage:

- provider contract tests for request and parse behavior
- PostgreSQL repository tests for canonical persistence and retrieval
- market contract tests for all four approved contract objects
- analysis input assembly tests for `analysis_runtime`

The contract and assembly tests must prove at least:

- `15m` closed analysis resolves to `canonical_closed_bar`
- `1h` and `1d` closed analysis resolve only to `aggregated_closed_bar` with `complete = true`
- open analysis resolves only to `derived_open_bar`
- `latest_tick_snapshot` is background-only
- missing closed-bar truth objects fail explicitly
- tick or open-bar background attachments may degrade to `null` without corrupting closed-bar
  primary inputs

### 11.2 Real-environment runbook track

The real-runtime track should prove the same contract against:

- real PostgreSQL
- real instrument, binding, and provider-policy rows
- real provider HTTP

The runbook should validate this sequence:

1. seed market, instrument, binding, and provider-policy fixtures
2. run `/admin/market/backfill`
3. verify `/market/canonical`
4. verify `/market/aggregated`
5. verify `/market/tick`
6. verify `/market/open-bar`
7. run one shared-analysis input assembly path and confirm the contract objects used are the
   approved ones for that scenario

The runbook must classify failures into at least:

- provider
- normalization
- persistence
- aggregation
- open-bar derivation
- contract assembly

## 12. Success Criteria

This phase is successful only when:

- `analysis_runtime` consumes approved market contract objects instead of mixed implicit market
  semantics
- closed-bar analysis inputs are restricted to canonical truth or complete aggregation outputs
- open-bar analysis inputs are restricted to explicit derived open-bar views
- latest tick remains background-only in analysis semantics
- downgrade behavior is explicit for tick and background open-bar paths
- automated regression covers contract materialization and analysis input assembly
- a real PostgreSQL plus provider operator flow proves the same contract boundary in practice

## 13. Files Likely In Scope

Likely later implementation scope:

- `crates/pa-market/src/service.rs`
- `crates/pa-api/src/analysis_runtime.rs`
- `crates/pa-api/src/market.rs`
- `crates/pa-api/src/router.rs`
- `docs/architecture/provider-db-e2e-test-plan.md`
- `docs/architecture/phase1-runtime.md`
- relevant tests under `crates/pa-market/tests/`
- relevant tests near `crates/pa-api/src/analysis_runtime.rs`

This phase should prefer narrow adapter additions and targeted tests over broad crate refactors.

## 14. Risks

Primary risks:

- leaving contract logic half in `analysis_runtime` and half in the new adapter
- letting market background convenience fields become unofficial primary analysis inputs
- confusing snapshot availability problems with truth-layer missing-data problems
- drifting into tick-persistence design before the contract boundary is stable

Mitigations:

- keep only four approved contract object types
- make input eligibility rules explicit and testable
- preserve distinct error classes for missing, unavailable, invalid, and provider failures
- keep this phase scoped to the market-data-to-analysis-runtime boundary

## 15. Relationship To Existing Work

This design builds on the current runtime state described by:

- `docs/architecture/phase1-runtime.md`
- `docs/architecture/provider-db-e2e-test-plan.md`
- `docs/project-analysis-zh.md`

It does not replace the existing market runtime. It formalizes the boundary that already exists in
partially implicit form and turns it into a stable contract that later phases can extend.
