# Phase 2 LLM Analysis Orchestration Design

Date: 2026-04-21
Project: `oh-paa`
Status: Approved for planning

## 1. Overview

Phase 1 established the data foundation for the multi-market price action analysis server:

- canonical K-lines are the only business-truth bars
- open bars are runtime-derived views
- shared per-bar analysis and shared daily market context exist as domain concepts
- user analysis consumes shared outputs plus private position context

Phase 2 focuses on making the analysis layer operationally robust. The main gap in Phase 1 is that shared analysis and user analysis are still simple synchronous skeletons. They do not yet provide:

- asynchronous task orchestration
- full input snapshotting for audit and replay
- versioned code-defined prompt contracts
- structured output schema enforcement
- retry, dead-letter, and execution-attempt tracking

Phase 2 adds a task-centered LLM orchestration layer that treats analysis execution as durable background work while preserving strict PA-oriented output contracts.

## 2. Goals

Phase 2 goal:

Build a durable, auditable, code-versioned LLM analysis orchestration layer for shared and user-specific price action analysis.

Phase 2 includes:

- full asynchronous task execution for all analysis types
- durable task, snapshot, attempt, result, and dead-letter persistence
- complete input JSON snapshotting at task creation time
- code-defined `PromptSpec` contracts with versioned input/output schemas
- strict JSON-only formal outputs
- retry policies based on error type
- `closed bar` strict deduplication
- `open bar` repeatable execution with formal persistence
- PA-specific structured outputs for:
  - shared bar analysis
  - shared daily market context
  - user position advice

Phase 2 does not include:

- database-managed prompt hot updates
- general-purpose workflow-engine abstraction
- frontend workflow design
- broker or account execution automation

## 3. Design Constraints Confirmed

The following requirements were explicitly confirmed during design:

- all analysis execution is asynchronous and task-based
- shared analysis, user manual analysis, and user scheduled analysis all enqueue tasks first
- `closed kline` analysis tasks are strictly deduplicated
- `open kline` analysis tasks may be repeatedly triggered
- user manual analysis is generally repeatable, but still respects the `closed/open` distinction above
- task creation must persist the full input JSON snapshot
- prompt definitions and versions are code-defined and shipped with releases
- formal analysis outputs are strictly structured JSON
- `open bar` analysis results are formally persisted and marked as `open`
- shared daily context must encode PA decision-tree nodes explicitly
- shared bar analysis must always express both bullish and bearish readings

## 4. Architecture Summary

Phase 2 uses a task-centered orchestration design.

### 4.1 Recommended architecture

Recommended approach:

- keep shared-analysis domain logic in `pa-analysis`
- keep user-analysis domain logic in `pa-user`
- add a focused `pa-orchestrator` crate for durable execution

This keeps PA semantics in the business crates while centralizing the durable execution mechanics.

### 4.2 Responsibility split

`pa-analysis` responsibilities:

- define shared-analysis input/output Rust models
- define `shared_bar_analysis_v1` and `shared_daily_context_v1` `PromptSpec`s
- assemble shared-analysis task snapshots
- expose shared-analysis result query interfaces

`pa-user` responsibilities:

- define user-analysis input/output Rust models
- define `user_position_advice_v1` `PromptSpec`
- assemble user-analysis task snapshots
- expose user-analysis result query interfaces

`pa-orchestrator` responsibilities:

- persist and lock tasks
- persist snapshots
- persist execution attempts
- execute LLM requests
- validate structured outputs
- apply retry rules
- persist final results
- persist dead-letter records
- expose a prompt registry keyed by `prompt_key + prompt_version`

`pa-api` responsibilities:

- create tasks
- query task state
- query final structured results
- expose attempt and dead-letter inspection endpoints

`pa-app` responsibilities:

- initialize the prompt registry
- initialize LLM client(s)
- start workers
- wire schedulers to task factories

## 5. Prompt Contract Model

Each analysis type is defined in code through a `PromptSpec`.

Each `PromptSpec` must include:

- `prompt_key`
- `prompt_version`
- `task_type`
- `system_prompt`
- `input_schema_version`
- `output_schema_version`
- `output_json_schema`
- `retry_policy_class`
- `result_semantics`
- `bar_state_support`

Rules:

- there is exactly one effective `PromptSpec` for a given `prompt_key + prompt_version`
- worker execution must resolve the task to a registered `PromptSpec`
- worker execution must validate snapshot inputs against the declared input type
- worker execution must validate LLM output against the declared output schema
- output validation failure is treated as a formal execution failure, not a partial success

Phase 2 initial `PromptSpec`s:

- `shared_bar_analysis_v1`
- `shared_daily_context_v1`
- `user_position_advice_v1`

## 6. Persistence Model

Phase 2 introduces five orchestration tables.

### 6.1 `analysis_tasks`

Purpose:

- durable task queue and lifecycle tracking

Required fields:

