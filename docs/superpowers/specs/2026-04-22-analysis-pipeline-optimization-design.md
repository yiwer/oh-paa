# Analysis Pipeline Optimization Design

Date: 2026-04-22
Project: `oh-paa`
Status: Drafted for review

## 1. Overview

The current shared analysis flow in `oh-paa` is functional but too flat:

- `shared_bar_analysis` directly turns runtime market context into final output
- `shared_daily_context` directly turns runtime market context into final output
- prompt definitions are code-defined, but model execution is not step-configurable
- prompt evolution, model replacement, and flow-order experiments are difficult to compare rigorously

The next phase should turn analysis into an explicitly layered, experiment-friendly pipeline that supports:

- durable intermediate public analysis assets
- step-level prompt and schema versioning
- step-level model/provider replacement using OpenAI-compatible transports
- replay-based evaluation on historical data
- continuous prompt and flow optimization based on evidence rather than intuition

## 2. Goals

Build a robust shared-analysis pipeline that is easier to optimize for:

- structured output stability
- PA judgment quality
- usefulness of downstream user-position advice

while keeping cost and latency within acceptable operational bounds.

This phase must support:

- a new persistent public intermediate asset: `shared_pa_state`
- continued support for `shared_bar_analysis` and `shared_daily_context`
- code-defined step specs with separable prompt and model bindings
- different models per step
- OpenAI-compatible provider abstraction so DeepSeek, DashScope, and future compatible endpoints can be swapped without changing orchestration flow
- replay-based evaluation over historical data

This phase does not include:

- online prompt editing UI
- generic workflow DSLs
- multi-reviewer or critic chains as default behavior
- portfolio-level multi-asset reasoning

## 3. Confirmed Design Decisions

The following decisions were explicitly confirmed during design:

- the pipeline may add a new intermediate internal step
- that step must be durable, queryable, versioned, and treated as a first-class public shared asset
- the preferred architecture is an assetized middle layer rather than direct final-output prompting
- flow order should remain experimentally adjustable rather than hard-coded from intuition alone
- evaluation priority order is:
  - structured output stability
  - PA judgment quality
  - usefulness for downstream user-position advice
  - cost and latency only as guardrail metrics

## 4. Analysis Asset Layers

The shared analysis system should be split into three public asset layers.

### 4.1 `shared_pa_state`

This is the new foundational shared asset.

Its role is to convert runtime market evidence into structured PA state rather than final commentary. It should be:

- persistent
- queryable
- versioned
- reusable across later shared and user analysis steps

Primary responsibility:

- encode structured observations, locations, relationships, and PA decision-tree state

Non-goals:

- no user-specific advice
- no heavy narrative explanation unless needed to preserve reasoning traceability

### 4.2 `shared_bar_analysis`

This remains the public, single-target-bar interpretation asset.

Its role is to consume `shared_pa_state` for the target bar and explain that bar from both long and short perspectives. It is the shared explanation layer for a single bar.

### 4.3 `shared_daily_context`

This remains the public, per-instrument, per-trading-day context asset.

Its role is to combine:

- recent `shared_pa_state`
- recent `shared_bar_analysis`
- multi-timeframe structure context

into one daily public market-background read.

### 4.4 Downstream user analysis

`user_position_advice` should continue to be downstream of shared assets. It should preferentially consume:

- `shared_daily_context`
- `shared_bar_analysis`
- and only secondarily `shared_pa_state` as supplemental evidence

It should not bypass shared assets to reinterpret raw provider market data.

## 5. Recommended Dependency and Flow Model

The recommended dependency model is:

- `shared_pa_state` is the base layer
- `shared_bar_analysis` depends on `shared_pa_state`
- `shared_daily_context` depends on `shared_pa_state` and may also reference recent `shared_bar_analysis`
- `user_position_advice` depends on shared assets, not raw provider payloads

### 5.1 Production order

Recommended default production order:

1. create `shared_pa_state`
2. create `shared_bar_analysis`
3. create `shared_daily_context`

However, `shared_daily_context` must not be over-coupled to `shared_bar_analysis`. It should be able to run from:

- recent `shared_pa_state`
- recent structure context
- optionally recent `shared_bar_analysis`

This preserves the ability to experiment with:

- lighter daily-context dependence on bar-analysis outputs
- stronger daily-context dependence on bar-analysis outputs

### 5.2 Consumption order

Recommended downstream consumption order:

1. `shared_daily_context`
2. `shared_bar_analysis`
3. `shared_pa_state` only as fallback or supporting evidence

### 5.3 Why not hard-code a single linear chain

The system should avoid forcing:

- `pa_state -> bar -> daily` as an absolute rule
- or `pa_state -> daily -> bar` as an absolute rule

because both can introduce undesirable bias:

- over-coupling daily context to per-bar interpretation quality
- or over-biasing bar interpretation with daily priors

The correct order boundary should be treated as an experiment variable and validated using historical replay.

## 6. Step Definition, Prompting, and Model Binding

The current `PromptSpec` approach should be split conceptually into three layers.

### 6.1 `AnalysisStepSpec`

This defines what a step is.

It should include:

- `step_key`
- `step_version`
- `task_type`
- `input_schema_version`
- `output_schema_version`
- `output_json_schema`
- `result_semantics`
- `bar_state_support`
- `dependency_policy`

Examples:

- `shared_pa_state_bar_v1`
- `shared_bar_analysis_v2`
- `shared_daily_context_v2`
- `user_position_advice_v2`

### 6.2 `PromptTemplateSpec`

This defines how the step is instructed.

It should contain:

- primary system instructions
- optional developer-style instruction blocks
- field-by-field expectations
- explicit PA-specific constraints
- explicit structured output constraints

Prompt templates should be versionable independently from model bindings.

### 6.3 `ModelExecutionProfile`

This defines how a step is executed against an LLM provider.

It should include:

- `provider`
- `base_url`
- `api_key_env`
- `model`
- `temperature`
- `max_tokens`
- `timeout_secs`
- `max_retries`
- `retry_initial_backoff_ms`
- `supports_json_schema`
- `supports_reasoning`
- `openai_api_style`

### 6.4 `StepExecutionBinding`

This is the step-to-model mapping layer.

It should bind one step version to one execution profile.

Examples:

- `shared_pa_state_bar_v1 -> pa_state_extract_fast`
- `shared_bar_analysis_v2 -> shared_bar_reasoner`
- `shared_daily_context_v2 -> daily_context_reasoner`
- `user_position_advice_v2 -> user_advice_balanced`

This binding should be externally configurable, with configuration-file support first and database management later if needed.

## 7. OpenAI-Compatible LLM Abstraction

The execution layer should become OpenAI-compatible first, with provider-specific adaptation behind that interface.

### 7.1 Transport model

Recommended architecture:

- `OpenAiCompatibleClient`
- provider adapters such as:
  - `deepseek`
  - `dashscope`
  - `openai`
- structured-output mode selection:
  - `native_json_schema`
  - `json_object`
  - `prompt_enforced_json`

### 7.2 Rationale

This matches confirmed requirements:

- LLM configuration should be dynamically replaceable
- different steps should support different models
- real experimentation should use DeepSeek and DashScope via compatible endpoints

The orchestration layer should not need to change when:

- provider changes
- model changes
- one step uses a reasoning model and another uses a cheaper structured-output-focused model

## 8. Shared Asset Schema Direction

### 8.1 `shared_pa_state_bar`

Recommended top-level fields:

- `bar_identity`
- `market_session_context`
- `bar_observation`
- `bar_shape`
- `location_context`
- `multi_timeframe_alignment`
- `support_resistance_map`
- `signal_assessment`
- `decision_tree_state`
- `evidence_log`

Key design intent:

- encode reusable PA facts and state
- not final commentary
- preserve traceability from conclusions to evidence

The `decision_tree_state` section should explicitly contain:

- `trend_context`
- `location_context`
- `signal_quality`
- `confirmation_state`
- `invalidation_conditions`
- `bias_balance`

### 8.2 `shared_bar_analysis`

Recommended top-level fields:

- `bar_identity`
- `bar_summary`
- `market_story`
- `bullish_case`
- `bearish_case`
- `two_sided_balance`
- `key_levels`
- `signal_bar_verdict`
- `continuation_path`
- `reversal_path`
- `invalidation_map`
- `follow_through_checkpoints`

