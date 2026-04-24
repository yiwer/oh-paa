# Durable Orchestration Storage Design

Date: 2026-04-25
Project: `oh-paa`
Status: Reviewed in chat, pending implementation plan

## 1. Overview

`oh-paa` already has a coherent Phase 2 orchestration model:

- `analysis_tasks`
- `analysis_snapshots`
- `analysis_attempts`
- `analysis_results`
- `analysis_dead_letters`

It also already has:

- enqueue/query APIs in `pa-api`
- a worker execution loop in `pa-orchestrator`
- an in-memory repository used by both API and worker paths

What it still does not have is a durable orchestration repository that survives process restarts and
can be used as the default runtime implementation.

This subproject replaces `InMemoryOrchestrationRepository` as the primary runtime path with a
PostgreSQL-backed repository while preserving current task semantics as closely as possible.

## 2. Goals

This phase must:

- persist orchestration state across process restarts
- preserve current API enqueue/query behavior
- preserve current worker lifecycle semantics
- keep attempt, result, and dead-letter history durable
- support startup recovery for abandoned `running` tasks

This phase is done only when:

- API and worker both use a durable repository in the app runtime
- task/snapshot/attempt/result/dead-letter state remains queryable after restart
- a crashed process can resume abandoned `running` work through explicit recovery logic

## 3. Non-Goals

This phase does not include:

- redesigning task identity or dedupe semantics
- adding a broader operator observability surface
- redesigning retry policy classes
- changing replay pipeline behavior
- changing prompt schemas or analysis payload contracts
- upgrading aggregation or session behavior

`session-calendar-aware` market work is explicitly a separate subproject.

## 4. Fixed Constraints

The following constraints are fixed:

- current `OrchestrationRepository` semantics are the compatibility baseline
- closed-bar dedupe behavior must remain stable
- open-bar repeatability behavior must remain stable
- worker retry / failed / dead-letter semantics must remain stable
- existing `analysis_*` tables remain the primary storage model
- schema changes may add only the minimum fields, indexes, or constraints needed for durable
  semantics; this phase must not redesign the database model

## 5. Recommended Approach

### 5.1 Selected strategy

Use a minimal-intrusion repository replacement:

1. keep `OrchestrationRepository` as the application-facing contract
2. add `PgOrchestrationRepository` inside `pa-orchestrator`
3. map current repository methods onto transactional PostgreSQL operations
4. wire `pa-app` to construct and inject the PostgreSQL repository
5. add startup recovery for stale `running` tasks

This keeps the change focused on durability rather than expanding into workflow redesign.

### 5.2 Rejected strategies

Do not do either of the following in this phase:

- refactor the orchestration contract first and then add PostgreSQL
- replace the current state tables with an append-only event ledger

Both would enlarge this subproject and make failures harder to attribute.

## 6. Runtime Architecture

### 6.1 Repository boundary

`pa-api` and the worker must continue to depend only on `OrchestrationRepository`.

The durable implementation lives in `pa-orchestrator`:

- `InMemoryOrchestrationRepository` remains for tests and pure in-memory scenarios
- `PgOrchestrationRepository` becomes the primary runtime implementation

### 6.2 Runtime wiring

`pa-app/src/main.rs` must:

- build `PgOrchestrationRepository`
- inject it into API state
- inject the same repository into the worker runtime
- run a startup recovery step before worker polling begins

The startup path must no longer default to `InMemoryOrchestrationRepository` for real runtime flows.

## 7. Data Semantics

### 7.1 Insert task with snapshot

`insert_task_with_snapshot(task, snapshot)` must execute in a single transaction.

Required behavior:

- reject task/snapshot ownership mismatch
- write both `analysis_tasks` and `analysis_snapshots` atomically
- preserve dedupe semantics

Dedupe rules remain unchanged:

- if `dedupe_key` is absent, insert normally
- if `dedupe_key` is present and an existing task already owns that key, return
  `DuplicateExistingTask(existing_task_id)`
- the method must never leave behind a task without its snapshot, or a snapshot without its task

### 7.2 Claim next pending task

`claim_next_pending_task()` is the highest-risk concurrent operation.

It must run in a single transaction and:

1. select one runnable task with `status in ('pending', 'retry_waiting')`
2. lock it so competing workers cannot claim the same row
3. update it to `running`
4. set `started_at` if it was previously unset
5. return the claimed task

The durable implementation must preserve the in-memory meaning:

- one task claim per worker poll
- no duplicate concurrent claims
- `retry_waiting` tasks are reclaimable

### 7.3 Release claimed task

`release_claimed_task(task_id, message)` must preserve current semantics:

- only a currently claimed/running task may be released
- the task returns to a runnable state rather than being terminally failed
- `last_error_code` and `last_error_message` are updated to explain the release

This phase keeps the current behavior rather than redesigning recovery semantics.

### 7.4 Outcome persistence

All worker outcomes must be persisted transactionally so that attempt history and task state cannot
diverge.

### Success

`persist_success_outcome(task_id, attempt, result)` must atomically:

- append the attempt
- insert the result
- mark the task `succeeded`
- set `finished_at`

### Retryable failure

`persist_schema_validation_failure_outcome(...)` and `persist_outbound_retry_outcome(...)` must
atomically:

- append the attempt
- increment `attempt_count` as currently expected by the worker/task flow
- move the task to `retry_waiting`
- update the last error fields

### Terminal failure

`persist_outbound_terminal_failure_outcome(...)` must atomically:

- append the attempt
- mark the task `failed`
- set `finished_at`
- update the last error fields

### Dead letter

`persist_outbound_dead_letter_outcome(...)` must atomically:

- append the attempt
- insert the dead letter row
- mark the task `dead_letter`
- set `finished_at`
- update the last error fields

## 8. Startup Recovery

Durability without recovery is not sufficient for this phase.

This design adopts explicit startup recovery for stale `running` tasks.

### 8.1 Recovery rule

At process startup, before worker polling begins:

- find tasks with `status = 'running'`
- restrict to tasks whose `started_at` is older than a configured or conservative recovery threshold
- move them to `retry_waiting`
- set a clear recovery error marker such as:
  - `last_error_code = 'worker_recovered_on_startup'`
  - `last_error_message = <human-readable recovery note>`

### 8.2 Why `retry_waiting`

Recovered tasks should return to `retry_waiting`, not directly to `pending`, because:

- they were already claimed once
- they represent interrupted execution rather than untouched work
- operators and tests should be able to distinguish “never started” from “started and recovered”

### 8.3 Non-goals for recovery

This phase does not add:

- heartbeat-based live worker leases
- distributed leader election
- automatic per-worker orphan rebalancing while the process remains healthy

It only guarantees safe recovery across process restart boundaries.

## 9. Schema Strategy

This phase reuses the existing `analysis_*` tables. It may add only minimal schema hardening needed
for correctness and performance.

Likely additions:

- an index supporting claim queries on `analysis_tasks(status, scheduled_at)`
- a dedupe index or uniqueness strategy for `analysis_tasks(dedupe_key)` where appropriate
- a uniqueness constraint for `analysis_attempts(task_id, attempt_no)`
- a uniqueness constraint for `analysis_results(task_id)`
- a uniqueness constraint for `analysis_dead_letters(task_id)`

If existing columns are insufficient for startup recovery or transactional correctness, only the
smallest compatible additions are allowed.

## 10. File Scope

Primary files in scope:

- `crates/pa-orchestrator/src/repository.rs`
- `crates/pa-orchestrator/src/models.rs`
- `crates/pa-orchestrator/src/worker.rs`
- `crates/pa-orchestrator/tests/worker.rs`
- `crates/pa-orchestrator/tests/dedupe.rs`
- `crates/pa-api/src/router.rs`
- `crates/pa-app/src/main.rs`
- `crates/pa-api/tests/smoke.rs`
- `migrations/`
- `docs/architecture/phase1-runtime.md`

Likely new artifacts:

- a PostgreSQL repository implementation in `pa-orchestrator`
- PostgreSQL-backed repository tests

## 11. Testing Strategy

This phase must be implemented test-first.

### 11.1 Repository semantic baseline

The current in-memory tests remain the baseline for expected behavior.

The PostgreSQL repository must prove equivalent semantics for:

- dedupe behavior
- claim atomicity
- retry waiting reclaim
- terminal failure transitions
- dead-letter persistence
- snapshot/result lookup paths

### 11.2 PostgreSQL integration tests

Add focused PostgreSQL tests for:

- atomic `insert_task_with_snapshot`
- transactional `claim_next_pending_task`
- no duplicate task claim under repeated access
- success path persists both attempt and result
- retry path persists attempt and updates task state
- dead-letter path persists attempt and dead letter together
- startup recovery moves stale `running` tasks back to `retry_waiting`

### 11.3 API/runtime verification

Runtime verification must prove:

- API enqueue/query still works with the durable repository
- worker outcomes remain queryable after restart boundaries
- startup recovery is idempotent and does not repeatedly corrupt the same task

## 12. Verification Bar

Before claiming this phase complete, verification must include at minimum:

- `cargo test -p pa-orchestrator`
- `cargo test -p pa-api --lib`
- `cargo test -p pa-api --test smoke`
- `cargo check --workspace`

If any PostgreSQL-gated tests self-skip due to missing environment variables, that must be stated
explicitly in the final report.

## 13. Risks

Primary risks:

- semantic drift between the in-memory repository and the PostgreSQL implementation
- partial transaction boundaries that let task state and attempt/result rows disagree
- incorrect claim logic that allows duplicate worker execution
- startup recovery that is either too aggressive or too weak

Mitigations:

- keep trait semantics stable
- prefer one transaction per repository state transition
- add claim-focused tests before implementation
- keep startup recovery simple and explicit

## 14. Success Criteria

This design is successful only when all of the following are true:

- the app no longer depends on `InMemoryOrchestrationRepository` as the primary runtime path
- orchestration state survives process restarts
- abandoned `running` tasks can be recovered on startup
- existing enqueue/query/worker semantics remain compatible
- no broader observability or orchestration redesign was pulled into scope

## 15. Deferred Follow-Up

Explicitly deferred to later work:

- richer task observability APIs
- operator task listing / filtering surfaces
- heartbeat-based worker leasing
- distributed scheduling concerns
- event-ledger orchestration redesign
- `session-calendar-aware` market aggregation and session semantics
