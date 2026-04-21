# Phase 2 LLM Analysis Orchestration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a durable, code-versioned LLM analysis orchestration layer with task persistence, full input snapshots, structured JSON validation, retry/dead-letter handling, and enqueue/query APIs for shared and user-specific analysis.

**Architecture:** Add a focused `pa-orchestrator` crate that owns task execution, snapshot persistence, attempts, retry policy, and worker behavior. Keep PA semantics in `pa-analysis` and `pa-user` by defining `PromptSpec`s, typed input/output contracts, and task factories there, then wire the new orchestration flow into `pa-api` and `pa-app`.

**Tech Stack:** Rust 2024, Tokio, Axum, SQLx + PostgreSQL, Serde, Serde JSON, Chrono, UUID, Async Trait, Tracing, `jsonschema`.

---

## File Structure Map

- `E:\rust-app\oh-paa\Cargo.toml`
  - Add `crates/pa-orchestrator` and any new workspace dependencies such as `jsonschema`.
- `E:\rust-app\oh-paa\migrations\002_phase2_analysis_orchestration.sql`
  - Create orchestration tables for tasks, snapshots, attempts, results, and dead letters.
- `E:\rust-app\oh-paa\crates\pa-core\src\error.rs`
  - Add orchestration-oriented error codes and retry classification helpers used across crates.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\Cargo.toml`
  - New crate manifest.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\lib.rs`
  - Re-export orchestrator public surface.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\models.rs`
  - Define task, snapshot, attempt, result, dead-letter, enums, and prompt contract types.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\repository.rs`
  - Repository trait plus in-memory implementation for tests.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\dedupe.rs`
  - Dedupe-key builders for shared and user analysis tasks.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\prompt_registry.rs`
  - Registry for `PromptSpec` lookup by `prompt_key + prompt_version`.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\llm.rs`
  - LLM client abstraction and fixture client.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\executor.rs`
  - Snapshot-driven execution plus JSON schema validation.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\retry.rs`
  - Retry policy classification and state transition helpers.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\worker.rs`
  - Worker loop and single-task execution orchestration.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\dedupe.rs`
  - Task identity and deduplication tests.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\executor.rs`
  - Prompt registry and schema-validation tests.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\worker.rs`
  - Retry, dead-letter, and result-persistence tests.
- `E:\rust-app\oh-paa\crates\pa-analysis\src\lib.rs`
  - Re-export prompt and task-factory surface.
- `E:\rust-app\oh-paa\crates\pa-analysis\src\models.rs`
  - Add typed shared prompt input/output models.
- `E:\rust-app\oh-paa\crates\pa-analysis\src\prompt_specs.rs`
  - Define `shared_bar_analysis_v1` and `shared_daily_context_v1`.
- `E:\rust-app\oh-paa\crates\pa-analysis\src\task_factory.rs`
  - Build snapshot-backed tasks for shared `open`, `closed`, and daily analysis.
- `E:\rust-app\oh-paa\crates\pa-analysis\tests\task_factory.rs`
  - Verify dedupe and snapshot shape for shared analysis tasks.
- `E:\rust-app\oh-paa\crates\pa-user\src\lib.rs`
  - Re-export user prompt and task-factory surface.
- `E:\rust-app\oh-paa\crates\pa-user\src\models.rs`
  - Add typed user prompt input/output models.
- `E:\rust-app\oh-paa\crates\pa-user\src\prompt_specs.rs`
  - Define `user_position_advice_v1`.
- `E:\rust-app\oh-paa\crates\pa-user\src\task_factory.rs`
  - Build snapshot-backed tasks for manual and scheduled user analysis.
- `E:\rust-app\oh-paa\crates\pa-user\tests\task_factory.rs`
  - Verify `open/closed` dedupe behavior and snapshot completeness.
- `E:\rust-app\oh-paa\crates\pa-api\src\analysis.rs`
  - Replace placeholder route with enqueue/query handlers for shared analysis tasks.
- `E:\rust-app\oh-paa\crates\pa-api\src\user.rs`
  - Replace placeholder route with manual-analysis enqueue handlers.
- `E:\rust-app\oh-paa\crates\pa-api\src\router.rs`
  - Add application state dependencies needed by new handlers.
- `E:\rust-app\oh-paa\crates\pa-api\tests\smoke.rs`
  - Extend smoke coverage to new analysis routes and task-query responses.
- `E:\rust-app\oh-paa\crates\pa-app\src\main.rs`
  - Register prompt specs, start worker(s), and wire repositories.
- `E:\rust-app\oh-paa\docs\architecture\phase1-runtime.md`
  - Update runtime notes with Phase 2 worker behavior and dead-letter expectations.

## Decomposition Note

This plan is intentionally one plan for one subsystem: durable LLM analysis orchestration. It touches multiple crates, but all changes serve the same end-to-end pipeline:

`domain task factory -> orchestration task persistence -> snapshot-backed worker execution -> result/dead-letter query`

The tasks below preserve that flow while keeping each increment independently testable.

### Task 1: Add the Orchestration Schema and Shared Core Enums

**Files:**
- Modify: `E:\rust-app\oh-paa\Cargo.toml`
- Modify: `E:\rust-app\oh-paa\crates\pa-core\src\error.rs`
- Create: `E:\rust-app\oh-paa\migrations\002_phase2_analysis_orchestration.sql`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\Cargo.toml`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\models.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\models.rs`

- [ ] **Step 1: Write the failing task-status and prompt-contract test**

```rust
use pa_orchestrator::{
    AnalysisBarState, AnalysisTaskStatus, PromptResultSemantics, PromptSpec,
    RetryPolicyClass,
};