- `task_id`
- `task_type`
- `task_status`
- `instrument_id`
- `user_id` nullable
- `timeframe` nullable
- `bar_state`
- `bar_open_time` nullable
- `bar_close_time` nullable
- `trading_date` nullable
- `trigger_type`
- `prompt_key`
- `prompt_version`
- `snapshot_id`
- `dedupe_key` nullable
- `attempt_count`
- `max_attempts`
- `scheduled_at`
- `started_at` nullable
- `finished_at` nullable
- `last_error_code` nullable
- `last_error_message` nullable

### 6.2 `analysis_snapshots`

Purpose:

- persist the exact input used for execution

Required fields:

- `snapshot_id`
- `task_id`
- `input_json`
- `input_hash`
- `schema_version`
- `created_at`

Rules:

- the full input JSON is persisted at task-creation time
- workers execute from this snapshot only
- later data changes do not alter historical execution inputs
- task-to-snapshot ownership must be enforced as a real 1:1 database relationship rather than a loose UUID convention

### 6.3 `analysis_attempts`

Purpose:

- audit every execution attempt

Required fields:

- `attempt_id`
- `task_id`
- `attempt_no`
- `worker_id`
- `llm_provider`
- `model`
- `request_payload_json`
- `raw_response_json`
- `parsed_output_json` nullable
- `status`
- `error_type` nullable
- `error_message` nullable
- `started_at`
- `finished_at` nullable

Rules:

- `task_id + attempt_no` must be unique
- an in-flight attempt may exist before `finished_at` is known

### 6.4 `analysis_results`

Purpose:

- store the final formal structured result

Required fields:

- `result_id`
- `task_id`
- `task_type`
- `instrument_id`
- `user_id` nullable
- `timeframe` nullable
- `bar_state`
- `bar_open_time` nullable
- `bar_close_time` nullable
- `trading_date` nullable
- `prompt_key`
- `prompt_version`
- `output_json`
- `created_at`

### 6.5 `analysis_dead_letters`

Purpose:

- persist tasks that exceeded retry limits or were marked unrecoverable

Required fields:

- `dead_letter_id`
- `task_id`
- `final_error_type`
- `final_error_message`
- `last_attempt_id`
- `archived_snapshot_json`
- `created_at`

## 7. Task Identity and Deduplication

### 7.1 Shared bar analysis

`closed bar` shared analysis:

- strictly deduplicated
- dedupe key includes:
  - `task_type`
  - `instrument_id`
  - `timeframe`
  - `bar_close_time`
  - `prompt_key`
  - `prompt_version`
  - `bar_state=closed`

`open bar` shared analysis:

- not deduplicated
- repeated requests always create new tasks

### 7.2 Shared daily context

Strictly deduplicated by:

- `task_type`
- `instrument_id`
- `trading_date`
- `prompt_key`
- `prompt_version`

### 7.3 User manual analysis

`closed bar` user analysis:

- strictly deduplicated
- dedupe key includes:
  - `task_type`
  - `user_id`
  - `instrument_id`
  - `timeframe`
  - `bar_close_time`
  - `prompt_key`
  - `prompt_version`
  - `position_snapshot_hash`
  - `bar_state=closed`

`open bar` user analysis:

- not deduplicated
- each trigger creates a new task

### 7.4 User scheduled analysis

Strictly deduplicated by logical schedule identity, including:

- `schedule_id`
- `user_id`
- `instrument_id`
- target bar or target trading date
- `prompt_key`
- `prompt_version`
- `position_snapshot_hash`

## 8. Task Lifecycle

Recommended task statuses:

- `pending`
- `running`
- `retry_waiting`
- `succeeded`
- `failed`
- `dead_letter`
- `cancelled`

State-flow rules:

- `pending -> running -> succeeded`
- `pending -> running -> retry_waiting -> pending` for retryable failure
- `pending -> running -> failed` for terminal non-retryable failure
- `pending -> running -> dead_letter` when retry budget is exhausted or manual dead-lettering applies

Task execution rules:

- worker must acquire a task transactionally
- worker must increment `attempt_count` exactly once per actual execution attempt
- worker must create an `analysis_attempts` row for every outbound LLM request
- worker must not mutate the original snapshot

## 9. Error Classification and Retry Policy

Automatically retryable classes:

- temporary network failure
- upstream timeout
- LLM provider rate limit
- temporary upstream `5xx`
- other transient infrastructure failures judged recoverable

Non-retryable classes:

- missing required snapshot data
- invalid input shape
- unmet business preconditions
- invalid structured output
- schema validation failure
- deterministic domain validation failure

Rules:

- retryable failures return the task to retry flow until `max_attempts` is reached
- non-retryable failures terminate immediately
- tasks exceeding retry budget are moved to `dead_letter`
- retry classification must be conservative; broad provider/storage buckets are not sufficient when they already contain deterministic parse or config failures

## 10. Execution Rules

Worker execution must follow this sequence:

1. lock one `pending` task
2. transition it to `running`
3. load the linked snapshot
4. resolve the code-defined `PromptSpec`
5. build the LLM request from snapshot plus prompt spec
6. call the LLM provider
7. validate returned JSON against the declared output schema
8. persist an execution attempt
9. on success, persist final result and mark the task `succeeded`
10. on failure, classify the error and transition to retry, failure, or dead-letter flow

