Date: 2026-04-24
Project: `oh-paa`
Status: Drafted for review

# Auditable Event-Ledger Orchestration Design

## 1. Overview

`oh-paa` already has a working Phase 2 analysis flow with:

- task creation APIs
- input snapshot construction
- a worker execution loop
- attempt, result, and dead-letter models
- an in-memory orchestration repository

What it does not yet have is a durable orchestration system that is:

- restart-safe
- fully auditable
- evidence-preserving
- queryable without relying on raw logs

This design upgrades orchestration persistence from an in-memory state holder to a PostgreSQL-backed
event ledger with derived read models. The ledger becomes the primary source of truth for task
lifecycle facts. Current task state, attempt views, results, and dead letters become projections or
optimized read models built from those facts.

The design deliberately does not optimize LLM reasoning quality, prompt quality, or task semantics in
this phase. Its purpose is to remove black-box behavior from the orchestration runtime so every
meaningful execution step is persisted, inspectable, and replayable for later manual tuning.

## 2. Goals

Build a durable orchestration storage path that:

- persists orchestration state across process restarts
- records every meaningful orchestration transition as an append-only event
- preserves the full evidence chain for each LLM execution attempt
- provides first-class audit queries by `task_id`, `trace_id`, `step_key`, and time range
- keeps current task lifecycle semantics stable
- keeps PostgreSQL as the first storage target for all orchestration evidence in this phase

This phase must end with:

- one PostgreSQL-backed orchestration repository used as the primary runtime path
- one append-only event ledger table for orchestration facts
- one unified artifact table for snapshots and execution evidence
- one set of read models for fast API queries
- API endpoints that expose task timeline, attempts, artifacts, and audit-event queries
- logging that consistently carries the IDs needed to correlate logs with persisted events

## 3. Non-Goals

This phase does not include:

- prompt optimization
- LLM schema redesign
- new task types
- new queue semantics beyond what is required for durable claim and recovery
- a frontend audit UI
- full-system migration of market, replay, and admin modules into the same ledger in this phase
- external object storage for large artifacts
- default redaction of persisted evidence

The design only requires that other modules can adopt the same event and trace conventions later.

## 4. Fixed Constraints

The following constraints are fixed:

- current task lifecycle semantics must remain stable
- current dedupe semantics must remain stable
- current retry and dead-letter behavior must remain stable
- every meaningful failure path must persist an auditable event, not only a log line
- logs are not the source of truth; the database ledger is
- raw request, raw response, input snapshot, validation errors, and dead-letter archives are stored in PostgreSQL in this phase
- evidence is stored in original form by default, while leaving room for future redaction support

## 5. Recommended Approach

### 5.1 Selected strategy

Use an event-ledger-first repository design:

1. append orchestration facts to a durable ledger
2. store snapshots and execution evidence as first-class artifacts
3. derive current task state and query-friendly read models from the ledger
4. expose audit-oriented query APIs on top of those read models

This is preferred over direct state-table mutation because the main requirement is not just
durability. The main requirement is durable, non-black-box execution evidence that can support
investigation, human review, and later tuning.

### 5.2 Rejected strategies

Do not use a state-table-first design with audit as a sidecar. That would keep the real source of
truth in mutable rows and reduce audit completeness.

Do not attempt a full cross-system event platform in this phase. That would expand scope beyond the
orchestration bottleneck and delay the durable repository replacement.

## 6. High-Level Architecture

The orchestration persistence model has three layers:

### 6.1 Ledger

`orchestration_events` is the primary fact store. Every meaningful lifecycle step is written here as
an append-only record.

### 6.2 Evidence

`analysis_artifacts` stores auditable JSON evidence linked to the event that created it, including:

- task input snapshots
- LLM request payloads
- raw LLM responses
- parsed outputs
- schema-validation failure payloads
- retry-decision evidence
- dead-letter archives

### 6.3 Projections

Projection tables support fast runtime and API reads without replacing the ledger as truth:

- `analysis_tasks_current`
- `analysis_attempts`
- `analysis_results`
- `analysis_dead_letters`

The current `analysis_tasks`, `analysis_attempts`, `analysis_results`, and `analysis_dead_letters`
tables may be retained and evolved as projections if that reduces migration friction. Their semantic
role changes from primary write target to derived read model.

## 7. Event Model

At minimum, the system must represent these event types:

- `task_enqueued`
- `task_dedupe_hit`
- `task_claimed`
- `task_reclaimed`
- `snapshot_loaded`
- `llm_request_built`
- `llm_call_started`
- `llm_call_succeeded`
- `llm_call_failed`
- `schema_validation_failed`
- `task_retry_scheduled`
- `task_succeeded`
- `task_failed`
- `task_dead_lettered`
- `projection_rebuild_requested`
- `api_task_queried`
- `api_attempts_queried`
- `api_timeline_queried`
- `api_artifacts_queried`
- `api_audit_events_queried`

Each event row should contain at least:

- `id`
- `task_id`
- `trace_id`
- `correlation_key`
- `event_type`
- `actor_type`
- `actor_id`
- `step_key`
- `step_version`
- `prompt_key`
- `prompt_version`
- `attempt_no`
- `artifact_id`
- `payload_json`
- `redaction_classification`
- `created_at`

`trace_id` is required even when a task is the only orchestration unit in scope. It will later allow
cross-module correlation without changing the event model.

## 8. Artifact Model

`analysis_artifacts` is the evidence table and should contain at least:

- `id`
- `task_id`
- `trace_id`
- `artifact_type`
- `content_json`
- `content_hash`
- `content_size`
- `schema_version`
- `created_by_event_id`
- `created_at`

Supported artifact types in this phase:

- `input_snapshot`
- `llm_request`
- `llm_raw_response`
- `parsed_output`
- `schema_error`
- `retry_decision`
- `dead_letter_archive`

The artifact table exists to make evidence queryable and reusable without overloading the event row
payload itself. Events describe what happened. Artifacts preserve the larger evidence objects linked
to what happened.

## 9. Projection Model

### 9.1 Task projection

`analysis_tasks_current` exposes the current state for runtime and API queries:

- task metadata
- current status
- current attempt count
- last error summary
- claim or lease metadata
- current projection version

### 9.2 Attempt projection

`analysis_attempts` becomes a query-optimized projection that includes:

- provider and model metadata
- worker metadata
- request and response artifact references
- parsed output artifact references
- terminal attempt status and error summary

### 9.3 Result projection

`analysis_results` remains the business-consumption surface for successful outputs.

### 9.4 Dead-letter projection

`analysis_dead_letters` remains the business and operator surface for terminal failures with archived
evidence.

Projection write failures must never invalidate already committed facts in the ledger. If projection
refresh fails after fact persistence, the ledger remains correct and a rebuild path must exist.

## 10. Transaction Boundaries

The runtime must use explicit transaction boundaries so state does not drift from evidence:

### 10.1 Enqueue transaction

In one transaction:

- insert `task_enqueued`
- insert the input snapshot artifact
- insert or refresh the task projection

If dedupe hits, do not create a second task. Instead:

- insert `task_dedupe_hit`
- return the existing task projection

### 10.2 Claim transaction

In one transaction:

- select one runnable task projection
- write `task_claimed`
- update claim metadata and projected status

### 10.3 Execution evidence transactions

Before and after outbound execution, persist evidence as facts:

- `llm_request_built` plus request artifact
- `llm_call_started`
- `llm_call_succeeded` plus raw response artifact
- `llm_call_failed` plus failure payload evidence when available

### 10.4 Outcome transaction

In one transaction per final execution decision:

- write the decision event
- write related artifacts
- refresh the affected projections
- insert result or dead-letter projection rows when applicable

### 10.5 Query audit transaction

Read endpoints do not change task state, but they should append lightweight audit events so access to
task evidence is itself queryable.

## 11. Runtime Flow

The worker flow becomes:

1. claim a runnable task from the projection
2. load the snapshot artifact
3. append `snapshot_loaded`
4. build the request and persist `llm_request_built`
5. append `llm_call_started`
6. execute the LLM call
7. persist success or failure evidence
8. validate output if a response was produced
9. append one terminal decision event for success, retry, failure, or dead letter
10. refresh projections

The API flow becomes:

1. resolve domain input
2. build task and snapshot
3. append enqueue facts
4. return the current task projection

At no point should worker or API code depend on in-memory mutable state as the authoritative source of
truth.

## 12. Recovery and Idempotency

The durable repository must support restart-safe behavior.

### 12.1 Lease or claim recovery

Running tasks must carry claim metadata. If a worker dies, tasks whose claim has expired may be
reclaimed. Reclaiming must create a distinct event such as `task_reclaimed`.

### 12.2 Idempotent attempt recording

Attempt writes must be idempotent on a stable key such as `task_id + attempt_no + event_type` or an
equivalent deterministic constraint. A restarted worker must not create duplicate terminal outcomes.

### 12.3 Projection rebuild