#[test]
fn prompt_spec_exposes_phase2_contract_fields() {
    let spec = PromptSpec {
        prompt_key: "shared_bar_analysis".into(),
        prompt_version: "v1".into(),
        task_type: "shared_bar_analysis".into(),
        system_prompt: "Return JSON only".into(),
        input_schema_version: "v1".into(),
        output_schema_version: "v1".into(),
        output_json_schema: serde_json::json!({"type": "object"}),
        retry_policy_class: RetryPolicyClass::LlmStructuredOutput,
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Closed, AnalysisBarState::Open],
    };

    assert_eq!(AnalysisTaskStatus::Pending.as_str(), "pending");
    assert_eq!(AnalysisTaskStatus::RetryWaiting.as_str(), "retry_waiting");
    assert_eq!(spec.prompt_version, "v1");
    assert_eq!(spec.bar_state_support.len(), 2);
}
```

- [ ] **Step 2: Run the test to verify it fails because the crate does not exist yet**

Run: `cargo test -p pa-orchestrator prompt_spec_exposes_phase2_contract_fields -- --exact`

Expected: FAIL with a package-not-found error for `pa-orchestrator`

- [ ] **Step 3: Add the workspace member and new dependency**

```toml
[workspace]
members = [
    "crates/pa-api",
    "crates/pa-analysis",
    "crates/pa-app",
    "crates/pa-core",
    "crates/pa-instrument",
    "crates/pa-market",
    "crates/pa-orchestrator",
    "crates/pa-user",
]

[workspace.dependencies]
jsonschema = "0.28"
```

- [ ] **Step 4: Create the orchestration migration**

```sql
CREATE TABLE analysis_tasks (
    id UUID PRIMARY KEY,
    task_type TEXT NOT NULL CHECK (length(trim(task_type)) > 0),
    status TEXT NOT NULL CHECK (
        status IN (
            'pending',
            'running',
            'retry_waiting',
            'succeeded',
            'failed',
            'dead_letter',
            'cancelled'
        )
    ),
    instrument_id UUID NOT NULL REFERENCES instruments (id) ON DELETE CASCADE,
    user_id UUID,
    timeframe TEXT CHECK (timeframe IS NULL OR length(trim(timeframe)) > 0),
    bar_state TEXT NOT NULL CHECK (bar_state IN ('none', 'open', 'closed')),
    bar_open_time TIMESTAMPTZ,
    bar_close_time TIMESTAMPTZ,
    trading_date DATE,
    trigger_type TEXT NOT NULL CHECK (length(trim(trigger_type)) > 0),
    prompt_key TEXT NOT NULL CHECK (length(trim(prompt_key)) > 0),
    prompt_version TEXT NOT NULL CHECK (length(trim(prompt_version)) > 0),
    snapshot_id UUID NOT NULL UNIQUE,
    dedupe_key TEXT CHECK (dedupe_key IS NULL OR length(trim(dedupe_key)) > 0),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts INTEGER NOT NULL CHECK (max_attempts > 0),
    scheduled_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    last_error_code TEXT,
    last_error_message TEXT,
    CHECK (attempt_count <= max_attempts),
    CHECK (finished_at IS NULL OR started_at IS NULL OR finished_at >= started_at)
);

CREATE TABLE analysis_snapshots (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL UNIQUE REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    input_json JSONB NOT NULL,
    input_hash TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT analysis_snapshots_task_id_id_unique UNIQUE (task_id, id)
);

CREATE TABLE analysis_attempts (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL CHECK (attempt_no > 0),
    worker_id TEXT NOT NULL CHECK (length(trim(worker_id)) > 0),
    llm_provider TEXT NOT NULL CHECK (length(trim(llm_provider)) > 0),
    model TEXT NOT NULL CHECK (length(trim(model)) > 0),
    request_payload_json JSONB NOT NULL,
    raw_response_json JSONB,
    parsed_output_json JSONB,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'cancelled')),
    error_type TEXT,
    error_message TEXT,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    CONSTRAINT analysis_attempts_task_attempt_unique UNIQUE (task_id, attempt_no),
    CHECK (finished_at IS NULL OR finished_at >= started_at)
);

