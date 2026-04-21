# Financial Price Action Analysis Server Design

Date: 2026-04-21
Project: `oh-paa`
Status: Approved for planning

## 1. Overview

This project is a ground-up refactor of the earlier `stock-everyday` service into a more compatible, extensible, and robust financial price action analysis server.

The old system was centered on the A-share market and had several structural limits:

- market abstraction was too narrow and effectively A-share specific
- provider abstraction was too thin for multi-market and multi-provider expansion
- K-line, latest price, and K-line aggregation semantics were not cleanly separated
- the LLM analysis flow did not match the desired split between public market analysis and user-specific analysis

The new system will prioritize the data foundation first. Phase 1 focuses on building a stable market-data and public-analysis substrate that later user-facing analysis can safely depend on.

## 2. Phase 1 Goals

Phase 1 goal:

Build a unified price action analysis substrate centered on `instrument_id` and canonical K-lines, supporting A-shares, crypto, and forex, with primary/fallback provider routing and reusable public analysis outputs.

Phase 1 includes:

- support for `A-share`, `crypto`, and `forex`
- provider implementations for `EastMoney` and `TwelveData`
- internal `instrument_id` as the only business primary key for tradable instruments
- system-normalized `canonical_kline` as the only business-truth K-line
- provider strategy of `primary provider + fallback provider`
- public analysis assets:
  - per-bar price action analysis
  - one shared daily market-context analysis per instrument
- system-wide time semantics based on `Asia/Shanghai`
- open-bar derivation in memory from latest tick plus the latest closed K-lines
- market-level default provider policies with instrument-level overrides
- minimal user-side integration through subscription, position maintenance, and manual user-triggered analysis

Phase 1 explicitly does not attempt to optimize every downstream analysis workflow. The priority is correctness and extensibility of the data and analysis substrate.

## 3. Core Design Principles

The design follows these principles:

- canonical first: business logic consumes normalized canonical data, not raw provider payloads
- isolate responsibilities: provider ingestion, canonicalization, public analysis, and user analysis are separate layers
- idempotent pipelines: K-line backfill and analysis triggering must be safely repeatable
- open-bar isolation: incomplete bars are derived views and must not pollute confirmed historical bars
- provider independence: providers supply data but do not define system truth
- staged complexity: Phase 1 intentionally keeps some semantics simple, especially around timezone and session handling

## 4. Domain Boundaries

The system is split into six bounded domains.

### 4.1 `instrument-registry`

Responsibilities:

- define markets and instruments
- maintain provider symbol bindings
- maintain provider routing policies
- serve as the authoritative mapping from external symbols to internal `instrument_id`

### 4.2 `market-data-spi`

Responsibilities:

- define provider capability interfaces
- standardize access for historical K-line fetches, latest tick queries, and health checks
- shield upstream providers from leaking directly into business logic

### 4.3 `market-data-core`

Responsibilities:

- call providers according to policy
- normalize upstream data into canonical tick and canonical K-line semantics
- write confirmed closed bars
- derive open bars from the latest tick and recent closed bars

### 4.4 `market-context`

Responsibilities:

- generate shared per-bar analysis
- generate one shared daily market-context analysis per instrument
- expose reusable public analysis assets to all users

### 4.5 `user-subscription`

Responsibilities:

- manage subscriptions
- manage user-entered position information
- manage user-side trigger configuration later in the roadmap

### 4.6 `user-analysis`

Responsibilities:

- combine public market analysis with user position context
- generate user-specific advice and reports
- avoid re-deriving shared market structure already produced in the public layer

## 5. Core Data Model

### 5.1 `market`

Purpose:

- represent A-share, crypto, and forex as top-level markets

Key fields:

- `market_code`
- `name`
- `timezone`
- `session_template`
- `default_provider_policy_ref`

Note:

Even though Phase 1 runs with unified `Asia/Shanghai` system time semantics, `market.timezone` and session metadata should still be persisted for future refinement.

### 5.2 `instrument`

Purpose:

- represent the internal business identity of a tradable symbol

Key fields:

- `instrument_id`
- `market_code`
- `display_name`
- `base_currency`
- `quote_currency`
- `trading_status`

Rule:

All business flows reference `instrument_id`. User subscriptions, holdings, public analysis, and user analysis must not key off provider symbols directly.