If projection refresh fails after events are committed, the system must support replaying ledger facts
to rebuild projections. A rebuild trigger itself should be visible as an event.

## 13. Query APIs

The orchestration API surface should be audit-first.

### 13.1 Keep and extend existing task queries

- `GET /analysis/tasks/{task_id}`
- `GET /analysis/tasks/{task_id}/attempts`
- `GET /analysis/results/{task_id}`
- `GET /analysis/dead-letters/{task_id}`

### 13.2 Add audit-focused queries

- `GET /analysis/tasks/{task_id}/timeline`
- `GET /analysis/tasks/{task_id}/artifacts`
- `GET /analysis/audit/events?...`
- `GET /analysis/audit/traces/{trace_id}`

Query filters must support at least:

- `task_id`
- `trace_id`
- `step_key`
- `event_type`
- `from`
- `to`

The response model should answer:

- what happened
- when it happened
- which worker, step, model, and prompt version were involved
- which evidence objects are attached
- why the task is in its current state

## 14. Logging Conventions

Logs must align with persisted facts rather than replace them.

Every critical log line on API and worker paths should carry:

- `trace_id`
- `task_id`
- `event_id`
- `event_type`
- `step_key`
- `prompt_key`
- `prompt_version`
- `attempt_no`
- `worker_id`

This allows an operator to move in either direction:

- from a log line to the persisted event
- from a persisted event to the correlated log line

Missing these correlation fields in critical paths is treated as an observability defect.

## 15. Sensitive Data Handling

This phase stores evidence in original form by default because audit completeness is the first
priority. However, the model must reserve future support for data-governance controls through fields
such as:

- `redaction_classification`
- optional artifact visibility scope
- policy-driven export restrictions

This keeps the design compatible with later redaction work without weakening current forensic value.

## 16. Testing Strategy

The implementation must be verified through:

### 16.1 Repository contract tests

Run the same orchestration repository contract suite against:

- `InMemoryOrchestrationRepository`
- the new PostgreSQL-backed repository

### 16.2 Restart recovery tests

Simulate process interruption after enqueue, after claim, and after outbound execution begins. Confirm
that the durable repository can recover the task or safely classify it for reclaim.

### 16.3 Idempotency tests

Verify that repeated claim attempts, repeated outcome persistence, and repeated projection refreshes do
not create duplicate terminal records or inconsistent task views.

### 16.4 Audit completeness tests

For success, retry, terminal failure, and dead-letter paths, confirm that:

- the full event chain exists
- the required artifacts exist
- the API can retrieve the timeline and artifacts

### 16.5 API query tests

Verify sorting and filtering behavior for timeline, audit-event, and artifact endpoints.

### 16.6 Logging field tests

Critical worker and API paths must emit the required correlation fields.

## 17. Success Criteria

The phase is successful only when:

- the primary runtime path no longer depends on `InMemoryOrchestrationRepository`
- orchestration facts survive process restarts
- every meaningful worker transition is represented by a persisted event
- every LLM attempt has a durable evidence chain
- operators can query task history and evidence directly through API endpoints
- logs consistently correlate with persisted task facts
- projection loss or staleness can be repaired from the ledger

## 18. Files Likely In Scope

Likely code and schema areas affected in the later implementation phase:

- `migrations/`
- `crates/pa-orchestrator/src/repository.rs`
- `crates/pa-orchestrator/src/models.rs`
- `crates/pa-orchestrator/src/worker.rs`
- `crates/pa-orchestrator/src/lib.rs`
- `crates/pa-api/src/analysis.rs`
- `crates/pa-api/src/router.rs`
- `crates/pa-app/src/main.rs`
- `docs/architecture/phase1-runtime.md`

## 19. Risks

Primary risks:

- overcomplicating the first durable repository cut
- allowing projections to quietly drift from ledger facts
- writing events without enough deterministic keys for idempotent recovery
- storing large JSON evidence without practical query boundaries

Mitigations:

- keep the task lifecycle semantics stable
- use append-only facts plus explicit projections
- enforce deterministic uniqueness for attempt-related writes
- keep API queries focused on bounded filters and task-scoped inspection

## 20. Relationship to Existing Specs

This design refines and strengthens `2026-04-23-durable-orchestration-storage-design.md`.

That earlier document correctly scoped the repository replacement, but it did not yet commit to:

- event-ledger-first persistence
- artifact-first evidence storage
- audit-query APIs
- mandatory log-to-event correlation

For the implementation phase, this document should be treated as the more specific design for
orchestration durability and auditability.