CREATE TABLE analysis_results (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL UNIQUE REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    task_type TEXT NOT NULL CHECK (length(trim(task_type)) > 0),
    instrument_id UUID NOT NULL REFERENCES instruments (id) ON DELETE CASCADE,
    user_id UUID,
    timeframe TEXT CHECK (timeframe IS NULL OR length(trim(timeframe)) > 0),
    bar_state TEXT NOT NULL CHECK (bar_state IN ('none', 'open', 'closed')),
    bar_open_time TIMESTAMPTZ,
    bar_close_time TIMESTAMPTZ,
    trading_date DATE,
    prompt_key TEXT NOT NULL CHECK (length(trim(prompt_key)) > 0),
    prompt_version TEXT NOT NULL CHECK (length(trim(prompt_version)) > 0),
    output_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE analysis_dead_letters (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL UNIQUE REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    final_error_type TEXT NOT NULL CHECK (length(trim(final_error_type)) > 0),
    final_error_message TEXT NOT NULL CHECK (length(trim(final_error_message)) > 0),
    last_attempt_id UUID REFERENCES analysis_attempts (id) ON DELETE SET NULL,
    archived_snapshot_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE analysis_tasks
ADD CONSTRAINT analysis_tasks_snapshot_link_fkey
FOREIGN KEY (id, snapshot_id)
REFERENCES analysis_snapshots (task_id, id)
DEFERRABLE INITIALLY DEFERRED;
```

- [ ] **Step 5: Create the new crate with core enums and `PromptSpec`**

```rust
// crates/pa-orchestrator/src/models.rs
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisTaskStatus {
    Pending,
    Running,
    RetryWaiting,
    Succeeded,
    Failed,
    DeadLetter,
    Cancelled,
}

impl AnalysisTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::RetryWaiting => "retry_waiting",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::DeadLetter => "dead_letter",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisBarState {
    None,
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicyClass {
    NetworkTransient,
    LlmRateLimited,
    LlmStructuredOutput,
    DomainValidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptResultSemantics {
    SharedAsset,
    UserPrivateAsset,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptSpec {
    pub prompt_key: String,
    pub prompt_version: String,
    pub task_type: String,
    pub system_prompt: String,
    pub input_schema_version: String,
    pub output_schema_version: String,
    pub output_json_schema: Value,
    pub retry_policy_class: RetryPolicyClass,
    pub result_semantics: PromptResultSemantics,
    pub bar_state_support: Vec<AnalysisBarState>,
}
```

- [ ] **Step 6: Add retry-oriented error helpers to `pa-core`**

```rust
// crates/pa-core/src/error.rs
impl AppError {
    pub fn is_retryable(&self) -> bool {
        match self {
            AppError::Provider { message, source } | AppError::Storage { message, source } => {
                let normalized = message.to_ascii_lowercase();
                normalized.contains("timeout")
                    || normalized.contains("timed out")
                    || normalized.contains("rate limit")
                    || normalized.contains("temporar")
                    || source.as_deref().is_some()
            }
            _ => false,
        }
    }
}
```

- [ ] **Step 7: Run the new model test**

Run: `cargo test -p pa-orchestrator prompt_spec_exposes_phase2_contract_fields -- --exact`

Expected: PASS

Run: `cargo test -p pa-orchestrator --test models`

Expected: PASS with the prompt-contract test plus enum string-mapping coverage

- [ ] **Step 8: Commit the schema and crate bootstrap**

```bash
git add Cargo.toml migrations/002_phase2_analysis_orchestration.sql crates/pa-core/src/error.rs crates/pa-orchestrator
git commit -m "feat: add phase2 orchestration schema and core enums"
```

### Task 2: Implement Task Identity, Snapshots, and In-Memory Repository Behavior

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\models.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\repository.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\dedupe.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\dedupe.rs`

- [ ] **Step 1: Write the failing dedupe test for `closed` vs `open` shared tasks**

```rust
use chrono::{TimeZone, Utc};
use pa_core::Timeframe;
use pa_orchestrator::{build_shared_bar_dedupe_key, AnalysisBarState};
use uuid::Uuid;

#[test]
fn closed_bar_dedupe_key_exists_and_open_bar_dedupe_key_does_not() {
    let instrument_id = Uuid::new_v4();
    let closed_key = build_shared_bar_dedupe_key(
        instrument_id,
        Timeframe::M15,
        Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap(),
        "shared_bar_analysis",
        "v1",
        AnalysisBarState::Closed,
    );

    let open_key = build_shared_bar_dedupe_key(
        instrument_id,
        Timeframe::M15,
        Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap(),
        "shared_bar_analysis",
        "v1",
        AnalysisBarState::Open,
    );

    assert!(closed_key.is_some());
    assert_eq!(open_key, None);
}
```

- [ ] **Step 2: Run the test to verify it fails because the dedupe helper does not exist yet**

Run: `cargo test -p pa-orchestrator closed_bar_dedupe_key_exists_and_open_bar_dedupe_key_does_not -- --exact`

Expected: FAIL with an unresolved import for `build_shared_bar_dedupe_key`

- [ ] **Step 3: Add snapshot, task, attempt, result, and dead-letter models**

```rust
// crates/pa-orchestrator/src/models.rs
use chrono::{DateTime, NaiveDate, Utc};
use pa_core::Timeframe;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisTask {
    pub id: Uuid,
    pub task_type: String,
    pub status: AnalysisTaskStatus,
    pub instrument_id: Uuid,
    pub user_id: Option<Uuid>,
    pub timeframe: Option<Timeframe>,
    pub bar_state: AnalysisBarState,
    pub bar_open_time: Option<DateTime<Utc>>,
    pub bar_close_time: Option<DateTime<Utc>>,
    pub trading_date: Option<NaiveDate>,
    pub trigger_type: String,
    pub prompt_key: String,
    pub prompt_version: String,
    pub snapshot_id: Uuid,
    pub dedupe_key: Option<String>,
    pub attempt_count: u32,
    pub max_attempts: u32,
    pub scheduled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisSnapshot {
    pub id: Uuid,
    pub task_id: Uuid,
    pub input_json: Value,
    pub input_hash: String,
    pub schema_version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisAttempt {
    pub id: Uuid,
    pub task_id: Uuid,
    pub attempt_no: u32,
    pub worker_id: String,
    pub llm_provider: String,
    pub model: String,
    pub request_payload_json: Value,
    pub raw_response_json: Option<Value>,
    pub parsed_output_json: Option<Value>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResult {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_type: String,
    pub instrument_id: Uuid,
    pub user_id: Option<Uuid>,
    pub timeframe: Option<Timeframe>,
    pub bar_state: AnalysisBarState,
    pub bar_open_time: Option<DateTime<Utc>>,
    pub bar_close_time: Option<DateTime<Utc>>,
    pub trading_date: Option<NaiveDate>,
    pub prompt_key: String,
    pub prompt_version: String,
    pub output_json: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisDeadLetter {
    pub id: Uuid,
    pub task_id: Uuid,
    pub final_error_type: String,
    pub final_error_message: String,
    pub archived_snapshot_json: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskEnvelope {
    pub task: AnalysisTask,
    pub snapshot: AnalysisSnapshot,
}
```

- [ ] **Step 4: Add repository trait and in-memory implementation**

```rust
// crates/pa-orchestrator/src/repository.rs
#[async_trait::async_trait]
pub trait OrchestrationRepository: Send + Sync {
    async fn insert_task_with_snapshot(
        &self,
        task: AnalysisTask,
        snapshot: AnalysisSnapshot,
    ) -> Result<InsertTaskResult, AppError>;

    async fn fetch_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError>;

    async fn load_snapshot(&self, snapshot_id: Uuid) -> Result<AnalysisSnapshot, AppError>;
}
```

```rust
// crates/pa-orchestrator/src/repository.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertTaskResult {
    Inserted,
    DuplicateExistingTask(Uuid),
}
```

- [ ] **Step 5: Implement dedupe builders**

```rust
// crates/pa-orchestrator/src/dedupe.rs
pub fn build_shared_bar_dedupe_key(
    instrument_id: Uuid,
    timeframe: Timeframe,
    bar_close_time: DateTime<Utc>,
    prompt_key: &str,
    prompt_version: &str,
    bar_state: AnalysisBarState,
) -> Option<String> {
    matches!(bar_state, AnalysisBarState::Closed).then(|| {
        format!(
            "shared_bar_analysis:{instrument_id}:{timeframe}:{bar_close_time}:{prompt_key}:{prompt_version}:closed",
            timeframe = timeframe.as_str(),
            bar_close_time = bar_close_time.to_rfc3339(),
        )
    })
}

pub fn sha256_json(value: &serde_json::Value) -> Result<String, AppError> {
    use sha2::{Digest, Sha256};

    let bytes = serde_json::to_vec(value).map_err(|err| AppError::Analysis {
        message: format!("failed to serialize json for hashing: {err}"),
        source: None,
    })?;

    Ok(format!("{:x}", Sha256::digest(bytes)))
}
```

- [ ] **Step 6: Run the dedupe and repository tests**

Run: `cargo test -p pa-orchestrator --test dedupe`

Expected: PASS with:
- one `closed/open` dedupe-key test
- one in-memory duplicate-task suppression test for `closed bar`
- one repeated insertion test for `open bar`

- [ ] **Step 7: Commit the repository and task-identity layer**

```bash
git add crates/pa-orchestrator/src/models.rs crates/pa-orchestrator/src/repository.rs crates/pa-orchestrator/src/dedupe.rs crates/pa-orchestrator/tests/dedupe.rs
git commit -m "feat: add orchestration repository and dedupe behavior"
```

### Task 3: Add Prompt Registry, LLM Client Abstraction, and Schema Validation

**Files:**
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\prompt_registry.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\llm.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\executor.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\executor.rs`

- [ ] **Step 1: Write the failing schema-validation test**

```rust
use pa_orchestrator::{Executor, FixtureLlmClient, PromptRegistry, PromptSpec, RetryPolicyClass};

#[tokio::test]
async fn executor_rejects_nonconforming_json() {
    let registry = PromptRegistry::default().with_spec(PromptSpec {
        prompt_key: "shared_bar_analysis".into(),
        prompt_version: "v1".into(),
        task_type: "shared_bar_analysis".into(),
        system_prompt: "Return JSON only".into(),
        input_schema_version: "v1".into(),
        output_schema_version: "v1".into(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": ["bullish_case", "bearish_case"]
        }),
        retry_policy_class: RetryPolicyClass::LlmStructuredOutput,
        result_semantics: pa_orchestrator::PromptResultSemantics::SharedAsset,
        bar_state_support: vec![pa_orchestrator::AnalysisBarState::Closed],
    });

    let client = FixtureLlmClient::with_json(serde_json::json!({"bullish_case": {}}));
    let executor = Executor::new(registry, client);

    let err = executor
        .execute_json("shared_bar_analysis", "v1", &serde_json::json!({"foo": "bar"}))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("schema"));
}
```

- [ ] **Step 2: Run the test to verify it fails because the executor does not exist**

Run: `cargo test -p pa-orchestrator executor_rejects_nonconforming_json -- --exact`

Expected: FAIL with unresolved imports for `Executor` and `PromptRegistry`

- [ ] **Step 3: Add the prompt registry**

```rust
// crates/pa-orchestrator/src/prompt_registry.rs
#[derive(Default)]
pub struct PromptRegistry {
    specs: std::collections::HashMap<(String, String), PromptSpec>,
}

impl PromptRegistry {
    pub fn with_spec(mut self, spec: PromptSpec) -> Self {
        self.specs.insert(
            (spec.prompt_key.clone(), spec.prompt_version.clone()),
            spec,
        );
        self
    }

    pub fn get(&self, prompt_key: &str, prompt_version: &str) -> Option<&PromptSpec> {
        self.specs
            .get(&(prompt_key.to_owned(), prompt_version.to_owned()))
    }
}
```

- [ ] **Step 4: Add the LLM client abstraction**

```rust
// crates/pa-orchestrator/src/llm.rs
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_json(
        &self,
        system_prompt: &str,
        input_json: &serde_json::Value,
    ) -> Result<serde_json::Value, AppError>;
}
```

- [ ] **Step 5: Implement the executor with JSON schema validation**

```rust
// crates/pa-orchestrator/src/executor.rs
pub struct Executor<C> {
    registry: PromptRegistry,
    client: C,
}

impl<C> Executor<C>
where
    C: LlmClient,
{
    pub async fn execute_json(
        &self,
        prompt_key: &str,
        prompt_version: &str,
        input_json: &serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let spec = self
            .registry
            .get(prompt_key, prompt_version)
            .ok_or_else(|| AppError::Analysis {
                message: format!("missing prompt spec: {prompt_key}:{prompt_version}"),
                source: None,
            })?;

        let output = self
            .client
            .generate_json(&spec.system_prompt, input_json)
            .await?;

        let validator = jsonschema::validator_for(&spec.output_json_schema)
            .map_err(|err| AppError::Analysis {
                message: format!("invalid output schema: {err}"),
                source: None,
            })?;

        if validator.is_valid(&output) {
            Ok(output)
        } else {
            Err(AppError::Analysis {
                message: "schema validation failed".into(),
                source: None,
            })
        }
    }
}
```

- [ ] **Step 6: Run the executor tests**

Run: `cargo test -p pa-orchestrator --test executor`

Expected: PASS with:
- one missing-prompt-spec failure test
- one invalid-output-schema failure test
- one valid structured-output success test

- [ ] **Step 7: Commit prompt registry and executor**

```bash
git add crates/pa-orchestrator/src/prompt_registry.rs crates/pa-orchestrator/src/llm.rs crates/pa-orchestrator/src/executor.rs crates/pa-orchestrator/tests/executor.rs Cargo.toml crates/pa-orchestrator/Cargo.toml
git commit -m "feat: add prompt registry and schema validated executor"
```

### Task 4: Add Shared Analysis Prompt Contracts and Task Factories

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\lib.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\models.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\src\prompt_specs.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\src\task_factory.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-analysis\tests\task_factory.rs`

- [ ] **Step 1: Write the failing shared-bar task-factory test**

```rust
use chrono::{TimeZone, Utc};
use pa_analysis::{build_shared_bar_analysis_task, SharedBarAnalysisInput};
use pa_orchestrator::AnalysisBarState;
use pa_core::Timeframe;
use uuid::Uuid;

#[test]
fn closed_shared_bar_task_has_dedupe_key_and_open_bar_task_does_not() {
    let input = SharedBarAnalysisInput {
        instrument_id: Uuid::new_v4(),
        timeframe: Timeframe::M15,
        bar_open_time: Utc.with_ymd_and_hms(2026, 4, 21, 1, 45, 0).unwrap(),
        bar_close_time: Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap(),
        bar_state: AnalysisBarState::Closed,
        canonical_bar_json: serde_json::json!({"close": 101}),
        structure_context_json: serde_json::json!({"recent_bars": []}),
    };

    let closed = build_shared_bar_analysis_task(input.clone()).unwrap();
    let open = build_shared_bar_analysis_task(SharedBarAnalysisInput {
        bar_state: AnalysisBarState::Open,
        ..input
    })
    .unwrap();

    assert!(closed.task.dedupe_key.is_some());
    assert_eq!(open.task.dedupe_key, None);
}
```

- [ ] **Step 2: Run the test to verify it fails because the task factory does not exist**

Run: `cargo test -p pa-analysis closed_shared_bar_task_has_dedupe_key_and_open_bar_task_does_not -- --exact`

Expected: FAIL with unresolved import errors for `build_shared_bar_analysis_task`

- [ ] **Step 3: Define typed shared-analysis input/output models**

```rust
// crates/pa-analysis/src/models.rs
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SharedBarAnalysisInput {
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub bar_open_time: DateTime<Utc>,
    pub bar_close_time: DateTime<Utc>,
    pub bar_state: pa_orchestrator::AnalysisBarState,
    pub canonical_bar_json: serde_json::Value,
    pub structure_context_json: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SharedBarAnalysisOutput {
    pub bar_state: String,
    pub bar_classification: serde_json::Value,
    pub bullish_case: serde_json::Value,
    pub bearish_case: serde_json::Value,
    pub two_sided_summary: serde_json::Value,
    pub nearby_levels: serde_json::Value,
    pub signal_strength: serde_json::Value,
    pub continuation_scenarios: serde_json::Value,
    pub reversal_scenarios: serde_json::Value,
    pub invalidation_levels: serde_json::Value,
    pub execution_bias_notes: serde_json::Value,
}
```

- [ ] **Step 4: Add `shared_bar_analysis_v1` and `shared_daily_context_v1` prompt specs**

```rust
// crates/pa-analysis/src/prompt_specs.rs
pub fn shared_bar_analysis_v1() -> pa_orchestrator::PromptSpec {
    pa_orchestrator::PromptSpec {
        prompt_key: "shared_bar_analysis".into(),
        prompt_version: "v1".into(),
        task_type: "shared_bar_analysis".into(),
        system_prompt: "You are a price-action analyst. Return JSON only.".into(),
        input_schema_version: "v1".into(),
        output_schema_version: "v1".into(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "bar_state",
                "bar_classification",
                "bullish_case",
                "bearish_case",
                "two_sided_summary",
                "nearby_levels",
                "signal_strength",
                "continuation_scenarios",
                "reversal_scenarios",
                "invalidation_levels",
                "execution_bias_notes"
            ]
        }),
        retry_policy_class: pa_orchestrator::RetryPolicyClass::LlmStructuredOutput,
        result_semantics: pa_orchestrator::PromptResultSemantics::SharedAsset,
        bar_state_support: vec![
            pa_orchestrator::AnalysisBarState::Open,
            pa_orchestrator::AnalysisBarState::Closed,
        ],
    }
}
```

- [ ] **Step 5: Implement shared task factories**

```rust
// crates/pa-analysis/src/task_factory.rs
pub fn build_shared_bar_analysis_task(
    input: SharedBarAnalysisInput,
) -> Result<pa_orchestrator::TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let input_json = serde_json::to_value(&input).map_err(|err| AppError::Analysis {
        message: format!("failed to serialize shared bar input: {err}"),
        source: None,
    })?;

    let dedupe_key = pa_orchestrator::build_shared_bar_dedupe_key(
        input.instrument_id,
        input.timeframe,
        input.bar_close_time,
        "shared_bar_analysis",
        "v1",
        input.bar_state,
    );

    Ok(pa_orchestrator::TaskEnvelope::new_shared(
        task_id,
        snapshot_id,
        "shared_bar_analysis",
        "v1",
        input.instrument_id,
        Some(input.timeframe),
        input.bar_state,
        Some(input.bar_open_time),
        Some(input.bar_close_time),
        dedupe_key,
        input_json,
    ))
}
```

- [ ] **Step 6: Run the shared task-factory tests**

Run: `cargo test -p pa-analysis --test task_factory`

Expected: PASS with:
- one `closed/open` dedupe behavior test
- one shared daily-context snapshot-shape test
- one prompt-spec field assertion test for required PA contract fields

- [ ] **Step 7: Commit the shared-analysis orchestration surface**

```bash
git add crates/pa-analysis/src/lib.rs crates/pa-analysis/src/models.rs crates/pa-analysis/src/prompt_specs.rs crates/pa-analysis/src/task_factory.rs crates/pa-analysis/tests/task_factory.rs crates/pa-analysis/Cargo.toml
git commit -m "feat: add shared analysis prompt specs and task factories"
```

### Task 5: Add User Analysis Prompt Contracts and Task Factories

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-user\src\lib.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-user\src\models.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-user\src\prompt_specs.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-user\src\task_factory.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-user\tests\task_factory.rs`

- [ ] **Step 1: Write the failing manual and scheduled user task-factory tests**

```rust
use chrono::{NaiveDate, TimeZone, Utc};
use pa_core::Timeframe;
use pa_orchestrator::AnalysisBarState;
use pa_user::{
    build_manual_user_analysis_task, build_scheduled_user_analysis_task,
    ManualUserAnalysisInput, ScheduledUserAnalysisInput,
};
use rust_decimal::Decimal;
use uuid::Uuid;

#[test]
fn closed_manual_user_task_has_position_hash_dedupe_and_open_task_does_not() {
    let user_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();
    let input = ManualUserAnalysisInput {
        user_id,
        instrument_id,
        timeframe: Timeframe::H1,
        bar_state: AnalysisBarState::Closed,
        bar_open_time: Some(Utc.with_ymd_and_hms(2026, 4, 21, 1, 0, 0).unwrap()),
        bar_close_time: Some(Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap()),
        trading_date: Some(NaiveDate::from_ymd_opt(2026, 4, 21).unwrap()),
        positions_json: serde_json::json!([{
            "side": "long",
            "quantity": Decimal::new(1, 0),
            "average_cost": Decimal::new(100, 0)
        }]),
        subscriptions_json: serde_json::json!([]),
        shared_bar_analysis_json: serde_json::json!({"bullish_case": {}, "bearish_case": {}}),
        shared_daily_context_json: serde_json::json!({"decision_tree_nodes": {}}),
    };

    let closed = build_manual_user_analysis_task(input.clone()).unwrap();
    let open = build_manual_user_analysis_task(ManualUserAnalysisInput {
        bar_state: AnalysisBarState::Open,
        ..input
    })
    .unwrap();

    assert!(closed.task.dedupe_key.is_some());
    assert_eq!(open.task.dedupe_key, None);
}

#[test]
fn scheduled_user_task_is_deduplicated_by_schedule_identity() {
    let input = ScheduledUserAnalysisInput {
        schedule_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        instrument_id: Uuid::new_v4(),
        timeframe: Timeframe::H1,
        trading_date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
        positions_json: serde_json::json!([]),
        subscriptions_json: serde_json::json!([]),
        shared_bar_analysis_json: serde_json::json!({"bullish_case": {}, "bearish_case": {}}),
        shared_daily_context_json: serde_json::json!({"decision_tree_nodes": {}}),
    };

    let task = build_scheduled_user_analysis_task(input).unwrap();
    assert!(task.task.dedupe_key.is_some());
}
```

- [ ] **Step 2: Run the test to verify it fails because the task factory does not exist yet**

Run: `cargo test -p pa-user closed_manual_user_task_has_position_hash_dedupe_and_open_task_does_not -- --exact`

Expected: FAIL with unresolved import errors for `ManualUserAnalysisInput` and `ScheduledUserAnalysisInput`

- [ ] **Step 3: Add typed user-analysis prompt IO models**

```rust
// crates/pa-user/src/models.rs
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ManualUserAnalysisInput {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub bar_state: pa_orchestrator::AnalysisBarState,
    pub bar_open_time: Option<DateTime<Utc>>,
    pub bar_close_time: Option<DateTime<Utc>>,
    pub trading_date: Option<NaiveDate>,
    pub positions_json: serde_json::Value,
    pub subscriptions_json: serde_json::Value,
    pub shared_bar_analysis_json: serde_json::Value,
    pub shared_daily_context_json: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScheduledUserAnalysisInput {
    pub schedule_id: Uuid,
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub trading_date: NaiveDate,
    pub positions_json: serde_json::Value,
    pub subscriptions_json: serde_json::Value,
    pub shared_bar_analysis_json: serde_json::Value,
    pub shared_daily_context_json: serde_json::Value,
}
```

- [ ] **Step 4: Add `user_position_advice_v1` prompt spec**

```rust
// crates/pa-user/src/prompt_specs.rs
pub fn user_position_advice_v1() -> pa_orchestrator::PromptSpec {
    pa_orchestrator::PromptSpec {
        prompt_key: "user_position_advice".into(),
        prompt_version: "v1".into(),
        task_type: "user_position_advice".into(),
        system_prompt: "Map shared PA structure to the user's position. Return JSON only.".into(),
        input_schema_version: "v1".into(),
        output_schema_version: "v1".into(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "position_state",
                "market_read_through",
                "bullish_path_for_user",
                "bearish_path_for_user",
                "hold_reduce_exit_conditions",
                "risk_control_levels",
                "invalidations",
                "action_candidates"
            ]
        }),
        retry_policy_class: pa_orchestrator::RetryPolicyClass::LlmStructuredOutput,
        result_semantics: pa_orchestrator::PromptResultSemantics::UserPrivateAsset,
        bar_state_support: vec![
            pa_orchestrator::AnalysisBarState::Open,
            pa_orchestrator::AnalysisBarState::Closed,
        ],
    }
}
```

- [ ] **Step 5: Implement user task factory with position snapshot hashing**

```rust
// crates/pa-user/src/task_factory.rs
pub fn build_manual_user_analysis_task(
    input: ManualUserAnalysisInput,
) -> Result<pa_orchestrator::TaskEnvelope, AppError> {
    let snapshot_json = serde_json::to_value(&input).map_err(|err| AppError::Analysis {
        message: format!("failed to serialize manual user analysis input: {err}"),
        source: None,
    })?;
    let position_snapshot_hash = pa_orchestrator::sha256_json(&input.positions_json)?;
    let dedupe_key = matches!(input.bar_state, pa_orchestrator::AnalysisBarState::Closed)
        .then(|| {
            format!(
                "user_position_advice:{}:{}:{}:{}:{}:{}:closed",
                input.user_id,
                input.instrument_id,
                input.timeframe.as_str(),
                input.bar_close_time.expect("closed bar_close_time").to_rfc3339(),
                "v1",
                position_snapshot_hash,
            )
        });

    Ok(pa_orchestrator::TaskEnvelope::new_user(
        input.user_id,
        input.instrument_id,
        input.timeframe,
        input.bar_state,
        input.bar_open_time,
        input.bar_close_time,
        input.trading_date,
        dedupe_key,
        snapshot_json,
    ))
}

pub fn build_scheduled_user_analysis_task(
    input: ScheduledUserAnalysisInput,
) -> Result<pa_orchestrator::TaskEnvelope, AppError> {
    let snapshot_json = serde_json::to_value(&input).map_err(|err| AppError::Analysis {
        message: format!("failed to serialize scheduled user analysis input: {err}"),
        source: None,
    })?;
    let position_snapshot_hash = pa_orchestrator::sha256_json(&input.positions_json)?;
    let dedupe_key = Some(format!(
        "user_scheduled_analysis:{}:{}:{}:{}:{}:{}",
        input.schedule_id,
        input.user_id,
        input.instrument_id,
        input.timeframe.as_str(),
        input.trading_date,
        position_snapshot_hash,
    ));

    Ok(pa_orchestrator::TaskEnvelope::new_scheduled_user(
        input.schedule_id,
        input.user_id,
        input.instrument_id,
        input.timeframe,
        input.trading_date,
        dedupe_key,
        snapshot_json,
    ))
}
```

- [ ] **Step 6: Run the user task-factory tests**

Run: `cargo test -p pa-user --test task_factory`

Expected: PASS with:
- one `closed/open` dedupe behavior test
- one full snapshot serialization test containing positions + shared outputs
- one scheduled-analysis dedupe-key test
- one prompt-spec PA field test

- [ ] **Step 7: Commit the user-analysis orchestration surface**

```bash
git add crates/pa-user/src/lib.rs crates/pa-user/src/models.rs crates/pa-user/src/prompt_specs.rs crates/pa-user/src/task_factory.rs crates/pa-user/tests/task_factory.rs crates/pa-user/Cargo.toml
git commit -m "feat: add user analysis prompt specs and task factories"
```

### Task 6: Implement Worker Execution, Retry Flow, and Dead-Letter Handling

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\models.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\retry.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\worker.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\repository.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\worker.rs`

- [ ] **Step 1: Write the failing worker retry/dead-letter test**

```rust
use pa_orchestrator::{
    run_single_task, AnalysisBarState, AnalysisTaskStatus, Executor, FixtureLlmClient,
    FixtureRepository, PromptRegistry, PromptResultSemantics, PromptSpec,
    RetryPolicyClass,
};

#[tokio::test]
async fn worker_moves_retry_exhausted_task_to_dead_letter() {
    let repository = FixtureRepository::with_retryable_task(2);
    let client = FixtureLlmClient::always_rate_limited();
    let executor = Executor::new(
        PromptRegistry::default().with_spec(PromptSpec {
            prompt_key: "shared_bar_analysis".into(),
            prompt_version: "v1".into(),
            task_type: "shared_bar_analysis".into(),
            system_prompt: "Return JSON only".into(),
            input_schema_version: "v1".into(),
            output_schema_version: "v1".into(),
            output_json_schema: serde_json::json!({"type": "object"}),
            retry_policy_class: RetryPolicyClass::LlmRateLimited,
            result_semantics: PromptResultSemantics::SharedAsset,
            bar_state_support: vec![AnalysisBarState::Closed],
        }),
        client,
    );

    run_single_task(&repository, &executor).await.unwrap();
    run_single_task(&repository, &executor).await.unwrap();

    let task = repository.only_task();
    assert_eq!(task.status, AnalysisTaskStatus::DeadLetter);
    assert_eq!(repository.dead_letters().len(), 1);
}
```

- [ ] **Step 2: Run the test to verify it fails because worker execution is not implemented**

Run: `cargo test -p pa-orchestrator worker_moves_retry_exhausted_task_to_dead_letter -- --exact`

Expected: FAIL with unresolved imports for `run_single_task` or missing dead-letter support

- [ ] **Step 3: Add retry classification helpers**

```rust
// crates/pa-orchestrator/src/retry.rs
pub enum RetryDecision {
    RetryNow,
    FailTerminal,
    MoveToDeadLetter,
}

pub fn classify_retry(
    err: &AppError,
    attempt_count: u32,
    max_attempts: u32,
) -> RetryDecision {
    if err.is_retryable() && attempt_count < max_attempts {
        RetryDecision::RetryNow
    } else if err.is_retryable() {
        RetryDecision::MoveToDeadLetter
    } else {
        RetryDecision::FailTerminal
    }
}
```

- [ ] **Step 4: Extend the repository contract for worker state transitions**

```rust
// crates/pa-orchestrator/src/repository.rs
#[async_trait::async_trait]
pub trait OrchestrationRepository: Send + Sync {
    async fn mark_task_running(&self, task_id: Uuid) -> Result<(), AppError>;
    async fn append_attempt(&self, attempt: AnalysisAttempt) -> Result<(), AppError>;
    async fn mark_task_retry_waiting(&self, task_id: Uuid, message: &str) -> Result<(), AppError>;
    async fn mark_task_failed(&self, task_id: Uuid, message: &str) -> Result<(), AppError>;
    async fn insert_result_and_complete(
        &self,
        result: AnalysisResult,
    ) -> Result<(), AppError>;
    async fn insert_dead_letter(
        &self,
        dead_letter: AnalysisDeadLetter,
    ) -> Result<(), AppError>;
}
```

- [ ] **Step 5: Implement the worker entrypoint**

```rust
// crates/pa-orchestrator/src/worker.rs
pub async fn run_single_task<R, C>(
    repository: &R,
    executor: &Executor<C>,
) -> Result<bool, AppError>
where
    R: OrchestrationRepository + ?Sized,
    C: LlmClient,
{
    let Some(task) = repository.fetch_next_pending_task().await? else {
        return Ok(false);
    };

    repository.mark_task_running(task.id).await?;
    let snapshot = repository.load_snapshot(task.snapshot_id).await?;
    let output = executor
        .execute_json(&task.prompt_key, &task.prompt_version, &snapshot.input_json)
        .await;

    match output {
        Ok(output_json) => {
            repository
                .insert_result_and_complete(AnalysisResult::from_task(task, output_json))
                .await?;
        }
        Err(err) => match classify_retry(&err, task.attempt_count + 1, task.max_attempts) {
            RetryDecision::RetryNow => repository
                .mark_task_retry_waiting(task.id, &err.to_string())
                .await?,
            RetryDecision::FailTerminal => repository
                .mark_task_failed(task.id, &err.to_string())
                .await?,
            RetryDecision::MoveToDeadLetter => repository
                .insert_dead_letter(AnalysisDeadLetter::from_task(task, &snapshot, &err))
                .await?,
        },
    }

    Ok(true)
}
```

- [ ] **Step 6: Run the worker tests**

Run: `cargo test -p pa-orchestrator --test worker`

Expected: PASS with:
- one success path writing result test
- one retryable error path returning task to retry flow
- one retry exhaustion dead-letter test
- one non-retryable validation failure test

- [ ] **Step 7: Commit the worker and retry layer**

```bash
git add crates/pa-orchestrator/src/retry.rs crates/pa-orchestrator/src/worker.rs crates/pa-orchestrator/src/repository.rs crates/pa-orchestrator/tests/worker.rs
git commit -m "feat: add orchestration worker retry and dead-letter flow"
```

### Task 7: Replace Placeholder APIs with Task Enqueue and Query Endpoints

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\analysis.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\user.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\router.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\lib.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-api\tests\smoke.rs`

- [ ] **Step 1: Write the failing enqueue smoke test**

```rust
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use pa_api::{app_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn create_shared_bar_task_returns_accepted_and_task_metadata() {
    let app = app_router(AppState::fixture());

    let response = app
        .oneshot(
            Request::post("/analysis/shared/bar")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"instrument_id":"00000000-0000-0000-0000-000000000001"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&body).unwrap().contains("\"task_id\""));
}
```

- [ ] **Step 2: Run the test to verify it fails because the route still returns placeholder content**

Run: `cargo test -p pa-api create_shared_bar_task_returns_accepted_and_task_metadata -- --exact`

Expected: FAIL with `501 Not Implemented`

- [ ] **Step 3: Add task-enqueue and task-query handlers**

```rust
// crates/pa-api/src/analysis.rs
use axum::{extract::{Path, State}, http::StatusCode, Json, Router, routing::{get, post}};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/shared/bar", post(create_shared_bar_task))
        .route("/shared/daily", post(create_shared_daily_task))
        .route("/tasks/{task_id}", get(get_task))
        .route("/results/{task_id}", get(get_result))
        .route("/tasks/{task_id}/attempts", get(get_attempts))
        .route("/dead-letters/{task_id}", get(get_dead_letter))
}

