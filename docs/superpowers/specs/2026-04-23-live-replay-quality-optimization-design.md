# Live Replay Quality Optimization Design

Date: 2026-04-23
Project: `oh-paa`
Status: Drafted for review

## 1. Overview

`oh-paa` already has:

- a step-oriented shared analysis pipeline
- step-level prompt and model bindings
- a live historical replay entrypoint for `crypto + 15m + baseline_a`
- programmatic replay quality scores

What it does not yet have is a disciplined optimization loop that can use real model credentials,
real provider data, and repeated `5-sample live replay` evidence to converge on a high-quality
step binding and prompt set.

This design defines that optimization loop.

The loop is explicitly quality-first:

- the final gate is repeated success on the curated `5-sample live replay`
- step-level model replacement is allowed
- prompt updates are allowed
- cost is not the primary objective
- unbounded token burn is not a valid stopping policy

## 2. Goals

Build a repeatable optimization workflow that:

- improves `shared_pa_state_bar`, `shared_bar_analysis`, `shared_daily_context`, and `user_position_advice`
- allows different models per step
- uses real replay evidence instead of intuition
- isolates failures to the first failing step
- converges on one recommended production-quality configuration

This phase must end with:

- one recommended step-level model binding set
- one recommended prompt set
- two consecutive `5-sample live replay` runs that meet the agreed quality bar

## 3. Non-Goals

This phase does not include:

- exhausting account balance or spending without a stopping rule
- expanding the dataset beyond the current curated `5-sample crypto 15m` slice
- changing the overall pipeline topology
- adding new analysis steps
- building a UI for prompt iteration
- optimizing primarily for cost or speed before quality is proven

## 4. Fixed Constraints

The following constraints were explicitly confirmed:

- the hard quality gate is the existing `5-sample live replay`
- success must be demonstrated on full-chain live execution, not only on single-sample tests
- step-level model bindings may differ across the four steps
- DashScope-compatible models may be used, including:
  - `qwen3.6-plus`
  - `qwen3.6-flash`
  - `qwen3.5-plus`
  - `qwen3.5-flash`
  - `qwen3-max`
  - `qwen-flash`
  - `deepseek-v3.2`
  - `glm-5.1`
- the optimization mode is quality-first rather than cost-first

## 5. Recommended Optimization Strategy

The recommended strategy is a quality-first funnel rather than brute-force full replay over the
entire model search space.

### 5.1 Rejected strategy: full brute-force search

Brute-forcing many full `5-sample live replay` combinations is too slow, too expensive, and too
noisy. It also makes it difficult to distinguish:

- model weakness
- prompt weakness
- transport instability
- one-off stochastic failures

### 5.2 Selected strategy: funnel optimization

The selected strategy is:

1. single-step stability screening
2. single-sample full-chain screening
3. `5-sample live replay` promotion
4. two-run final confirmation on the winning candidate

This preserves the `5-sample live replay` as the only final gate while still allowing rapid
elimination of weak candidates earlier in the process.

## 6. Candidate Model Allocation

The search space should be reduced by step responsibility rather than all-model cross product.

### 6.1 `shared_pa_state_bar_v1`

Primary responsibility:

- strict JSON legality
- complete object skeleton preservation
- low variance on repeated calls

Initial candidate pool:

- `qwen3.6-plus`
- `qwen3.5-plus`
- `qwen3-max`
- `deepseek-v3.2`

### 6.2 `shared_bar_analysis_v2`

Primary responsibility:

- balanced bullish and bearish reasoning
- strong key-level detail
- continuation and reversal path completeness

Initial candidate pool:

- `qwen3.6-plus`
- `qwen3-max`
- `deepseek-v3.2`
- `glm-5.1`

### 6.3 `shared_daily_context_v2`

Primary responsibility:

- multi-timeframe synthesis
- context consistency
- strong decision-tree completeness

Initial candidate pool:

- `qwen3.6-plus`
- `qwen3-max`
- `deepseek-v3.2`
- `glm-5.1`

### 6.4 `user_position_advice_v2`

Primary responsibility:

- faithful use of upstream shared outputs
- low schema drift
- stable and consistent downstream framing

Initial candidate pool:

- `qwen3.5-plus`
- `qwen3.6-plus`
- `deepseek-v3.2`

## 7. Funnel Execution Order

### 7.1 Stage 1: single-step screening

Purpose:

- remove obviously unstable candidates before full-chain execution

Execution rules:

- test one step at a time
- change only one model at a time
- start with `shared_pa_state_bar_v1`

Pass conditions:

- repeated calls return valid JSON
- required top-level sections remain present
- required nested decision-tree objects remain present

Immediate fail conditions:

- invalid JSON
- missing required objects
- large structural drift on identical input

### 7.2 Stage 2: single-sample full-chain screening

Purpose:

- validate that the step candidate can survive a real four-step chain

Execution rules:

- run one curated sample through the full chain
- only use candidates that passed Stage 1

Pass conditions:

- all four target steps complete
- all four target steps are schema-valid
- downstream steps remain consistent with upstream outputs

Fail conditions:

- any schema failure in the chain
- clear cross-step contradiction
- consistent loss of key levels, signal bars, or shared-state fidelity

### 7.3 Stage 3: five-sample promotion

Purpose:

- validate the strongest candidates on the real operational gate

Execution rules:

- only promote a small set of strongest candidates
- first run one full `5-sample live replay`
- if the candidate hits the quality bar, run the same `5-sample live replay` again

Only two consecutive passing runs count as final success.

## 8. Quality Scoring and Candidate Promotion

Every candidate run must be recorded with:

- `candidate_id`
- step-level model bindings
- prompt versions
- dataset type: `single-step`, `single-sample`, or `5-sample`
- whether the run completed
- first failing step
- failure type
- all replay quality scores
- short human findings note

### 8.1 Core scored fields

The important replay quality fields are:

- `schema_hit_rate`
- `cross_step_consistency_rate`
- `decision_tree_completeness`
- `key_level_completeness`
- `signal_bar_completeness`
- `bull_bear_dual_path_completeness`

### 8.2 Failure categories

Every failed candidate should be classified as one of:

- `transport`
- `invalid_json`
- `schema_validation`
- `cross_step_drift`

### 8.3 Promotion rules

Promotion rules are fixed:

- Stage 1 to Stage 2:
  - only if repeated single-step outputs remain structurally stable
- Stage 2 to Stage 3:
  - only if the single-sample full chain is fully schema-valid
- Final selection:
  - only if two consecutive `5-sample live replay` runs both pass the hard quality gate

If multiple candidates pass, choose the winner by:

1. higher `cross_step_consistency_rate`
2. higher `signal_bar_completeness`
3. higher `key_level_completeness`
4. lower run-to-run variance
5. stronger warmup-step stability

## 9. Variable Isolation Discipline

To keep the optimization loop interpretable:

- only one variable may change per round
- a round may change one step binding or one prompt
- a round must not change multiple steps at once
- a round must not change a model and a prompt simultaneously
- prompt changes must apply only to the first failing step

This rule is mandatory. The purpose is to preserve causality between:

- change made
- replay result
- next decision

## 10. High-Quality Stop Standard

Optimization must stop once one candidate satisfies all of the following on the same curated
`5-sample live replay` dataset:

- two consecutive complete runs
- no warmup interruption
- both runs produce a full report
- both runs satisfy:
  - `schema_hit_rate = 1.00`
  - `cross_step_consistency_rate >= 0.95`
  - `decision_tree_completeness >= 0.95`
  - `key_level_completeness >= 0.95`
  - `signal_bar_completeness >= 0.95`
  - `bull_bear_dual_path_completeness >= 0.95`

This is the only approved stopping condition for the quality-first optimization phase.

## 11. Required Artifacts Per Round

Each round must leave behind:

- the candidate config or equivalent binding snapshot
- the replay report JSON when applicable
- a short findings note
- a record of:
  - what changed
  - what failed first
  - whether the candidate advanced

Recommended archive outputs:

- `docs/superpowers/archives/<date>-<candidate>-report.json`
- `docs/superpowers/archives/<date>-<candidate>-findings.md`

## 12. Final Deliverables

At the end of this phase, the project should have:

- the final recommended step-level model binding set
- the final recommended prompt versions
- two passing `5-sample live replay` reports
- one short operator command set for re-running the winning configuration
- one short summary explaining why the winning candidate beat the alternatives

## 13. Testing and Verification Expectations

Before claiming success:

- the existing replay-related regression tests must still pass
- the winning candidate must pass the two-run `5-sample live replay` confirmation gate
- report paths and findings notes must be written into the archive folder

## 14. Design Summary

The optimization phase should not behave like an unbounded spend loop.

Instead, it should behave like a disciplined evidence-driven search:

- narrow the candidate pool by step role
- eliminate unstable models early
- validate promising chains on single-sample replay
- promote only strong candidates to `5-sample live replay`
- stop once the two-run high-quality gate is satisfied

That gives `oh-paa` a defensible best-practice boundary for this first live replay production slice.
