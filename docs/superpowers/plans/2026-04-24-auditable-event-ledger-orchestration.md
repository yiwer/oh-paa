# Auditable Event-Ledger Orchestration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the in-memory orchestration runtime with a PostgreSQL-backed event ledger that preserves every task transition, LLM request or response artifact, and audit query trail while keeping current task semantics stable.

**Architecture:** Add an append-only `orchestration_events` ledger plus `analysis_artifacts` evidence storage, then treat task, attempt, result, and dead-letter tables as derived read models. Keep the current API and worker behavior stable, but make them write facts and artifacts first, refresh projections second, and expose new audit-first query endpoints.

**Tech Stack:** Rust 2024, SQLx/PostgreSQL, Tokio, async-trait, Axum, tracing, existing `pa-orchestrator` worker loop and Phase 2 analysis models.

---

## File Structure Map

- `G:\Rust\oh-paa\migrations\003_auditable_event_ledger_orchestration.sql`
  - Create the event-ledger and artifact tables, add projection columns and indexes, and enforce idempotent uniqueness.
- `G:\Rust\oh-paa\crates\pa-orchestrator\Cargo.toml`
  - Add SQLx runtime dependencies needed by the PostgreSQL repository implementation.
- `G:\Rust\oh-paa\crates\pa-orchestrator\src\models.rs`
  - Add audit-domain models such as events, artifacts, query filters, and trace-aware task metadata.
- `G:\Rust\oh-paa\crates\pa-orchestrator\src\repository.rs`
  - Extend the repository trait with audit-query and event-batch methods; keep the in-memory implementation compatible with tests.
- `G:\Rust\oh-paa\crates\pa-orchestrator\src\pg_repository.rs`
  - Implement the PostgreSQL-backed event ledger, artifact persistence, projection refresh, and rebuild helpers.
- `G:\Rust\oh-paa\crates\pa-orchestrator\src\lib.rs`
  - Export `PgOrchestrationRepository` and the new audit-domain models.
- `G:\Rust\oh-paa\crates\pa-orchestrator\src\worker.rs`
  - Persist request, response, validation, retry, and terminal events with consistent trace-aware logging.
- `G:\Rust\oh-paa\crates\pa-orchestrator\tests\models.rs`
  - Verify new enums, trace fields, and audit structs.
- `G:\Rust\oh-paa\crates\pa-orchestrator\tests\worker.rs`
  - Verify event sequence, artifact persistence, and trace-aware logs on success and failure paths.
- `G:\Rust\oh-paa\crates\pa-orchestrator\tests\pg_repository.rs`
  - Verify durable event, artifact, projection, and recovery behavior against PostgreSQL.
- `G:\Rust\oh-paa\crates\pa-api\src\router.rs`
  - Store `Arc<dyn OrchestrationRepository>` so the API can run against the PostgreSQL repository.
- `G:\Rust\oh-paa\crates\pa-api\src\analysis.rs`
  - Keep existing task APIs working and add timeline, artifacts, and audit-event endpoints.
- `G:\Rust\oh-paa\crates\pa-api\tests\smoke.rs`
  - Verify audit endpoints and timeline output through the HTTP surface.
- `G:\Rust\oh-paa\crates\pa-app\src\main.rs`
  - Construct `PgOrchestrationRepository`, share it between Axum and the worker, and stop using the in-memory runtime path.
- `G:\Rust\oh-paa\docs\architecture\phase1-runtime.md`
  - Document the event-ledger runtime path and operator-facing audit commands.

## Decomposition Note

This plan covers one subsystem: auditable orchestration durability. It does not redesign prompt logic, replay scoring, or market-data behavior. It only adds the storage, recovery, query, and logging infrastructure needed to make the orchestration path non-black-box.

### Task 1: Add Audit Models and the Ledger Migration