async fn create_shared_bar_task(
    State(state): State<AppState>,
    Json(request): Json<CreateSharedBarTaskRequest>,
) -> Result<(StatusCode, Json<CreateTaskResponse>), AppError> {
    let response = state.analysis_api.create_shared_bar_task(request).await?;
    Ok((StatusCode::ACCEPTED, Json(response)))
}
```

- [ ] **Step 4: Add manual user-analysis enqueue handler**

```rust
// crates/pa-api/src/user.rs
pub fn routes() -> Router<AppState> {
    Router::new().route("/analysis/manual", post(create_manual_user_analysis_task))
}
```

- [ ] **Step 5: Extend the smoke test**

Run: `cargo test -p pa-api --test smoke`

Expected: PASS with:
- `/healthz` -> `200`
- `/analysis/shared/bar` -> `202`
- `/analysis/tasks/{id}` -> `200`
- `/user/analysis/manual` -> `202`

- [ ] **Step 6: Commit the API surface**

```bash
git add crates/pa-api/src/analysis.rs crates/pa-api/src/user.rs crates/pa-api/src/router.rs crates/pa-api/src/lib.rs crates/pa-api/tests/smoke.rs
git commit -m "feat: add analysis orchestration api endpoints"
```

### Task 8: Wire Runtime Registration, Worker Startup, and Operator Notes

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-app\src\main.rs`
- Modify: `E:\rust-app\oh-paa\docs\architecture\phase1-runtime.md`