This asset must remain explicitly two-sided. It is the public interpretation layer for one target bar.

### 8.3 `shared_daily_context`

Recommended top-level fields:

- `context_identity`
- `market_background`
- `dominant_structure`
- `intraday_vs_higher_timeframe_state`
- `key_support_levels`
- `key_resistance_levels`
- `signal_bars`
- `candle_pattern_map`
- `decision_tree_nodes`
- `liquidity_context`
- `scenario_map`
- `risk_notes`
- `session_playbook`

The `decision_tree_nodes` section should explicitly contain:

- `trend_context`
- `location_context`
- `signal_quality`
- `confirmation_state`
- `invalidation_conditions`
- `path_of_least_resistance`

The schema must keep explicit PA decision-tree structure rather than generic prose commentary.

## 9. Prompting Rules

All shared and user prompts should follow these rules:

- instructions must prioritize structured evidence-based output over free-form analyst prose
- every field should define:
  - what question it answers
  - which input evidence it may cite
  - what it must not invent
- shared prompts must not output user-specific advice
- user prompts must build on shared assets rather than re-reading raw market provider data
- prompts should enforce JSON-only behavior but actual correctness is still guarded by local schema validation

## 10. Evaluation and Replay Framework

Pipeline optimization should be driven by replayable experiments.

### 10.1 Experiment unit

One experiment unit should capture:

- `pipeline_variant`
- `step_binding_set`
- `prompt_version_set`
- `dataset_slice`

This makes flow-order comparisons and model-binding comparisons reproducible.

### 10.2 Dataset organization

Historical replay datasets should be grouped into:

- `bar replay set`
- `daily replay set`
- `user replay set`

Coverage should include:

- A-shares
- crypto
- forex
- 15m / 1h / 1d contexts
- trend, range, failed breakout, reversal, and shock-follow-through regimes

### 10.3 Evaluation priority

Primary evaluation dimensions:

1. structured output stability
2. PA judgment quality
3. usefulness for downstream user-position advice

Secondary guardrail dimension:

4. cost and latency

### 10.4 Scoring layers

Two scoring layers should be used:

1. programmatic checks
- schema hit rate
- field completeness
- enum validity
- invalid-output rate
- repeated-run stability
- dependency-resolution correctness

2. qualitative scoring
- LLM-as-judge with fixed rubric
- human spot review on key or disputed samples

The judge layer is not the source of truth. It is a high-throughput screening mechanism.

### 10.5 Experiment logs

Each experiment run should persist:

- `experiment_id`
- `dataset_id`
- `pipeline_variant`
- per-step provider and model
- per-step prompt version
- per-step outputs
- per-step latency
- schema validation outcome
- judge score
- human notes when present

## 11. Phase 1 Delivery Scope for This Optimization Track

The first implementation phase should focus on the smallest complete optimization loop.

### 11.1 In scope

1. add persistent `shared_pa_state_bar`
2. refactor step definitions, prompt templates, and execution bindings into separable layers
3. add OpenAI-compatible execution support for:
   - DeepSeek-compatible endpoints
   - DashScope-compatible endpoints
4. update `shared_bar_analysis` and `shared_daily_context` to consume `shared_pa_state`
5. add replay-and-experiment infrastructure for historical samples
6. reconnect `user_position_advice` so it remains compatible with the new shared asset chain

### 11.2 Out of scope

- online prompt admin UI
- workflow DSLs
- reviewer/critic multi-pass chains by default
- portfolio-level reasoning
- fully automatic model-search systems
- advanced production rescheduling logic for high-frequency real-time recomputation

### 11.3 Done criteria

This phase is complete when:

- historical samples can be replayed through multiple pipeline variants
- different steps can use different LLM models
- `shared_pa_state -> shared_bar_analysis / shared_daily_context` runs end-to-end
- outputs remain schema-stable
- experiment logs allow comparison between flow variants, prompt versions, and model bindings

## 12. Architecture Summary

In one sentence:

This phase turns shared analysis into a layered, durable, replay-evaluable PA pipeline built around `shared_pa_state` as a first-class public asset, while decoupling step definition, prompt evolution, and model execution so each stage can be optimized independently with OpenAI-compatible LLM providers.