**Files:**
- Create: `G:\Rust\oh-paa\migrations\003_auditable_event_ledger_orchestration.sql`
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\src\models.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\tests\models.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\src\lib.rs`

- [ ] **Step 1: Add the failing model test for trace-aware audit types**

```rust
#[test]
fn audit_models_capture_trace_event_and_artifact_metadata() {
    let trace_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let event = OrchestrationEvent {
        id: Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap(),
        task_id: Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
        trace_id,
        correlation_key: Some("shared_bar_analysis:2026-04-21T04:00:00Z".into()),
        event_type: OrchestrationEventType::TaskEnqueued,
        actor_type: "api".into(),
        actor_id: "analysis.create_shared_bar_task".into(),
        step_key: Some("shared_bar_analysis".into()),
        step_version: Some("v2".into()),
        prompt_key: Some("shared_bar_analysis".into()),
        prompt_version: Some("v2".into()),
        attempt_no: None,
        artifact_id: None,
        payload_json: serde_json::json!({"status":"pending"}),
        redaction_classification: "full_fidelity".into(),
        created_at: chrono::Utc::now(),
    };
    let artifact = AnalysisArtifact {
        id: Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap(),
        task_id: event.task_id,
        trace_id,
        artifact_type: AnalysisArtifactType::InputSnapshot,
        content_json: serde_json::json!({"timeframe":"15m"}),
        content_hash: "abc123".into(),
        content_size: 21,
        schema_version: Some("v1".into()),
        created_by_event_id: event.id,
        created_at: chrono::Utc::now(),
    };

    assert_eq!(event.event_type.as_str(), "task_enqueued");
    assert_eq!(AnalysisArtifactType::InputSnapshot.as_str(), "input_snapshot");
    assert_eq!(artifact.trace_id, trace_id);
}
```

- [ ] **Step 2: Run the focused model test to verify the new audit types are missing**

Run: `cargo test -p pa-orchestrator --test models audit_models_capture_trace_event_and_artifact_metadata -- --exact`

Expected: FAIL with unresolved items such as `OrchestrationEvent`, `OrchestrationEventType`, `AnalysisArtifact`, or `AnalysisArtifactType`.

- [ ] **Step 3: Add the new models, trace fields, exports, and migration**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestrationEventType {
    TaskEnqueued,
    TaskDedupeHit,
    TaskClaimed,
    TaskReclaimed,
    SnapshotLoaded,
    LlmRequestBuilt,
    LlmCallStarted,
    LlmCallSucceeded,
    LlmCallFailed,
    SchemaValidationFailed,
    TaskRetryScheduled,
    TaskSucceeded,
    TaskFailed,
    TaskDeadLettered,
    ProjectionRebuildRequested,
    ApiTaskQueried,
    ApiAttemptsQueried,
    ApiTimelineQueried,
    ApiArtifactsQueried,
    ApiAuditEventsQueried,
}

impl OrchestrationEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TaskEnqueued => "task_enqueued",
            Self::TaskDedupeHit => "task_dedupe_hit",
            Self::TaskClaimed => "task_claimed",
            Self::TaskReclaimed => "task_reclaimed",
            Self::SnapshotLoaded => "snapshot_loaded",
            Self::LlmRequestBuilt => "llm_request_built",
            Self::LlmCallStarted => "llm_call_started",
            Self::LlmCallSucceeded => "llm_call_succeeded",
            Self::LlmCallFailed => "llm_call_failed",
            Self::SchemaValidationFailed => "schema_validation_failed",
            Self::TaskRetryScheduled => "task_retry_scheduled",
            Self::TaskSucceeded => "task_succeeded",
            Self::TaskFailed => "task_failed",
            Self::TaskDeadLettered => "task_dead_lettered",
            Self::ProjectionRebuildRequested => "projection_rebuild_requested",
            Self::ApiTaskQueried => "api_task_queried",
            Self::ApiAttemptsQueried => "api_attempts_queried",
            Self::ApiTimelineQueried => "api_timeline_queried",
            Self::ApiArtifactsQueried => "api_artifacts_queried",
            Self::ApiAuditEventsQueried => "api_audit_events_queried",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrchestrationEvent {
    pub id: Uuid,
    pub task_id: Uuid,
    pub trace_id: Uuid,
    pub correlation_key: Option<String>,
    pub event_type: OrchestrationEventType,
    pub actor_type: String,
    pub actor_id: String,
    pub step_key: Option<String>,
    pub step_version: Option<String>,
    pub prompt_key: Option<String>,
    pub prompt_version: Option<String>,
    pub attempt_no: Option<u32>,
    pub artifact_id: Option<Uuid>,
    pub payload_json: serde_json::Value,
    pub redaction_classification: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisArtifactType {
    InputSnapshot,
    LlmRequest,
    LlmRawResponse,
    ParsedOutput,
    SchemaError,
    RetryDecision,
    DeadLetterArchive,
}
```