- [ ] **Step 1: Write the failing app compile check expectation**

Run: `cargo check -p pa-app`

Expected: FAIL because `pa-app` does not yet construct the orchestrator dependencies

- [ ] **Step 2: Register prompt specs and build the worker dependencies**

```rust
// crates/pa-app/src/main.rs
let prompt_registry = pa_orchestrator::PromptRegistry::default()
    .with_spec(pa_analysis::shared_bar_analysis_v1())
    .with_spec(pa_analysis::shared_daily_context_v1())
    .with_spec(pa_user::user_position_advice_v1());

let llm_client = pa_orchestrator::OpenAiJsonClient::new();
let orchestrator_repository = pa_orchestrator::PgOrchestrationRepository::new(pool.clone());
let worker = pa_orchestrator::Worker::new(orchestrator_repository, prompt_registry, llm_client);

tokio::spawn(async move {
    worker.run_forever().await
});
```

- [ ] **Step 3: Update runtime notes**

```markdown
## Phase 2 Worker Notes

- all shared and user analysis requests enqueue durable tasks first
- workers execute only from persisted snapshots
- `closed bar` tasks are deduplicated while `open bar` tasks may repeat
- structured JSON output is schema-validated before result persistence
- retry exhaustion moves tasks to dead-letter storage for operator inspection
```

