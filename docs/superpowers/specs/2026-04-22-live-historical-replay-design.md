# Live Historical Replay Design

Date: 2026-04-22
Project: `oh-paa`
Status: Drafted for review

## 1. Overview

`oh-paa` already has:

- step-oriented prompt/model bindings
- OpenAI-compatible execution
- a layered shared-analysis chain
- offline deterministic replay with fixture-based outputs

What it does not yet have is a way to run real historical samples through the actual LLM pipeline using real provider data and real model credentials, then compare prompt and model quality from evidence.

This phase adds that missing layer.

The first slice is intentionally narrow:

- market: `crypto`
- timeframe: `15m`
- pipeline variant: `baseline_a`
- bar state: `closed` only

The goal is to establish one reliable, repeatable, real-execution evaluation loop before expanding to A-shares, forex, open bars, or multi-variant comparisons.

## 2. Goals

Build a first-class `live historical replay` capability that:

- reuses the existing step registry and OpenAI-compatible executor
- fetches or assembles real historical market context for selected samples
- executes the actual four-step analysis chain
- logs per-step outputs, metadata, and failures
- scores analysis quality using structural and consistency-oriented checks
- supports iterative prompt refinement based on observed failure patterns

This phase must prove that the team can move from:

- offline fixture replay

to:

- real historical replay with actual LLM outputs

without introducing a second orchestration system.

## 3. Non-Goals

This phase does not include:

- expanding immediately to A-shares and forex
- open-bar replay
- tick reconstruction or tick storage
- automated PA correctness judging against future price outcomes
- online prompt editing UI
- portfolio-level multi-instrument reasoning
- replacing the existing offline replay mode

The offline mode remains useful for deterministic regression checks. The new real mode is an additional execution path, not a replacement.

## 4. Confirmed Design Decisions

The following decisions are fixed for this slice:

- the first real replay slice is `crypto + 15m + baseline_a`
- data should come from real historical provider responses rather than pre-baked step outputs
- execution should reuse `Executor<OpenAiCompatibleClient>`
- the first loop should focus on `closed` bars only
- prompt optimization should be driven by evidence from replay reports
- scoring should prioritize schema stability, field completeness, and cross-step consistency before any deeper trading-outcome evaluation

## 5. Replay Modes

`pa_app::replay` should support two clearly separated execution modes.

### 5.1 Fixture Replay

This is the existing deterministic mode.

Its characteristics:

- reads dataset-defined step outputs
- validates them against the real registered schemas
- reports provider/model metadata from resolved bindings
- is fully deterministic and cheap

Its purpose is regression testing for:

- report shape
- schema contracts
- variant integrity
- scoring behavior

### 5.2 Live Historical Replay

This is the new mode.

Its characteristics:

- builds real step inputs from historical market data
- calls the configured real LLM providers
- validates actual returned outputs against the same step schemas
- records raw step outcomes and failure details

Its purpose is evaluation and prompt optimization for:

- actual structured-output stability
- actual PA field quality
- actual cross-step coherence

These two modes must remain explicit so reports are never confused with each other.

## 6. First-Slice Dataset Design

The first live dataset should contain a small, curated set of crypto `15m` bars.

Recommended starting size:

- 5 to 10 samples

Each sample should represent one target closed `15m` bar plus enough surrounding context to build the analysis chain.

Coverage should include at least:

- breakout continuation
- failed breakout
- range rejection
- compression before expansion
- sharp reversal or shock-follow-through

Each sample identity should include:

- symbol
- target timeframe
- target bar open time
- target bar close time
- trading date
- market code
- pipeline variant

The first preferred symbol is:

- `BTC/USD` or `BTC/USDT`

The exact symbol should be whichever symbol is already easiest to source consistently from the TwelveData integration used by `oh-paa`.

## 7. Data Sources and Configuration

### 7.1 Market Data Source

The first real replay slice should use:

- TwelveData historical K-lines

The sample builder should fetch enough historical bars to construct:

- the target `15m` bar
- recent `15m` bars
- aggregated `1h` structure
- aggregated `1d` structure

No tick data should be involved in this slice.

### 7.2 LLM Configuration Source

The system should support loading a real replay config that uses actual configured credentials rather than `config.example.toml`.

Available real config sources already present on disk:

- `E:\rust-app\pa-analyze-server\config.toml`
  - contains TwelveData and DashScope-compatible settings
- `E:\rust-app\stock-everyday\config.toml`
  - contains DeepSeek-compatible settings

The new replay flow must not copy secret values into repo files.

Instead, it should:

- read real config from an external path supplied at runtime
- map that config into `oh-paa`'s current step-oriented LLM config shape
- allow provider/model bindings for the replay run to differ from `config.example.toml`

## 8. Live Replay Execution Flow

The first-slice live replay flow should be:

1. load replay sample definitions
2. load real replay configuration from a user-supplied config path
3. resolve the step registry and provider runtimes from that config
4. fetch historical `15m` bars for the sample symbol and required time window
5. derive the target bar and surrounding context
6. derive higher-timeframe structure inputs from historical bars
7. build `shared_pa_state_bar_v1` input
8. execute `shared_pa_state_bar_v1`
9. build `shared_bar_analysis_v2` input from the live upstream result
10. execute `shared_bar_analysis_v2`
11. build `shared_daily_context_v2` input from live upstream results and structure context
12. execute `shared_daily_context_v2`
13. build a synthetic user-position fixture for the first slice
14. execute `user_position_advice_v2`
15. validate every step output
16. compute experiment scores
17. write a structured experiment report