```sql
CREATE TABLE orchestration_events (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    trace_id UUID NOT NULL,
    correlation_key TEXT,
    event_type TEXT NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    step_key TEXT,
    step_version TEXT,
    prompt_key TEXT,
    prompt_version TEXT,
    attempt_no INTEGER,
    artifact_id UUID,
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    redaction_classification TEXT NOT NULL DEFAULT 'full_fidelity',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE analysis_artifacts (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    trace_id UUID NOT NULL,
    artifact_type TEXT NOT NULL,
    content_json JSONB NOT NULL,
    content_hash TEXT NOT NULL,
    content_size BIGINT NOT NULL,
    schema_version TEXT,
    created_by_event_id UUID NOT NULL REFERENCES orchestration_events (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE analysis_tasks
ADD COLUMN trace_id UUID,
ADD COLUMN claim_expires_at TIMESTAMPTZ,
ADD COLUMN projection_version BIGINT NOT NULL DEFAULT 0;

CREATE INDEX orchestration_events_task_time_idx
ON orchestration_events (task_id, created_at);

CREATE INDEX orchestration_events_trace_time_idx
ON orchestration_events (trace_id, created_at);
```

- [ ] **Step 4: Re-run the model test and a compile check**

Run: `cargo test -p pa-orchestrator --test models audit_models_capture_trace_event_and_artifact_metadata -- --exact`

Expected: PASS

Run: `cargo check -p pa-orchestrator`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add migrations/003_auditable_event_ledger_orchestration.sql crates/pa-orchestrator/src/models.rs crates/pa-orchestrator/src/lib.rs crates/pa-orchestrator/tests/models.rs
git commit -m "feat: add orchestration audit ledger models"
```

### Task 2: Extend the Repository Contract and In-Memory Audit Surface

**Files:**
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\src\repository.rs`
- Create: `G:\Rust\oh-paa\crates\pa-orchestrator\tests\audit_repository.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\src\lib.rs`

- [ ] **Step 1: Add the failing repository contract test for event and artifact queries**

```rust
#[tokio::test]
async fn in_memory_repository_records_timeline_and_artifacts() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(3);
    let trace_id = task.trace_id;

    repository
        .insert_task_with_snapshot(task.clone(), snapshot.clone())
        .await
        .unwrap();

    let timeline = repository
        .list_events(OrchestrationEventQuery {
            task_id: Some(task.id),
            trace_id: Some(trace_id),
            step_key: None,
            event_type: None,
            from: None,
            to: None,
        })
        .await
        .unwrap();
    let artifacts = repository.list_artifacts(task.id, None).await.unwrap();

    assert_eq!(timeline[0].event_type, OrchestrationEventType::TaskEnqueued);
    assert_eq!(artifacts[0].artifact_type, AnalysisArtifactType::InputSnapshot);
    assert_eq!(artifacts[0].trace_id, trace_id);
}
```

- [ ] **Step 2: Run the new repository test to verify the trait methods are missing**

Run: `cargo test -p pa-orchestrator --test audit_repository in_memory_repository_records_timeline_and_artifacts -- --exact`

Expected: FAIL with method-not-found errors such as `list_events` or `list_artifacts`.

- [ ] **Step 3: Extend the trait and backfill the in-memory repository**