- [ ] **Step 4: Run focused verification**

Run: `cargo check --workspace`

Expected: PASS

Run: `cargo test -p pa-orchestrator`

Expected: PASS

Run: `cargo test -p pa-analysis --test task_factory`

Expected: PASS

Run: `cargo test -p pa-user --test task_factory`

Expected: PASS

Run: `cargo test -p pa-api --test smoke`

Expected: PASS

- [ ] **Step 5: Run full verification**

Run: `cargo fmt --all`

Expected: formatting succeeds

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: PASS

Run: `cargo test --workspace`

Expected: PASS

- [ ] **Step 6: Commit the runtime wiring**

```bash
git add crates/pa-app/src/main.rs docs/architecture/phase1-runtime.md Cargo.lock
git commit -m "feat: wire phase2 llm orchestration runtime"
```

## Verification Checklist

Run these commands before claiming the plan has been executed successfully:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected:

- formatting completes successfully
- clippy reports no warnings
- all workspace tests pass

## Spec Coverage Self-Review

- asynchronous task execution for shared/manual/scheduled analysis: covered by Tasks 1, 2, 6, and 7
- full input snapshot persistence: covered by Tasks 1, 2, 4, and 5
- code-defined `PromptSpec` contracts: covered by Tasks 1, 3, 4, and 5
- strict JSON-only output validation: covered by Tasks 3 and 6
- `closed bar` strict dedupe and `open bar` repeatability: covered by Tasks 2, 4, and 5
- PA-oriented `shared_daily_context` decision tree fields: covered by Task 4 prompt-spec schema
- bullish and bearish dual-sided shared-bar analysis: covered by Task 4 prompt-spec schema
- user analysis built on shared outputs plus positions: covered by Task 5 snapshot and schema definitions
- retry, dead-letter, and execution attempts: covered by Task 6
- enqueue/query API semantics: covered by Task 7
- runtime registration and worker startup: covered by Task 8

Gaps intentionally deferred:

- scheduled-analysis API CRUD beyond the task-factory and task-type support in this phase
- production OpenAI transport details beyond the abstraction boundary
- dashboard UX for dead-letter inspection

## Placeholder Scan

Checked for and removed:

- common placeholder markers
- vague "handle this later" language
- references to undefined tasks without file paths or commands

## Type Consistency Review

- `AnalysisTaskStatus`, `AnalysisBarState`, `PromptSpec`, and `RetryPolicyClass` are introduced in Task 1 and reused consistently later
- `build_shared_bar_dedupe_key` from Task 2 is the helper consumed by Task 4
- `shared_bar_analysis_v1`, `shared_daily_context_v1`, and `user_position_advice_v1` are introduced before runtime registration in Task 8
- the worker in Task 6 consumes the repository and executor contracts introduced in Tasks 2 and 3
- API routes in Task 7 match the spec and the runtime expectations in Task 8