This flow must preserve the current baseline dependency order:

- `shared_pa_state_bar_v1`
- `shared_bar_analysis_v2`
- `shared_daily_context_v2`
- `user_position_advice_v2`

That order should stay configurable by variant later, but only `baseline_a` is required in this slice.

## 9. Input Construction Rules

### 9.1 Shared PA State Input

The replay builder must assemble:

- target bar identity
- target bar OHLCV payload
- market session context
- relevant market context JSON

The bar must always be treated as:

- `closed`

### 9.2 Shared Bar Analysis Input

This step must consume:

- the real `shared_pa_state_json` returned by the previous step
- recent PA states from preceding bars in the replay context

It must not consume pre-baked output fixtures in live mode.

### 9.3 Shared Daily Context Input

This step must consume:

- recent PA states
- recent shared-bar analyses
- multi-timeframe structure context
- market background context

For the first crypto slice, `market_background_json` may stay lightweight, but it must include at least:

- market/session classification
- higher-timeframe structure summary
- recent volatility or expansion/compression state

and it must always be derived from historical context rather than invented by the runner.

### 9.4 User Position Advice Input

The first slice should use a small synthetic position fixture so the full chain can be exercised.

The synthetic user fixture should be simple and explicit:

- one long-position case or one flat-position case per sample
- embedded directly in the sample definition

The goal is not user personalization depth yet. The goal is verifying whether the shared chain produces coherent downstream user advice when used as real upstream evidence.

## 10. Experiment Report Shape

The live replay report should extend the current replay report shape, not invent a separate format.

It should include:

- deterministic `experiment_id`
- `dataset_id`
- `pipeline_variant`
- `execution_mode` with value `live_historical`
- `config_source_path`
- per-sample and per-step outputs
- per-step provider
- per-step model
- per-step prompt version
- per-step latency
- per-step raw response capture when available
- per-step schema validation result
- per-step outbound failure details when present
- aggregate programmatic scores

The report must make it obvious whether a run was:

- fixture replay
- or live historical replay

so reports cannot be mixed during later comparisons.

## 11. Scoring Strategy for the First Slice

The first slice should use programmatic scoring only.

### 11.1 Structural Scores

Required:

- `schema_hit_rate`
- `valid_step_runs`
- `total_step_runs`
- `latency_coverage`
- `avg_latency_ms`

### 11.2 Field Completeness Scores

Add scores that check whether required PA-oriented fields are populated meaningfully, not merely present.

Examples:

- decision-tree completeness
- key-level completeness
- signal-bar completeness
- bullish/bearish dual-path completeness

These should remain heuristic but transparent.

### 11.3 Cross-Step Consistency Scores

Add checks for contradictions such as:

- `shared_bar_analysis` contradicting the upstream `shared_pa_state` bias map
- `shared_daily_context` ignoring or reversing upstream structure without explanation
- `user_position_advice` giving actions that conflict with both shared daily and shared bar context

The first implementation can use rule-based checks rather than another LLM judge.

## 12. Prompt Optimization Loop

The intended workflow after the first live slice is:

1. run the live crypto 15m dataset
2. inspect failed or weak-scoring steps
3. classify issues by step:
   - output shape instability
   - missing PA fields
   - vague support/resistance output
   - weak bull/bear symmetry
   - cross-step contradictions
4. revise the prompt for the weakest step first
5. rerun the same dataset with the revised prompt
6. compare reports across runs

The first optimization target should usually be:

- `shared_pa_state_bar_v1`

because downstream quality depends heavily on the fidelity of that shared PA state.

## 13. Failure Handling

Live historical replay must treat failures as first-class output.

Expected failure categories:

- market data fetch failure
- missing historical bars
- invalid input assembly
- provider HTTP failure
- LLM timeout
- non-JSON response
- schema validation failure
- downstream step blocked by upstream failure

Reports must not hide these states.

If a step fails, the report should record:

- the failed step identity
- provider/model used
- error category
- error message
- whether downstream steps were skipped or attempted

## 14. CLI and Operator Experience

The replay CLI should be extended so an operator can choose:

- replay mode
- dataset path
- real config path
- pipeline variant

Minimum required operator flow for the first slice:

`cargo run -p pa-app --bin replay_analysis -- --mode live --dataset <dataset> --config <real-config> --variant baseline_a`

The CLI must make the config path explicit so live runs never silently fall back to `config.example.toml`.

## 15. Deliverables

This phase is complete when:

- a curated crypto `15m` live replay dataset exists
- the replay system can run in real live-historical mode
- real provider data and real LLM execution are used for the selected slice
- reports clearly distinguish live replay from fixture replay
- structural and consistency scores are emitted
- at least one prompt-improvement iteration can be demonstrated from report evidence

## 16. Out of Scope for Later Phases

The following are intentionally deferred:

- A-shares live replay
- forex live replay
- open-bar replay
- tick-informed unfinished-bar reconstruction
- automatic future-outcome scoring
- model-grid search across many variants
- prompt admin UI
- database persistence for experiment history

## 17. Summary

This design adds a real-execution historical replay lane to `oh-paa` without replacing the existing deterministic replay lane.

The first slice stays narrow on purpose:

- `crypto`
- `15m`
- `baseline_a`
- `closed bars only`

That constraint is what makes it possible to establish one trustworthy quality-improvement loop:

- real data
- real models
- real structured outputs
- real failure visibility
- report-driven prompt refinement