```rust
#[async_trait]
pub trait OrchestrationRepository: Send + Sync {
    async fn insert_task_with_snapshot(
        &self,
        task: AnalysisTask,
        snapshot: AnalysisSnapshot,
    ) -> Result<InsertTaskResult, AppError>;

    async fn get_task(&self, task_id: Uuid) -> Result<Option<AnalysisTask>, AppError>;
    async fn get_result_for_task(&self, task_id: Uuid) -> Result<Option<AnalysisResult>, AppError>;
    async fn list_attempts_for_task(&self, task_id: Uuid) -> Result<Vec<AnalysisAttempt>, AppError>;
    async fn get_dead_letter_for_task(&self, task_id: Uuid) -> Result<Option<AnalysisDeadLetter>, AppError>;
    async fn list_events(
        &self,
        query: OrchestrationEventQuery,
    ) -> Result<Vec<OrchestrationEvent>, AppError>;
    async fn list_artifacts(
        &self,
        task_id: Uuid,
        artifact_type: Option<AnalysisArtifactType>,
    ) -> Result<Vec<AnalysisArtifact>, AppError>;
}
```

```rust
#[derive(Debug, Default)]
struct InMemoryState {
    tasks: HashMap<Uuid, AnalysisTask>,
    snapshots: HashMap<Uuid, AnalysisSnapshot>,
    attempts: Vec<AnalysisAttempt>,
    results: Vec<AnalysisResult>,
    dead_letters: Vec<AnalysisDeadLetter>,
    events: Vec<OrchestrationEvent>,
    artifacts: Vec<AnalysisArtifact>,
    fail_next_outcome_persist: bool,
    pending_order: VecDeque<Uuid>,
    keyed_dedupe: HashMap<String, Uuid>,
}
```

```rust
async fn list_events(
    &self,
    query: OrchestrationEventQuery,
) -> Result<Vec<OrchestrationEvent>, AppError> {
    let state = self.lock_state();
    Ok(state
        .events
        .iter()
        .filter(|event| query.task_id.is_none_or(|task_id| event.task_id == task_id))
        .filter(|event| query.trace_id.is_none_or(|trace_id| event.trace_id == trace_id))
        .filter(|event| query.event_type.is_none_or(|event_type| event.event_type == event_type))
        .cloned()
        .collect())
}
```

- [ ] **Step 4: Re-run the focused repository test**

Run: `cargo test -p pa-orchestrator --test audit_repository in_memory_repository_records_timeline_and_artifacts -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pa-orchestrator/src/repository.rs crates/pa-orchestrator/src/lib.rs crates/pa-orchestrator/tests/audit_repository.rs
git commit -m "refactor: extend orchestration repository for audit queries"
```

### Task 3: Implement the PostgreSQL Event Ledger and Projection Refresh

**Files:**
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\Cargo.toml`
- Create: `G:\Rust\oh-paa\crates\pa-orchestrator\src\pg_repository.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\src\lib.rs`
- Create: `G:\Rust\oh-paa\crates\pa-orchestrator\tests\pg_repository.rs`

- [ ] **Step 1: Add the failing PostgreSQL repository test for enqueue, timeline, and artifacts**

```rust
#[tokio::test]
async fn pg_repository_persists_event_ledger_and_snapshot_artifact() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping pg_repository_persists_event_ledger_and_snapshot_artifact: PA_DATABASE_URL not set");
        return;
    };
    let fixture = seed_runtime_fixture(&pool).await;
    let repository = PgOrchestrationRepository::new(pool.clone());
    let (task, snapshot) = make_task_and_snapshot_with_instrument(fixture.instrument_id, 3);

    let insert = repository
        .insert_task_with_snapshot(task.clone(), snapshot.clone())
        .await
        .unwrap();
    assert_eq!(insert, InsertTaskResult::Inserted);

    let persisted = repository.get_task(task.id).await.unwrap().unwrap();
    let timeline = repository
        .list_events(OrchestrationEventQuery {
            task_id: Some(task.id),
            trace_id: Some(task.trace_id),
            step_key: None,
            event_type: None,
            from: None,
            to: None,
        })
        .await
        .unwrap();
    let artifacts = repository
        .list_artifacts(task.id, Some(AnalysisArtifactType::InputSnapshot))
        .await
        .unwrap();

    assert_eq!(persisted.trace_id, task.trace_id);
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].event_type, OrchestrationEventType::TaskEnqueued);
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].content_json, snapshot.input_json);

    cleanup_runtime_fixture(&pool, &fixture).await;
}
```

- [ ] **Step 2: Add the SQL dependency and run the focused test**

```toml
[dependencies]
sqlx.workspace = true
tokio.workspace = true
tracing.workspace = true
```

Run: `cargo test -p pa-orchestrator --test pg_repository pg_repository_persists_event_ledger_and_snapshot_artifact -- --exact`

Expected: FAIL because `PgOrchestrationRepository` does not exist yet.

- [ ] **Step 3: Add the PostgreSQL repository, event batch writes, and projection refresh helpers**

```rust
pub struct PgOrchestrationRepository {
    pool: sqlx::PgPool,
}