### 5.3 `instrument_symbol_binding`

Purpose:

- map one internal instrument to one or more provider symbols

Key fields:

- `instrument_id`
- `provider`
- `provider_symbol`
- `provider_exchange`
- `enabled`

This table allows the system to reconcile symbol-code differences across providers while keeping one internal identity.

### 5.4 `provider_policy`

Purpose:

- define routing rules for K-line and tick data

Key fields:

- `scope_type` (`market` or `instrument`)
- `scope_id`
- `kline_primary`
- `kline_fallback`
- `tick_primary`
- `tick_fallback`

Rule:

- most configuration lives at market scope
- exceptional instruments may override market defaults

### 5.5 `canonical_kline`

Purpose:

- store confirmed closed bars only

Key fields:

- `instrument_id`
- `timeframe`
- `bar_open_time`
- `bar_close_time`
- `open`
- `high`
- `low`
- `close`
- `volume`
- `source_provider`
- `ingested_at`

Rules:

- this is the single business-truth K-line table
- only closed bars are written
- writes must be idempotent

### 5.6 `live_tick_snapshot`

Purpose:

- store the latest known market tick or last price snapshot per instrument

Key fields:

- `instrument_id`
- `last_price`
- `last_size`
- `tick_time`
- `provider`
- `version`

This table supports latest-price queries and open-bar derivation.

### 5.7 `derived_open_bar_cache`

Purpose:

- store the current incomplete bar state as a derived runtime view

Key fields:

- `instrument_id`
- `timeframe`
- `bar_open_time`
- `open`
- `high`
- `low`
- `close`
- `last_tick_time`

Rules:

- this is a derived cache, not a historical truth table
- it may live in memory or a cache tier
- it must not be merged into `canonical_kline` before bar confirmation

### 5.8 `bar_analysis`

Purpose:

- store reusable shared analysis for one confirmed bar

Key fields:

- `instrument_id`
- `timeframe`
- `bar_close_time`
- `analysis_version`
- `input_snapshot_ref`
- `result_json`
- `created_at`

Key:

`instrument_id + timeframe + bar_close_time + analysis_version`

### 5.9 `daily_market_context`

Purpose:

- store one shared daily market-context analysis per instrument

Key fields:

- `instrument_id`
- `trading_date`
- `analysis_version`
- `context_json`
- `created_at`

### 5.10 User-side entities

Phase 1 keeps user-side entities intentionally simple:

- `user_subscription`
- `user_position_lot`
- `user_position_snapshot`
- `user_analysis_task`
- `user_analysis_report`

These entities consume public analysis assets instead of reconstructing market semantics from scratch.

## 6. Provider SPI

The provider layer must be capable enough for multi-market expansion without overfitting to one source.

Phase 1 provider capabilities:

- fetch historical K-lines for `15m`, `1h`, and `1d`
- fetch latest tick or latest price snapshot
- expose provider health status

Phase 1 provider routing strategy:

- each request uses the configured primary provider first
- when the primary provider fails or returns insufficient data, the fallback provider is attempted
- all fallback events must be logged and observable

Phase 1 does not include:

- provider voting or result arbitration
- dynamic quality scoring across multiple providers
- user-defined custom provider plugins

## 7. Time and Session Semantics

Phase 1 adopts one simplifying rule:

- system execution and canonical bar semantics are based on `Asia/Shanghai`

Reason:

- reduces operational and aggregation complexity in the first stage
- simplifies scheduling and bar-boundary handling across modules

Caveat:

- this is a deliberate phase-bound compromise, not the final long-term model
- market/session metadata must still be preserved so later versions can move toward per-market timezone/session semantics

## 8. Data and Analysis Flows

### 8.1 Historical K-line backfill flow

Flow:

`scheduler/admin trigger -> select instrument + timeframe -> resolve provider policy -> query primary provider -> fallback if needed -> normalize -> gap/dedup validation -> write canonical_kline`

Rules:

- only closed bars are persisted
- re-runs must be safe and idempotent
- provider switching must not alter upper-layer semantics

### 8.2 Latest tick ingestion flow

Flow:

`socket/poll/timer source -> provider query -> normalize -> update live_tick_snapshot -> update derived_open_bar_cache`

Rules:

- tick data expresses current state, not historical truth
- open-bar cache is derivative and mutable
- tick interruption must not corrupt confirmed history

### 8.3 Shared per-bar analysis flow

Flow:

`canonical_kline insert for closed bar -> enqueue public-analysis task -> assemble normalized context -> generate bar_analysis`

Rules:

- input data comes only from canonical layers
- one bar-analysis output is shared by all users
- failures are retryable without creating duplicates

### 8.4 Shared daily market-context flow

Flow:

`scheduled trigger -> collect recent 15m/1h/1d structures and relevant bar_analysis outputs -> generate one daily_market_context per instrument`

Rules:

- one daily shared output per instrument per trading date
- daily context should not block bar-by-bar analysis

### 8.5 User-specific analysis flow

Flow:

`manual user trigger or later scheduled trigger -> load bar_analysis + daily_market_context + user positions -> generate user analysis report`

Rules:

- user analysis must not call providers directly
- user analysis should reuse shared market judgment as much as possible

## 9. Administrative and Product Capabilities

### 9.1 Admin capabilities

Phase 1 admin functions:

- manage markets
- manage instruments
- manage provider symbol bindings
- configure market-level default provider policies
- override provider policy for selected instruments
- trigger historical backfill
- inspect latest tick freshness and latest closed-bar freshness
- inspect provider health and fallback activity
- re-run bar analysis or daily market-context analysis
- inspect task states and recent failures

### 9.2 Public data and analysis APIs

Recommended API groupings:

- `admin api`
- `market data api`
- `public analysis api`
- `user api`

Separation rule:

Public shared analysis APIs and user-specific analysis APIs should remain separate so public caching and reuse stay stable as user workflows evolve.

## 10. Error Handling and Resilience

### 10.1 Provider call resilience

Required:

- request timeouts
- bounded retries
- explicit fallback behavior
- error classification for:
  - no data
  - rate limit
  - network failure
  - malformed upstream payload

### 10.2 Canonicalization safeguards

Required:

- deduplication and idempotent writes
- validation of bar time boundaries
- validation of OHLC consistency
- graceful handling for incomplete volume when market/provider semantics differ

### 10.3 Task execution safeguards

Required task states:

- `pending`
- `running`
- `succeeded`
- `failed`
- `dead_letter`

Rules:

- duplicate tasks for the same logical key should be suppressible
- analysis failure must not block market-data ingestion

### 10.4 Observability

Required:

- trace or correlation IDs per task
- provider latency metrics
- provider failure and fallback counts
- K-line gap-fill counts
- latest successful closed-bar timestamps per instrument and timeframe

## 11. Testing Strategy

Phase 1 testing should focus on semantic correctness, not only endpoint coverage.

Required test categories:

- provider contract tests for `EastMoney` and `TwelveData`
- canonicalization tests for K-line normalization and invalid bar rejection
- idempotency tests for repeated backfill runs
- open-bar derivation tests using tick updates plus closed-bar context
- provider fallback tests
- task-trigger tests for:
  - closed-bar to bar-analysis
  - daily schedule to daily market-context
- user-analysis tests verifying public analysis reuse with position context

Recommended non-functional checks:

- fault-injection tests around provider failures
- replay tests for interrupted ingestion windows

## 12. Phase 1 Out of Scope

Phase 1 does not include:

- provider result arbitration or cross-provider quality scoring
- multi-timezone exact market-session modeling
- historical tick warehousing and replay
- additional timeframes beyond `15m`, `1h`, and `1d`
- broker syncing or advanced account automation
- multi-instrument portfolio interaction analysis
- visual prompt-management tooling
- a full generalized event-bus platform

These items may be planned later, but they should not distort the Phase 1 architecture.

## 13. Architecture Summary

Phase 1 should produce a system where:

- administrators define instruments and provider policies
- providers feed raw market data into a canonical data layer
- confirmed closed bars are stored as immutable business truth
- open bars exist only as derived runtime state
- shared public analysis is generated once and reused broadly
- user-specific analysis consumes public analysis plus private position context

In one sentence:

The Phase 1 system is a unified, provider-agnostic price action substrate for A-shares, crypto, and forex, designed around canonical K-lines, primary/fallback data routing, and a strict split between shared market analysis and user-specific analysis.