Hard rules:

- workers do not fetch business-domain inputs during execution
- workers do not rebuild prompt context from live domain tables
- results are persisted only after schema validation succeeds

## 11. PA-Oriented Structured Output Contracts

### 11.1 `shared_daily_context_v1`

This output represents the shared public market-context read for one instrument and trading date.

It must contain these top-level fields:

- `market_background`
- `market_structure`
- `key_support_levels`
- `key_resistance_levels`
- `signal_bars`
- `candle_patterns`
- `decision_tree_nodes`
- `liquidity_context`
- `risk_notes`
- `scenario_map`

`decision_tree_nodes` must explicitly include:

- `trend_context`
- `location_context`
- `signal_quality`
- `confirmation_state`
- `invalidation_conditions`

Rule:

- this schema must capture explicit PA decision-tree state, not generic prose commentary

### 11.2 `shared_bar_analysis_v1`

This output represents one shared public reading of a single target bar.

It must contain these top-level fields:

- `bar_state`
- `bar_classification`
- `bullish_case`
- `bearish_case`
- `two_sided_summary`
- `nearby_levels`
- `signal_strength`
- `continuation_scenarios`
- `reversal_scenarios`
- `invalidation_levels`
- `execution_bias_notes`

Rules:

- both bullish and bearish interpretations are mandatory
- single-sided output is invalid
- `bar_state` must explicitly distinguish `open` and `closed`

### 11.3 `user_position_advice_v1`

This output maps shared PA structure into user-position advice.

It must contain these top-level fields:

- `position_state`
- `market_read_through`
- `bullish_path_for_user`
- `bearish_path_for_user`
- `hold_reduce_exit_conditions`
- `risk_control_levels`
- `invalidations`
- `action_candidates`

Rules:

- user output must build on shared outputs, not reinterpret raw provider data
- user output is position-aware rather than market-definition-first

## 12. Core Data Flows

### 12.1 Shared `closed bar` analysis

Flow:

`canonical_kline write -> shared task factory -> snapshot persistence -> task persistence -> worker execution -> result persistence`

Task-factory responsibilities:

- read canonical bar data
- collect surrounding structure context
- declare `bar_state=closed`
- compute dedupe key
- persist the full snapshot

### 12.2 Shared `open bar` analysis

Flow:

`user/system trigger -> shared task factory -> snapshot persistence -> worker execution -> result persistence`

Task-factory responsibilities:

- capture the current open-bar state
- capture latest tick timestamp
- capture surrounding recent closed bars
- declare `bar_state=open`

Rule:

- every trigger may generate a new task and a new formal result

### 12.3 Shared daily context

Flow:

`daily scheduler -> daily-context task factory -> snapshot persistence -> worker execution -> result persistence`

Snapshot content must include:

- recent `15m`, `1h`, and `1d` structure
- relevant recent shared bar analyses
- key support and resistance areas
- signal-bar candidates
- market background context

### 12.4 User manual analysis

Flow:

`user request -> user task factory -> snapshot persistence -> task persistence -> worker execution -> result persistence`

Snapshot content must include:

- user subscriptions
- user positions
- linked shared bar analysis
- linked shared daily context
- target bar or date context

Rule:

- user analysis never calls providers directly

### 12.5 User scheduled analysis

Flow:

`schedule hit -> user scheduled task factory -> snapshot persistence -> worker execution -> result persistence`

Rule:

- scheduled and manual user analysis share the same execution pipeline

## 13. API Surface

Phase 2 API additions should follow enqueue-and-query semantics.

Create-task endpoints:

- `POST /analysis/shared/bar`
- `POST /analysis/shared/daily`
- `POST /user/analysis/manual`

Recommended future endpoint:

- `POST /user/analysis/scheduled`

Task and result query endpoints:

- `GET /analysis/tasks/:task_id`
- `GET /analysis/results/:task_id`
- `GET /analysis/tasks/:task_id/attempts`
- `GET /analysis/dead-letters/:task_id`

Create-task endpoint response should include:

- `task_id`
- `task_status`
- `created_at`
- `deduplicated`
- `result_id` nullable

Rule:

- API handlers enqueue work and return task metadata
- API handlers do not synchronously execute LLM analysis

## 14. Observability and Audit

Phase 2 must make every execution traceable through:

- task record
- full snapshot
- attempt history
- formal result
- dead-letter record when applicable

Operationally useful fields and signals include:

- worker identifier
- model name
- provider name
- error type
- last error code
- attempt number
- final status
- prompt key and version

## 15. Out of Scope

Phase 2 does not include:

- runtime prompt editing in database
- generic low-code workflow orchestration
- portfolio-level multi-instrument reasoning
- broker order execution
- frontend UX for prompt management
- cross-provider LLM arbitration

## 16. Architecture Summary

In one sentence:

Phase 2 turns the current shared-analysis and user-analysis skeletons into a durable, code-versioned, PA-aware LLM execution system built on task persistence, full input snapshots, strict structured output validation, typed retry behavior, and explicit shared-versus-user analysis boundaries.