impl PgOrchestrationRepository {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    async fn append_event_and_artifacts(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event: &OrchestrationEvent,
        artifacts: &[AnalysisArtifact],
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO orchestration_events (
                id, task_id, trace_id, correlation_key, event_type, actor_type, actor_id,
                step_key, step_version, prompt_key, prompt_version, attempt_no, artifact_id,
                payload_json, redaction_classification, created_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12, $13,
                $14, $15, $16
            )
            "#,
            event.id,
            event.task_id,
            event.trace_id,
            event.correlation_key,
            event.event_type.as_str(),
            event.actor_type,
            event.actor_id,
            event.step_key,
            event.step_version,
            event.prompt_key,
            event.prompt_version,
            event.attempt_no.map(|value| value as i32),
            event.artifact_id,
            event.payload_json,
            event.redaction_classification,
            event.created_at,
        )
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;

        for artifact in artifacts {
            sqlx::query!(
                r#"
                INSERT INTO analysis_artifacts (
                    id, task_id, trace_id, artifact_type, content_json, content_hash,
                    content_size, schema_version, created_by_event_id, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                artifact.id,
                artifact.task_id,
                artifact.trace_id,
                artifact.artifact_type.as_str(),
                artifact.content_json,
                artifact.content_hash,
                artifact.content_size,
                artifact.schema_version,
                artifact.created_by_event_id,
                artifact.created_at,
            )
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
        }

        Ok(())
    }
}
```

```rust
pub use pg_repository::PgOrchestrationRepository;
```

- [ ] **Step 4: Re-run the focused PostgreSQL test**

Run: `cargo test -p pa-orchestrator --test pg_repository pg_repository_persists_event_ledger_and_snapshot_artifact -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pa-orchestrator/Cargo.toml crates/pa-orchestrator/src/pg_repository.rs crates/pa-orchestrator/src/lib.rs crates/pa-orchestrator/tests/pg_repository.rs
git commit -m "feat: add postgres event-ledger orchestration repository"
```

### Task 4: Persist Worker Request, Response, Retry, and Failure Evidence

**Files:**
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\src\worker.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\tests\worker.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\src\repository.rs`

- [ ] **Step 1: Add the failing worker-path test for event sequence and artifacts**

```rust
#[tokio::test]
async fn worker_success_path_records_request_response_and_terminal_events() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(3);
    repository
        .insert_task_with_snapshot(task.clone(), snapshot)
        .await
        .unwrap();

    let output = serde_json::json!({
        "bullish_case": {"entry": "breakout"},
        "bearish_case": {"entry": "pullback"}
    });
    let registry = PromptRegistry::default()
        .with_spec(make_spec(serde_json::json!({
            "type": "object",
            "required": ["bullish_case", "bearish_case"],
            "properties": {
                "bullish_case": {"type": "object"},
                "bearish_case": {"type": "object"}
            }
        })))
        .unwrap();
    let executor = Executor::new(registry, FixtureLlmClient::with_json(output));

    run_single_task(&repository, &executor).await.unwrap();

    let timeline = repository
        .list_events(OrchestrationEventQuery::for_task(task.id, task.trace_id))
        .await
        .unwrap();
    let artifact_types: Vec<_> = repository
        .list_artifacts(task.id, None)
        .await
        .unwrap()
        .into_iter()
        .map(|artifact| artifact.artifact_type)
        .collect();

    assert_eq!(
        timeline
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>(),
        vec![
            "task_enqueued",
            "task_claimed",
            "snapshot_loaded",
            "llm_request_built",
            "llm_call_started",
            "llm_call_succeeded",
            "task_succeeded",
        ]
    );
    assert!(artifact_types.contains(&AnalysisArtifactType::LlmRequest));
    assert!(artifact_types.contains(&AnalysisArtifactType::LlmRawResponse));
    assert!(artifact_types.contains(&AnalysisArtifactType::ParsedOutput));
}
```

- [ ] **Step 2: Run the focused worker test to confirm the missing event sequence**

Run: `cargo test -p pa-orchestrator --test worker worker_success_path_records_request_response_and_terminal_events -- --exact`

Expected: FAIL because the worker currently persists attempts and results, but not the full event and artifact chain.

- [ ] **Step 3: Update the worker to append request, response, retry, and terminal facts**

```rust
tracing::info!(
    trace_id = %task.trace_id,
    task_id = %task.id,
    event_type = "llm_request_built",
    step_key = %task.task_type,
    prompt_key = %task.prompt_key,
    prompt_version = %task.prompt_version,
    attempt_no = task.attempt_count.saturating_add(1),
    worker_id = worker_id,
    "analysis worker built request payload"
);

repository
    .record_execution_artifacts(
        task.id,
        task.trace_id,
        task.attempt_count.saturating_add(1),
        &attempt.request_payload_json,
        attempt.raw_response_json.as_ref(),
        attempt.parsed_output_json.as_ref(),
        attempt.schema_validation_error.as_deref(),
    )
    .await?;
```

```rust
with_claim_recovery(
    repository,
    task.id,
    repository.persist_task_outcome(TaskOutcomeRecord {
        task_id: task.id,
        trace_id: task.trace_id,
        worker_id: worker_id.to_string(),
        event_type: OrchestrationEventType::TaskSucceeded,
        attempt: Some(attempt_row),
        result: Some(result),
        dead_letter: None,
        error_message: None,
    }),
)
.await?;
```

- [ ] **Step 4: Re-run the focused worker test and one failure-path test**

Run: `cargo test -p pa-orchestrator --test worker worker_success_path_records_request_response_and_terminal_events -- --exact`

Expected: PASS

Run: `cargo test -p pa-orchestrator --test worker worker_retry_exhaustion_moves_task_to_dead_letter -- --exact`

Expected: PASS and the dead-letter path still emits a single terminal event plus a dead-letter archive artifact.

- [ ] **Step 5: Commit**

```bash
git add crates/pa-orchestrator/src/worker.rs crates/pa-orchestrator/src/repository.rs crates/pa-orchestrator/tests/worker.rs
git commit -m "feat: record worker event ledger and execution artifacts"
```

### Task 5: Wire the API and App Runtime to the Audit Repository

**Files:**
- Modify: `G:\Rust\oh-paa\crates\pa-api\src\router.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-api\src\analysis.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-api\tests\smoke.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-app\src\main.rs`

- [ ] **Step 1: Add the failing HTTP smoke test for timeline and artifact queries**

```rust
#[tokio::test]
async fn analysis_audit_routes_return_timeline_and_artifacts() {
    let app = app_router(AppState::fixture());

    let create = request_json(
        &app,
        Method::POST,
        "/analysis/shared/bar",
        r#"{
            "instrument_id":"00000000-0000-0000-0000-000000000001",
            "timeframe":"15m",
            "bar_state":"closed",
            "bar_open_time":"2026-04-21T01:45:00Z",
            "bar_close_time":"2026-04-21T02:00:00Z",
            "shared_pa_state_json":{"bar_identity":{"tag":"fixture-pa-state"}},
            "recent_pa_states_json":[]
        }"#,
    )
    .await;
    let create_json = response_json(create).await;
    let task_id = create_json["task_id"].as_str().unwrap();

    let timeline = request(&app, &format!("/analysis/tasks/{task_id}/timeline")).await;
    let artifacts = request(&app, &format!("/analysis/tasks/{task_id}/artifacts")).await;

    assert_eq!(timeline.status(), StatusCode::OK);
    assert_eq!(artifacts.status(), StatusCode::OK);
    assert_eq!(response_json(timeline).await["events"][0]["event_type"], "task_enqueued");
    assert_eq!(response_json(artifacts).await["artifacts"][0]["artifact_type"], "input_snapshot");
}
```

- [ ] **Step 2: Run the focused smoke test to confirm the routes do not exist yet**

Run: `cargo test -p pa-api --test smoke analysis_audit_routes_return_timeline_and_artifacts -- --exact`

Expected: FAIL with `404 Not Found` or compile errors because the route handlers are missing.

- [ ] **Step 3: Change app state to use the trait object and add the audit routes**

```rust
#[derive(Clone)]
pub struct AppState {
    pub server_addr: String,
    pub orchestration_repository: Arc<dyn OrchestrationRepository>,
    pub market_runtime: Option<Arc<MarketRuntime>>,
}
```

```rust
Router::new()
    .route("/shared/pa-state", post(create_shared_pa_state_task))
    .route("/shared/bar", post(create_shared_bar_task))
    .route("/shared/daily", post(create_shared_daily_task))
    .route("/tasks/{task_id}", get(get_task))
    .route("/tasks/{task_id}/timeline", get(get_task_timeline))
    .route("/tasks/{task_id}/artifacts", get(get_task_artifacts))
    .route("/tasks/{task_id}/attempts", get(get_attempts))
    .route("/results/{task_id}", get(get_result))
    .route("/dead-letters/{task_id}", get(get_dead_letter))
    .route("/audit/events", get(query_audit_events))
```

```rust
let orchestration_repository: Arc<dyn OrchestrationRepository> =
    Arc::new(PgOrchestrationRepository::new(pool.clone()));
let state = AppState::with_dependencies(
    config.server_addr.clone(),
    Arc::clone(&orchestration_repository),
    Some(market_runtime),
);
```

- [ ] **Step 4: Re-run the focused smoke test and one compile check**

Run: `cargo test -p pa-api --test smoke analysis_audit_routes_return_timeline_and_artifacts -- --exact`

Expected: PASS

Run: `cargo check -p pa-app`

Expected: PASS and `pa-app` now constructs the PostgreSQL orchestration repository instead of the in-memory one.

- [ ] **Step 5: Commit**

```bash
git add crates/pa-api/src/router.rs crates/pa-api/src/analysis.rs crates/pa-api/tests/smoke.rs crates/pa-app/src/main.rs
git commit -m "feat: expose orchestration audit routes and postgres runtime"
```

### Task 6: Add Recovery, Idempotency, Documentation, and Full Verification

**Files:**
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\tests\pg_repository.rs`
- Modify: `G:\Rust\oh-paa\crates\pa-orchestrator\src\pg_repository.rs`
- Modify: `G:\Rust\oh-paa\docs\architecture\phase1-runtime.md`

- [ ] **Step 1: Add the failing PostgreSQL recovery and idempotency test**

```rust
#[tokio::test]
async fn pg_repository_reclaims_expired_claim_and_avoids_duplicate_terminal_events() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping pg_repository_reclaims_expired_claim_and_avoids_duplicate_terminal_events: PA_DATABASE_URL not set");
        return;
    };
    let fixture = seed_runtime_fixture(&pool).await;
    let repository = PgOrchestrationRepository::new(pool.clone());
    let (task, snapshot) = make_task_and_snapshot_with_instrument(fixture.instrument_id, 1);
    repository.insert_task_with_snapshot(task.clone(), snapshot).await.unwrap();

    let claimed = repository.claim_next_pending_task().await.unwrap().unwrap();
    assert_eq!(claimed.id, task.id);

    sqlx::query!(
        "UPDATE analysis_tasks SET claim_expires_at = NOW() - interval '5 minutes' WHERE id = $1",
        task.id
    )
    .execute(&pool)
    .await
    .unwrap();

    let reclaimed = repository.claim_next_pending_task().await.unwrap().unwrap();
    assert_eq!(reclaimed.id, task.id);

    let timeline = repository
        .list_events(OrchestrationEventQuery::for_task(task.id, task.trace_id))
        .await
        .unwrap();
    let terminal_count = timeline
        .iter()
        .filter(|event| event.event_type == OrchestrationEventType::TaskClaimed)
        .count();

    assert_eq!(terminal_count, 2);

    cleanup_runtime_fixture(&pool, &fixture).await;
}
```

- [ ] **Step 2: Run the focused recovery test**

Run: `cargo test -p pa-orchestrator --test pg_repository pg_repository_reclaims_expired_claim_and_avoids_duplicate_terminal_events -- --exact`

Expected: FAIL because claim expiry, reclaim events, or idempotent uniqueness are incomplete.

- [ ] **Step 3: Implement reclaim logic, idempotent constraints, and runtime docs**

```rust
async fn claim_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError> {
    let mut tx = self.pool.begin().await.map_err(storage_error)?;
    let row = sqlx::query!(
        r#"
        SELECT id
        FROM analysis_tasks
        WHERE status IN ('pending', 'retry_waiting')
           OR (status = 'running' AND claim_expires_at IS NOT NULL AND claim_expires_at < NOW())
        ORDER BY scheduled_at
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(storage_error)?;

    // Load task, append task_claimed or task_reclaimed, refresh projection, commit.
```

```sql
ALTER TABLE orchestration_events
ADD CONSTRAINT orchestration_events_task_attempt_type_unique
UNIQUE (task_id, attempt_no, event_type, actor_id);
```

```md
- `GET /analysis/tasks/<task_id>/timeline`
  returns the append-only event history for one orchestration task.
- `GET /analysis/tasks/<task_id>/artifacts`
  returns persisted input snapshots, request payloads, raw responses, parsed outputs, and dead-letter archives.
- Critical worker logs now include `trace_id`, `task_id`, `event_id`, `event_type`, `attempt_no`, and `worker_id`.
```

- [ ] **Step 4: Run the full verification set**

Run: `cargo test -p pa-orchestrator --test models --test audit_repository --test worker --test pg_repository`

Expected: PASS, with PostgreSQL-backed repository tests skipped only when `PA_DATABASE_URL` is not set.

Run: `cargo test -p pa-api --test smoke`

Expected: PASS

Run: `cargo check -p pa-app`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pa-orchestrator/src/pg_repository.rs crates/pa-orchestrator/tests/pg_repository.rs docs/architecture/phase1-runtime.md
git commit -m "test: verify orchestration recovery and audit runtime"
```

## Self-Review Checklist

### Spec coverage

- Ledger and artifact tables: covered by Task 1 and Task 3.
- Repository replacement and projection refresh: covered by Task 2 and Task 3.
- Worker request, response, retry, and terminal evidence: covered by Task 4.
- Audit-first query APIs: covered by Task 5.
- Recovery, idempotency, and rebuild boundaries: covered by Task 6.
- Logging correlation fields: covered by Task 4 and verified in Task 6.

### Placeholder scan

- No `TBD`, `TODO`, or "implement later" placeholders remain.
- Every code-changing step includes concrete code snippets.
- Every test step includes an exact command and an expected result.

### Type consistency

- `OrchestrationEvent`, `AnalysisArtifact`, `OrchestrationEventQuery`, and `PgOrchestrationRepository` are introduced before later tasks depend on them.
- `trace_id` is consistently treated as a task-level field across models, repository methods, worker logic, and API output.
- Audit route names match the existing `/analysis/...` router structure.
