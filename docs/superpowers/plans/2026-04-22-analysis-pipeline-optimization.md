# Analysis Pipeline Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a layered shared-analysis pipeline around persistent `shared_pa_state`, step-level prompt/model bindings, OpenAI-compatible LLM execution, and replay-based evaluation for historical market data.

**Architecture:** Extend `pa-orchestrator` from a prompt-registry executor into a step-oriented execution engine with explicit step specs, prompt templates, execution profiles, and OpenAI-compatible providers. Keep PA semantics in `pa-analysis` and `pa-user`, add a new first-class shared asset for `shared_pa_state`, then wire `pa-api` and `pa-app` so shared bar and shared daily analysis consume the new asset and can be evaluated via replay runs.

**Tech Stack:** Rust 2024, Tokio, Axum, SQLx + PostgreSQL, Serde, Serde JSON, Chrono, UUID, Reqwest, `jsonschema`, Tracing, TOML config.

---

## File Structure Map

- `E:\rust-app\oh-paa\crates\pa-core\src\config.rs`
  - Extend app config with OpenAI-compatible LLM provider definitions, execution profiles, and step bindings.
- `E:\rust-app\oh-paa\config.example.toml`
  - Document provider config, execution profiles, and step bindings for DeepSeek and DashScope.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\models.rs`
  - Add `AnalysisStepSpec`, `PromptTemplateSpec`, `ModelExecutionProfile`, `StepExecutionBinding`, and request metadata fields for attempts.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\prompt_registry.rs`
  - Evolve into a step registry keyed by step identity instead of prompt-only identity.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\llm.rs`
  - Split fixture transport from real OpenAI-compatible client behavior.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\openai_client.rs`
  - Implement OpenAI-compatible transport with DeepSeek and DashScope adapters.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\src\executor.rs`
  - Resolve step bindings, build transport requests, validate schema, and persist request metadata.
- `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\executor.rs`
  - Cover step-profile resolution and structured-output mode fallback.
- `E:\rust-app\oh-paa\crates\pa-analysis\src\models.rs`
  - Add `SharedPaStateBarInput` and `SharedPaStateBarOutput`; update shared bar/daily I/O models to reference PA state.
- `E:\rust-app\oh-paa\crates\pa-analysis\src\prompt_specs.rs`
  - Define step specs and prompt templates for `shared_pa_state_bar_v1`, `shared_bar_analysis_v2`, and `shared_daily_context_v2`.
- `E:\rust-app\oh-paa\crates\pa-analysis\src\task_factory.rs`
  - Add a PA-state task factory and revise shared bar/daily task factories to point at new step keys and snapshot contracts.
- `E:\rust-app\oh-paa\crates\pa-analysis\src\repository.rs`
  - Persist and query `shared_pa_state` alongside existing shared bar/daily shared assets.
- `E:\rust-app\oh-paa\crates\pa-analysis\tests\task_factory.rs`
  - Verify dedupe behavior and new snapshot shapes.
- `E:\rust-app\oh-paa\crates\pa-analysis\tests\pa_state_task.rs`
  - Verify one-result-per-identity semantics for closed bars and repeatability for open bars.
- `E:\rust-app\oh-paa\crates\pa-api\src\analysis_runtime.rs`
  - Assemble PA-state input from market runtime and revise downstream shared bar/daily input assembly to consume PA-state assets.
- `E:\rust-app\oh-paa\crates\pa-api\src\analysis.rs`
  - Add PA-state creation/query endpoints and revise shared bar/shared daily enqueue handlers.
- `E:\rust-app\oh-paa\crates\pa-api\tests\smoke.rs`
  - Cover the new public asset flow end-to-end.
- `E:\rust-app\oh-paa\crates\pa-user\src\models.rs`
  - Revise user-analysis input/output to carry optional PA-state evidence references.
- `E:\rust-app\oh-paa\crates\pa-user\src\prompt_specs.rs`
  - Move to step-oriented prompt definitions and updated user advice schema.
- `E:\rust-app\oh-paa\crates\pa-user\src\task_factory.rs`
  - Bind user analysis tasks to new step keys and expanded snapshot shape.
- `E:\rust-app\oh-paa\crates\pa-app\src\main.rs`
  - Initialize provider configs, step bindings, prompt registry, worker executor, and replay command entry points.
- `E:\rust-app\oh-paa\crates\pa-app\src\lib.rs`
  - Export replay helpers and app bootstrap helpers so tests can call them directly.
- `E:\rust-app\oh-paa\crates\pa-app\src\bin\replay_analysis.rs`
  - Add a CLI to replay historical samples through named pipeline variants.
- `E:\rust-app\oh-paa\crates\pa-app\src\replay.rs`
  - Shared replay runner, experiment logging models, and scorer plumbing for the CLI.
- `E:\rust-app\oh-paa\crates\pa-app\tests\replay.rs`
  - Verify replay variant execution and experiment log shape.
- `E:\rust-app\oh-paa\testdata\analysis_replay\sample_set.json`
  - Seed replay samples covering A-shares, crypto, and forex.
- `E:\rust-app\oh-paa\docs\architecture\phase1-runtime.md`
  - Update runtime notes with the new layered analysis flow.

## Decomposition Note

This is one plan for one subsystem: the shared-analysis optimization track. It touches configuration, orchestration, domain analysis, API wiring, and replay tooling, but every task serves the same end-to-end flow:

`market runtime -> shared_pa_state -> shared_bar/shared_daily -> user advice -> replay evaluation`

The tasks below keep that flow testable in increments and intentionally stop before reviewer chains or workflow DSLs.

### Task 1: Add Step-Oriented LLM Configuration

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-core\src\config.rs`
- Modify: `E:\rust-app\oh-paa\config.example.toml`
- Test: `E:\rust-app\oh-paa\crates\pa-core\tests\config.rs`

- [ ] **Step 1: Write the failing config-loading test for providers, profiles, and bindings**

```rust
use pa_core::config::AppConfig;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn load_from_path_reads_llm_provider_profiles_and_bindings() {
    let path = std::env::temp_dir().join(format!(
        "llm-config-{}.toml",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::write(&path, r#"
database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "demo"

[llm.providers.deepseek]
base_url = "https://api.deepseek.com"
api_key = "deepseek-key"
openai_api_style = "chat_completions"

[llm.providers.dashscope]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "dashscope-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.shared_bar_reasoner]
provider = "deepseek"
model = "deepseek-reasoner"
max_tokens = 32768
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = true

[llm.execution_profiles.pa_state_extract_fast]
provider = "dashscope"
model = "qwen-plus"
max_tokens = 12000
max_retries = 2
per_call_timeout_secs = 180
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "pa_state_extract_fast"
"#).unwrap();

    let config = AppConfig::load_from_path(&path).unwrap();

    assert_eq!(config.llm.providers.len(), 2);
    assert_eq!(config.llm.providers["deepseek"].base_url, "https://api.deepseek.com");
    assert_eq!(config.llm.execution_profiles["shared_bar_reasoner"].model, "deepseek-reasoner");
    assert_eq!(
        config.llm.step_bindings["shared_pa_state_bar_v1"].execution_profile,
        "pa_state_extract_fast"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails because `llm` config is not defined**

Run: `cargo test -p pa-core load_from_path_reads_llm_provider_profiles_and_bindings -- --exact`

Expected: FAIL with an unknown field or missing field error referencing `llm`

- [ ] **Step 3: Add config structs for provider definitions, execution profiles, and step bindings**

```rust
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlmConfig {
    pub providers: std::collections::BTreeMap<String, LlmProviderConfig>,
    pub execution_profiles: std::collections::BTreeMap<String, LlmExecutionProfileConfig>,
    pub step_bindings: std::collections::BTreeMap<String, LlmStepBindingConfig>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlmProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub openai_api_style: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlmExecutionProfileConfig {
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    pub max_retries: u32,
    pub per_call_timeout_secs: u64,
    pub retry_initial_backoff_ms: u64,
    pub supports_json_schema: bool,
    pub supports_reasoning: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlmStepBindingConfig {
    pub execution_profile: String,
}
```

- [ ] **Step 4: Add `llm` to `AppConfig` and extend the example config with DeepSeek and DashScope defaults**

```toml
[llm.providers.deepseek]
base_url = "https://api.deepseek.com"
api_key = "replace-with-real-key"
openai_api_style = "chat_completions"

[llm.providers.dashscope]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "replace-with-real-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.pa_state_extract_fast]
provider = "dashscope"
model = "qwen-plus"
max_tokens = 12000
max_retries = 2
per_call_timeout_secs = 180
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "pa_state_extract_fast"
```

- [ ] **Step 5: Run the config tests to verify the new config shape parses and old unknown-key rejection still works**

Run: `cargo test -p pa-core --test config`

Expected: PASS with the existing config tests plus the new provider/profile/binding coverage

- [ ] **Step 6: Commit the configuration slice**

```bash
git add crates/pa-core/src/config.rs crates/pa-core/tests/config.rs config.example.toml
git commit -m "feat: add step-oriented llm configuration"
```

### Task 2: Refactor Orchestrator Contracts Around Step Specs and Execution Profiles

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\models.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\prompt_registry.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\lib.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\models.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\executor.rs`

- [ ] **Step 1: Write the failing step-registry test**

```rust
use pa_orchestrator::{
    AnalysisBarState, AnalysisStepSpec, ModelExecutionProfile, PromptResultSemantics,
    PromptTemplateSpec, StepExecutionBinding, StepRegistry,
};

#[test]
fn step_registry_resolves_step_template_profile_and_binding() {
    let registry = StepRegistry::default()
        .with_step(AnalysisStepSpec {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            task_type: "shared_pa_state_bar".into(),
            input_schema_version: "v1".into(),
            output_schema_version: "v1".into(),
            output_json_schema: serde_json::json!({"type":"object"}),
            result_semantics: PromptResultSemantics::SharedAsset,
            bar_state_support: vec![AnalysisBarState::Closed, AnalysisBarState::Open],
            dependency_policy: "market_runtime_only".into(),
        })
        .unwrap()
        .with_prompt_template(PromptTemplateSpec {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            system_prompt: "Return JSON".into(),
            developer_instructions: vec!["Do not invent data".into()],
        })
        .unwrap()
        .with_execution_profile(ModelExecutionProfile {
            profile_key: "pa_state_extract_fast".into(),
            provider: "dashscope".into(),
            model: "qwen-plus".into(),
            max_tokens: 12000,
            timeout_secs: 180,
            max_retries: 2,
            retry_initial_backoff_ms: 1000,
            supports_json_schema: false,
            supports_reasoning: false,
        })
        .unwrap()
        .with_binding(StepExecutionBinding {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            execution_profile: "pa_state_extract_fast".into(),
        })
        .unwrap();

    let resolved = registry.resolve("shared_pa_state_bar", "v1").unwrap();
    assert_eq!(resolved.step.task_type, "shared_pa_state_bar");
    assert_eq!(resolved.profile.model, "qwen-plus");
    assert_eq!(resolved.prompt.developer_instructions.len(), 1);
}
```

- [ ] **Step 2: Run the test to verify it fails because the new types and registry do not exist**

Run: `cargo test -p pa-orchestrator step_registry_resolves_step_template_profile_and_binding -- --exact`

Expected: FAIL with unresolved import errors for `AnalysisStepSpec`, `PromptTemplateSpec`, `ModelExecutionProfile`, `StepExecutionBinding`, or `StepRegistry`

- [ ] **Step 3: Add the new step, prompt-template, and execution-profile types**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisStepSpec {
    pub step_key: String,
    pub step_version: String,
    pub task_type: String,
    pub input_schema_version: String,
    pub output_schema_version: String,
    pub output_json_schema: Value,
    pub result_semantics: PromptResultSemantics,
    pub bar_state_support: Vec<AnalysisBarState>,
    pub dependency_policy: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptTemplateSpec {
    pub step_key: String,
    pub step_version: String,
    pub system_prompt: String,
    pub developer_instructions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelExecutionProfile {
    pub profile_key: String,
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_initial_backoff_ms: u64,
    pub supports_json_schema: bool,
    pub supports_reasoning: bool,
}
```

- [ ] **Step 4: Replace prompt-only lookup with a registry that resolves step spec, prompt template, profile, and binding together**

```rust
pub struct ResolvedStep<'a> {
    pub step: &'a AnalysisStepSpec,
    pub prompt: &'a PromptTemplateSpec,
    pub profile: &'a ModelExecutionProfile,
}

pub fn resolve(&self, step_key: &str, step_version: &str) -> Option<ResolvedStep<'_>> {
    let binding = self
        .bindings
        .get(&(step_key.to_owned(), step_version.to_owned()))?;
    let step = self.steps.get(&(step_key.to_owned(), step_version.to_owned()))?;
    let prompt = self.prompts.get(&(step_key.to_owned(), step_version.to_owned()))?;
    let profile = self.profiles.get(&binding.execution_profile)?;
    Some(ResolvedStep { step, prompt, profile })
}
```

- [ ] **Step 5: Update exports so all downstream crates can consume the new types**

```rust
pub use models::{
    AnalysisAttempt, AnalysisBarState, AnalysisDeadLetter, AnalysisResult, AnalysisSnapshot,
    AnalysisStepSpec, AnalysisTask, AnalysisTaskStatus, ModelExecutionProfile,
    PromptResultSemantics, PromptTemplateSpec, RetryPolicyClass, StepExecutionBinding,
    TaskEnvelope,
};
pub use prompt_registry::StepRegistry;
```

- [ ] **Step 6: Run the orchestrator model and registry tests**

Run: `cargo test -p pa-orchestrator --test models --test executor`

Expected: PASS with the new step-registry test and existing model validation tests

- [ ] **Step 7: Commit the orchestrator contract refactor**

```bash
git add crates/pa-orchestrator/src/models.rs crates/pa-orchestrator/src/prompt_registry.rs crates/pa-orchestrator/src/lib.rs crates/pa-orchestrator/tests/models.rs crates/pa-orchestrator/tests/executor.rs
git commit -m "refactor: add step-oriented orchestration contracts"
```

### Task 3: Implement the OpenAI-Compatible Client and Executor Binding Resolution

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\llm.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\openai_client.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\executor.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-orchestrator\src\worker.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\executor.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-orchestrator\tests\worker.rs`

- [ ] **Step 1: Write the failing executor test for profile-driven transport requests**

```rust
use pa_orchestrator::{
    AnalysisBarState, AnalysisStepSpec, ExecutionOutcome, Executor, FixtureLlmClient,
    ModelExecutionProfile, PromptResultSemantics, PromptTemplateSpec, StepExecutionBinding,
    StepRegistry,
};

#[tokio::test]
async fn executor_uses_bound_execution_profile_metadata() {
    let registry = StepRegistry::default()
        .with_step(AnalysisStepSpec {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            task_type: "shared_pa_state_bar".into(),
            input_schema_version: "v1".into(),
            output_schema_version: "v1".into(),
            output_json_schema: serde_json::json!({"type":"object"}),
            result_semantics: PromptResultSemantics::SharedAsset,
            bar_state_support: vec![AnalysisBarState::Closed],
            dependency_policy: "market_runtime_only".into(),
        })
        .unwrap()
        .with_prompt_template(PromptTemplateSpec {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            system_prompt: "Return JSON".into(),
            developer_instructions: vec![],
        })
        .unwrap()
        .with_execution_profile(ModelExecutionProfile {
            profile_key: "pa_state_extract_fast".into(),
            provider: "dashscope".into(),
            model: "qwen-plus".into(),
            max_tokens: 12000,
            timeout_secs: 180,
            max_retries: 2,
            retry_initial_backoff_ms: 1000,
            supports_json_schema: false,
            supports_reasoning: false,
        })
        .unwrap()
        .with_binding(StepExecutionBinding {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            execution_profile: "pa_state_extract_fast".into(),
        })
        .unwrap();
    let executor = Executor::new(registry, FixtureLlmClient::with_json(serde_json::json!({})));

    let outcome = executor
        .execute_json("shared_pa_state_bar", "v1", &serde_json::json!({"bar_identity":{}}))
        .await
        .unwrap();

    match outcome {
        ExecutionOutcome::Success(attempt) => {
            assert_eq!(attempt.llm_provider, "dashscope");
            assert_eq!(attempt.model, "qwen-plus");
            assert_eq!(attempt.request_payload_json["max_tokens"], 12000);
        }
        other => panic!("expected success, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run the test to verify it fails because executor still uses the old prompt-only request shape**

Run: `cargo test -p pa-orchestrator executor_uses_bound_execution_profile_metadata -- --exact`

Expected: FAIL because the executor does not resolve execution profiles and the request payload lacks `max_tokens`

- [ ] **Step 3: Add a transport request type that carries prompt text plus execution-profile metadata**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest {
    pub provider: String,
    pub model: String,
    pub system_prompt: String,
    pub developer_instructions: Vec<String>,
    pub input_json: Value,
    pub max_tokens: u32,
    pub timeout_secs: u64,
    pub structured_output_mode: StructuredOutputMode,
}
```

- [ ] **Step 4: Implement `OpenAiCompatibleClient` with JSON-schema, JSON-object, and prompt-enforced modes**

```rust
pub enum StructuredOutputMode {
    NativeJsonSchema,
    JsonObject,
    PromptEnforcedJson,
}

pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    providers: std::collections::BTreeMap<String, OpenAiProviderRuntime>,
}

impl OpenAiCompatibleClient {
    async fn post_chat_completions(
        &self,
        request: &LlmRequest,
    ) -> Result<serde_json::Value, AppError> {
        let provider = self.providers.get(&request.provider).ok_or_else(|| AppError::Validation {
            message: format!("missing llm provider runtime: {}", request.provider),
            source: None,
        })?;

        let payload = serde_json::json!({
            "model": request.model,
            "messages": build_messages(request),
            "max_tokens": request.max_tokens,
            "response_format": response_format_for(request.structured_output_mode),
        });

        let response = self
            .http
            .post(format!("{}/chat/completions", provider.base_url.trim_end_matches('/')))
            .bearer_auth(&provider.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(provider_error)?;

        response.json::<serde_json::Value>().await.map_err(provider_error)
    }
}
```

- [ ] **Step 5: Update `Executor` to resolve the full step, build the new request, and record profile/provider metadata in attempts**

```rust
let resolved = self
    .step_registry
    .resolve(step_key, step_version)
    .ok_or_else(|| AppError::Analysis {
        message: format!("missing step registration: {step_key}:{step_version}"),
        source: None,
    })?;

let llm_request = LlmRequest {
    provider: resolved.profile.provider.clone(),
    model: resolved.profile.model.clone(),
    system_prompt: resolved.prompt.system_prompt.clone(),
    developer_instructions: resolved.prompt.developer_instructions.clone(),
    input_json: input_json.clone(),
    max_tokens: resolved.profile.max_tokens,
    timeout_secs: resolved.profile.timeout_secs,
    structured_output_mode: choose_structured_output_mode(resolved.profile),
};
```

- [ ] **Step 6: Run executor and worker tests to verify profile-driven request metadata and retry handling still pass**

Run: `cargo test -p pa-orchestrator --test executor --test worker`

Expected: PASS with profile-resolution tests, schema-validation tests, and worker retry/dead-letter tests

- [ ] **Step 7: Commit the transport and executor slice**

```bash
git add crates/pa-orchestrator/src/llm.rs crates/pa-orchestrator/src/openai_client.rs crates/pa-orchestrator/src/executor.rs crates/pa-orchestrator/src/worker.rs crates/pa-orchestrator/tests/executor.rs crates/pa-orchestrator/tests/worker.rs
git commit -m "feat: add openai-compatible llm execution"
```

### Task 4: Add `shared_pa_state_bar` as a First-Class Shared Asset

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\models.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\prompt_specs.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\task_factory.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\repository.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\tests\pa_state_task.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\tests\task_factory.rs`

- [ ] **Step 1: Write the failing task-factory test for closed-bar dedupe and open-bar repeatability**

```rust
use pa_analysis::{build_shared_pa_state_bar_task, SharedPaStateBarInput};
use chrono::{DateTime, Utc};
use pa_core::Timeframe;
use pa_orchestrator::AnalysisBarState;

#[test]
fn closed_pa_state_task_has_dedupe_key_and_open_task_does_not() {
    let bar_open_time = DateTime::parse_from_rfc3339("2026-04-21T01:45:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let bar_close_time = DateTime::parse_from_rfc3339("2026-04-21T02:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let closed = build_shared_pa_state_bar_task(SharedPaStateBarInput {
        instrument_id: uuid::Uuid::nil(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Closed,
        bar_open_time,
        bar_close_time,
        bar_json: serde_json::json!({"kind":"canonical_closed_bar"}),
        market_context_json: serde_json::json!({"market":{"market_code":"crypto"}}),
    }).unwrap();

    let open = build_shared_pa_state_bar_task(SharedPaStateBarInput {
        instrument_id: uuid::Uuid::nil(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Open,
        bar_open_time,
        bar_close_time,
        bar_json: serde_json::json!({"kind":"derived_open_bar"}),
        market_context_json: serde_json::json!({"market":{"market_code":"crypto"}}),
    }).unwrap();

    assert!(closed.task.dedupe_key.is_some());
    assert!(open.task.dedupe_key.is_none());
}
```

- [ ] **Step 2: Run the test to verify it fails because PA-state types and task factories do not exist**

Run: `cargo test -p pa-analysis closed_pa_state_task_has_dedupe_key_and_open_task_does_not -- --exact`

Expected: FAIL with unresolved import or missing function errors for `SharedPaStateBarInput` and `build_shared_pa_state_bar_task`

- [ ] **Step 3: Add typed PA-state input/output models with the agreed decision-tree fields**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedPaStateBarInput {
    pub instrument_id: Uuid,
    #[serde(with = "timeframe_serde")]
    pub timeframe: Timeframe,
    #[serde(with = "bar_state_serde")]
    pub bar_state: AnalysisBarState,
    pub bar_open_time: DateTime<Utc>,
    pub bar_close_time: DateTime<Utc>,
    pub bar_json: Value,
    pub market_context_json: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedPaStateBarOutput {
    pub bar_identity: Value,
    pub market_session_context: Value,
    pub bar_observation: Value,
    pub bar_shape: Value,
    pub location_context: Value,
    pub multi_timeframe_alignment: Value,
    pub support_resistance_map: Value,
    pub signal_assessment: Value,
    pub decision_tree_state: Value,
    pub evidence_log: Value,
}
```

- [ ] **Step 4: Define the `shared_pa_state_bar_v1` step and prompt template**

```rust
pub fn shared_pa_state_bar_v1() -> AnalysisStepSpec {
    AnalysisStepSpec {
        step_key: "shared_pa_state_bar".into(),
        step_version: "v1".into(),
        task_type: "shared_pa_state_bar".into(),
        input_schema_version: "v1".into(),
        output_schema_version: "v1".into(),
        output_json_schema: serde_json::json!({
            "type":"object",
            "required":[
                "bar_identity",
                "market_session_context",
                "bar_observation",
                "bar_shape",
                "location_context",
                "multi_timeframe_alignment",
                "support_resistance_map",
                "signal_assessment",
                "decision_tree_state",
                "evidence_log"
            ],
            "properties": {
                "decision_tree_state": {
                    "type":"object",
                    "required":[
                        "trend_context",
                        "location_context",
                        "signal_quality",
                        "confirmation_state",
                        "invalidation_conditions",
                        "bias_balance"
                    ]
                }
            }
        }),
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Closed, AnalysisBarState::Open],
        dependency_policy: "market_runtime_only".into(),
    }
}
```

- [ ] **Step 5: Implement a PA-state task factory and repository helpers**

```rust
pub fn build_shared_pa_state_bar_task(input: SharedPaStateBarInput) -> Result<TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc::now();
    let input_json = serialize_snapshot_input(&input, "shared pa state input")?;
    let input_hash = sha256_json(&input_json)?;
    let dedupe_key = match input.bar_state {
        AnalysisBarState::Closed => Some(format!(
            "shared_pa_state_bar:{}:{}:{}:{}:{}",
            input.instrument_id,
            input.timeframe.as_str(),
            input.bar_close_time.to_rfc3339(),
            "shared_pa_state_bar",
            "v1"
        )),
        AnalysisBarState::Open => None,
        AnalysisBarState::None => unreachable!("pa state bar task requires open or closed"),
    };

    Ok(TaskEnvelope {
        task: AnalysisTask {
            id: task_id,
            task_type: "shared_pa_state_bar".to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id: input.instrument_id,
            user_id: None,
            timeframe: Some(input.timeframe),
            bar_state: input.bar_state,
            bar_open_time: Some(input.bar_open_time),
            bar_close_time: Some(input.bar_close_time),
            trading_date: None,
            trigger_type: "event".to_string(),
            prompt_key: "shared_pa_state_bar".to_string(),
            prompt_version: "v1".to_string(),
            snapshot_id,
            dedupe_key,
            attempt_count: 0,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            scheduled_at,
            started_at: None,
            finished_at: None,
            last_error_code: None,
            last_error_message: None,
        },
        snapshot: AnalysisSnapshot {
            id: snapshot_id,
            task_id,
            input_json,
            input_hash,
            schema_version: "v1".to_string(),
            created_at: scheduled_at,
        },
    })
  }
  ```

- [ ] **Step 6: Run the PA-analysis tests covering the new asset and task semantics**

Run: `cargo test -p pa-analysis --test task_factory --test pa_state_task`

Expected: PASS with closed/open dedupe behavior and repository idempotency for the new asset

- [ ] **Step 7: Commit the PA-state domain slice**

```bash
git add crates/pa-analysis/src/models.rs crates/pa-analysis/src/prompt_specs.rs crates/pa-analysis/src/task_factory.rs crates/pa-analysis/src/repository.rs crates/pa-analysis/src/lib.rs crates/pa-analysis/tests/task_factory.rs crates/pa-analysis/tests/pa_state_task.rs
git commit -m "feat: add shared pa state shared asset"
```

### Task 5: Rebase Shared Bar and Shared Daily Tasks on Top of PA State

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\models.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\prompt_specs.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-analysis\src\task_factory.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\analysis_runtime.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\analysis.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\tests\smoke.rs`

- [ ] **Step 1: Write the failing API smoke test for the new shared-asset chain**

```rust
#[tokio::test]
async fn analysis_routes_create_pa_state_before_shared_outputs() {
    let app = app_router(AppState::fixture());

    let pa_state = request_json(
        &app,
        Method::POST,
        "/analysis/shared/pa-state/bar",
        r#"{
            "instrument_id":"00000000-0000-0000-0000-000000000001",
            "timeframe":"15m",
            "bar_state":"closed",
            "bar_open_time":"2026-04-21T01:45:00Z",
            "bar_close_time":"2026-04-21T02:00:00Z",
            "bar_json":{"kind":"canonical_closed_bar"},
            "market_context_json":{"market":{"market_code":"crypto"}}
        }"#,
    ).await;

    assert_eq!(pa_state.status(), StatusCode::ACCEPTED);
}
```

- [ ] **Step 2: Run the smoke test to verify it fails because the PA-state route is missing**

Run: `cargo test -p pa-api analysis_routes_create_pa_state_before_shared_outputs -- --exact`

Expected: FAIL with a 404 response on `/analysis/shared/pa-state/bar`

- [ ] **Step 3: Update shared bar and shared daily input models to carry PA-state references and evidence arrays**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedBarAnalysisInput {
    pub instrument_id: Uuid,
    #[serde(with = "timeframe_serde")]
    pub timeframe: Timeframe,
    pub bar_open_time: DateTime<Utc>,
    pub bar_close_time: DateTime<Utc>,
    #[serde(with = "bar_state_serde")]
    pub bar_state: AnalysisBarState,
    pub shared_pa_state_json: Value,
    pub recent_pa_states_json: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedDailyContextInput {
    pub instrument_id: Uuid,
    pub trading_date: NaiveDate,
    pub recent_pa_states_json: Value,
    pub recent_shared_bar_analyses_json: Value,
    pub multi_timeframe_structure_json: Value,
    pub market_background_json: Value,
}
```

- [ ] **Step 4: Update prompt specs for `shared_bar_analysis_v2` and `shared_daily_context_v2`**

```rust
pub fn shared_bar_analysis_v2() -> AnalysisStepSpec {
    AnalysisStepSpec {
        step_key: "shared_bar_analysis".into(),
        step_version: "v2".into(),
        task_type: "shared_bar_analysis".into(),
        input_schema_version: "v2".into(),
        output_schema_version: "v2".into(),
        output_json_schema: serde_json::json!({
            "type":"object",
            "required":[
                "bar_identity",
                "bar_summary",
                "market_story",
                "bullish_case",
                "bearish_case",
                "two_sided_balance",
                "key_levels",
                "signal_bar_verdict",
                "continuation_path",
                "reversal_path",
                "invalidation_map",
                "follow_through_checkpoints"
            ]
        }),
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Closed, AnalysisBarState::Open],
        dependency_policy: "requires_pa_state".into(),
    }
}
```

- [ ] **Step 5: Implement runtime assembly that auto-resolves PA-state assets before building shared bar/daily snapshots**

```rust
let pa_state_json = match request.shared_pa_state_json {
    Some(value) => value,
    None => find_matching_shared_pa_state(
        state,
        request.instrument_id,
        timeframe,
        bar_state,
        resolved.bar_open_time,
        resolved.bar_close_time,
    )?,
};

Ok(SharedBarAnalysisInput {
    instrument_id: request.instrument_id,
    timeframe,
    bar_open_time: resolved.bar_open_time,
    bar_close_time: resolved.bar_close_time,
    bar_state,
    shared_pa_state_json: pa_state_json,
    recent_pa_states_json: collect_recent_shared_pa_states(state, request.instrument_id, timeframe, 8),
})
```

- [ ] **Step 6: Add the new PA-state route and extend shared bar/shared daily route tests**

Run: `cargo test -p pa-api --test smoke`

Expected: PASS with the new PA-state creation flow and shared bar/shared daily runtime assembly coverage

- [ ] **Step 7: Commit the shared-asset wiring slice**

```bash
git add crates/pa-analysis/src/models.rs crates/pa-analysis/src/prompt_specs.rs crates/pa-analysis/src/task_factory.rs crates/pa-api/src/analysis_runtime.rs crates/pa-api/src/analysis.rs crates/pa-api/tests/smoke.rs
git commit -m "feat: wire shared analysis through pa state"
```

### Task 6: Update User Analysis to Consume the New Shared Asset Chain

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-user\src\models.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-user\src\prompt_specs.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-user\src\task_factory.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\analysis_runtime.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-user\tests\task_factory.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-user\tests\manual_user_analysis.rs`

- [ ] **Step 1: Write the failing manual-user-analysis test for PA-state-aware snapshots**

```rust
#[tokio::test]
async fn manual_user_analysis_includes_shared_pa_state_when_present() {
    let request = ManualUserAnalysisInput {
        user_id: uuid::Uuid::nil(),
        instrument_id: uuid::Uuid::nil(),
        timeframe: pa_core::Timeframe::M15,
        bar_state: pa_orchestrator::AnalysisBarState::Closed,
        bar_open_time: Some(chrono::DateTime::parse_from_rfc3339("2026-04-21T01:45:00Z").unwrap().with_timezone(&chrono::Utc)),
        bar_close_time: Some(chrono::DateTime::parse_from_rfc3339("2026-04-21T02:00:00Z").unwrap().with_timezone(&chrono::Utc)),
        trading_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 4, 21).unwrap()),
        positions_json: serde_json::json!([]),
        subscriptions_json: serde_json::json!([]),
        shared_pa_state_json: serde_json::json!({"decision_tree_state":{}}),
        shared_bar_analysis_json: serde_json::json!({"bullish_case":{},"bearish_case":{}}),
        shared_daily_context_json: serde_json::json!({"decision_tree_nodes":{}}),
    };

    let envelope = build_manual_user_analysis_task(request).unwrap();
    assert!(envelope.snapshot.input_json["shared_pa_state_json"].is_object());
}
```

- [ ] **Step 2: Run the user tests to verify they fail because user snapshots do not carry PA state yet**

Run: `cargo test -p pa-user --test task_factory --test manual_user_analysis`

Expected: FAIL with unknown field or missing field errors for `shared_pa_state_json`

- [ ] **Step 3: Extend user input models and prompt output contract to include PA-state evidence**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualUserAnalysisInput {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    #[serde(with = "timeframe_serde")]
    pub timeframe: Timeframe,
    #[serde(with = "bar_state_serde")]
    pub bar_state: AnalysisBarState,
    pub bar_open_time: Option<DateTime<Utc>>,
    pub bar_close_time: Option<DateTime<Utc>>,
    pub trading_date: Option<NaiveDate>,
    pub positions_json: Value,
    pub subscriptions_json: Value,
    pub shared_pa_state_json: Value,
    pub shared_bar_analysis_json: Value,
    pub shared_daily_context_json: Value,
}
```

- [ ] **Step 4: Update manual and scheduled task factories to hash PA-state evidence into the dedupe context**

```rust
let context_hash = sha256_json(&serde_json::json!({
    "positions": positions_json,
    "subscriptions": subscriptions_json,
    "shared_pa_state": shared_pa_state_json,
    "shared_bar_analysis": shared_bar_analysis_json,
    "shared_daily_context": shared_daily_context_json,
}))?;
```

- [ ] **Step 5: Update runtime assembly so manual user analysis auto-resolves matching PA-state assets when callers omit them**

```rust
let shared_pa_state_json = match request.shared_pa_state_json {
    Some(value) => value,
    None => find_matching_shared_pa_state(
        state,
        request.instrument_id,
        timeframe,
        bar_state,
        resolved.bar_open_time,
        resolved.bar_close_time,
    )?,
};
```

- [ ] **Step 6: Run the user-analysis tests to verify the new shared-asset chain is reflected in snapshots and service behavior**

Run: `cargo test -p pa-user --test task_factory --test manual_user_analysis`

Expected: PASS with updated snapshot serialization and missing-shared-asset error coverage

- [ ] **Step 7: Commit the user-analysis compatibility slice**

```bash
git add crates/pa-user/src/models.rs crates/pa-user/src/prompt_specs.rs crates/pa-user/src/task_factory.rs crates/pa-api/src/analysis_runtime.rs crates/pa-user/tests/task_factory.rs crates/pa-user/tests/manual_user_analysis.rs
git commit -m "feat: connect user analysis to pa state chain"
```

### Task 7: Wire the App and Worker to Real Step Bindings and Provider Profiles

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-app\src\main.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-app\src\lib.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\src\router.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-api\tests\smoke.rs`

- [ ] **Step 1: Write the failing app bootstrap test for step registration and provider wiring**

```rust
#[tokio::test]
async fn app_bootstrap_registers_all_analysis_steps() {
    let registry = build_step_registry_from_config(&pa_core::config::load_from_path("config.example.toml").unwrap()).unwrap();

    assert!(registry.resolve("shared_pa_state_bar", "v1").is_some());
    assert!(registry.resolve("shared_bar_analysis", "v2").is_some());
    assert!(registry.resolve("shared_daily_context", "v2").is_some());
    assert!(registry.resolve("user_position_advice", "v2").is_some());
}
```

- [ ] **Step 2: Run the test to verify it fails because the app bootstrap still constructs a prompt-only registry**

Run: `cargo test -p pa-app app_bootstrap_registers_all_analysis_steps -- --exact`

Expected: FAIL because `build_step_registry_from_config` does not exist and the app still uses `PromptRegistry`

- [ ] **Step 3: Add bootstrap helpers that translate config into provider runtimes, execution profiles, and bindings**

```rust
fn build_step_registry_from_config(config: &pa_core::config::AppConfig) -> Result<pa_orchestrator::StepRegistry, AppError> {
    pa_orchestrator::StepRegistry::default()
        .with_step(pa_analysis::shared_pa_state_bar_v1())?
        .with_prompt_template(pa_analysis::shared_pa_state_bar_prompt_v1())?
        .with_step(pa_analysis::shared_bar_analysis_v2())?
        .with_prompt_template(pa_analysis::shared_bar_analysis_prompt_v2())?
        .with_step(pa_analysis::shared_daily_context_v2())?
        .with_prompt_template(pa_analysis::shared_daily_context_prompt_v2())?
        .with_step(pa_user::user_position_advice_v2())?
        .with_prompt_template(pa_user::user_position_advice_prompt_v2())?
        .with_profiles_from_config(&config.llm)?
        .with_bindings_from_config(&config.llm)
}
```

- [ ] **Step 4: Replace the fixture-only executor bootstrap with an OpenAI-compatible client in non-test startup**

```rust
let step_registry = build_step_registry_from_config(&config)?;
let llm_client = pa_orchestrator::OpenAiCompatibleClient::from_config(&config.llm)?;
let worker_executor = Executor::new(step_registry, llm_client);
```

- [ ] **Step 5: Run app and smoke tests to verify the application still boots and analysis routes are still wired**

Run: `cargo test -p pa-app && cargo test -p pa-api --test smoke`

Expected: PASS with the new registry bootstrap and unchanged API bootstrap semantics

- [ ] **Step 6: Commit the app bootstrap slice**

```bash
git add crates/pa-app/src/main.rs crates/pa-api/src/router.rs crates/pa-api/tests/smoke.rs
git commit -m "feat: bootstrap step-bound llm execution"
```

### Task 8: Add Replay and Experiment Logging for Historical Evaluation

**Files:**
- Modify: `E:\rust-app\oh-paa\crates\pa-app\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-app\src\replay.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-app\src\bin\replay_analysis.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-app\tests\replay.rs`
- Create: `E:\rust-app\oh-paa\testdata\analysis_replay\sample_set.json`
- Modify: `E:\rust-app\oh-paa\docs\architecture\phase1-runtime.md`

- [ ] **Step 1: Write the failing replay test for variant execution and experiment logging**

```rust
#[tokio::test]
async fn replay_runner_records_variant_step_outputs_and_scores() {
    let report = pa_app::replay::run_replay_variant_from_path(
        "testdata/analysis_replay/sample_set.json",
        "baseline_a",
    ).await.unwrap();

    assert_eq!(report.dataset_id, "sample_set");
    assert_eq!(report.pipeline_variant, "baseline_a");
    assert!(!report.step_runs.is_empty());
    assert!(report.programmatic_scores["schema_hit_rate"].as_f64().unwrap() >= 0.0);
}
```

- [ ] **Step 2: Run the test to verify it fails because replay tooling does not exist**

Run: `cargo test -p pa-app replay_runner_records_variant_step_outputs_and_scores -- --exact`

Expected: FAIL with missing module or missing function errors for `pa_app::replay`

- [ ] **Step 3: Add replay models and a runner that executes named variants against historical samples**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayExperimentReport {
    pub experiment_id: String,
    pub dataset_id: String,
    pub pipeline_variant: String,
    pub step_runs: Vec<ReplayStepRun>,
    pub programmatic_scores: serde_json::Map<String, serde_json::Value>,
}

pub async fn run_replay_variant_from_path(
    path: impl AsRef<std::path::Path>,
    pipeline_variant: &str,
) -> Result<ReplayExperimentReport, AppError> {
    let dataset = load_replay_dataset(path)?;
    let step_runs = execute_variant(dataset.clone(), pipeline_variant).await?;
    let programmatic_scores = score_step_runs(&step_runs);

    Ok(ReplayExperimentReport {
        experiment_id: uuid::Uuid::new_v4().to_string(),
        dataset_id: dataset.dataset_id,
        pipeline_variant: pipeline_variant.to_string(),
        step_runs,
        programmatic_scores,
    })
}
```

- [ ] **Step 4: Add a sample replay dataset with A-share, crypto, and forex cases**

```json
{
  "dataset_id": "sample_set",
  "samples": [
    {
      "sample_id": "crypto-btc-15m-breakout",
      "market": "crypto",
      "timeframe": "15m",
      "shared_pa_state_input": {
        "instrument_id": "22222222-2222-2222-2222-222222222202",
        "bar_state": "closed"
      }
    },
    {
      "sample_id": "cn-a-1h-rejection",
      "market": "cn_a",
      "timeframe": "1h",
      "shared_pa_state_input": {
        "instrument_id": "11111111-1111-1111-1111-111111111101",
        "bar_state": "closed"
      }
    }
  ]
}
```

- [ ] **Step 5: Add a CLI entry point that writes the replay report to stdout as JSON**

```rust
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let dataset = std::env::args().nth(1).expect("dataset path");
    let variant = std::env::args().nth(2).expect("pipeline variant");
    let report = pa_app::replay::run_replay_variant_from_path(dataset, &variant).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
```

- [ ] **Step 6: Run the replay tests and the CLI against the sample dataset**

Run: `cargo test -p pa-app --test replay && cargo run -p pa-app --bin replay_analysis -- testdata/analysis_replay/sample_set.json baseline_a`

Expected: PASS for the replay test and a JSON report on stdout containing `dataset_id`, `pipeline_variant`, `step_runs`, and `programmatic_scores`

- [ ] **Step 7: Commit the replay and experiment slice**

```bash
git add crates/pa-app/src/replay.rs crates/pa-app/src/bin/replay_analysis.rs crates/pa-app/tests/replay.rs testdata/analysis_replay/sample_set.json docs/architecture/phase1-runtime.md
git commit -m "feat: add analysis replay evaluation tooling"
```

### Task 9: Final Verification

**Files:**
- Modify: `E:\rust-app\oh-paa\docs\superpowers\plans\2026-04-22-analysis-pipeline-optimization.md`

- [ ] **Step 1: Run the full workspace test suite**

Run: `cargo test --workspace`

Expected: PASS with shared analysis, user analysis, orchestrator, market runtime, and replay coverage all green

- [ ] **Step 2: Run the workspace linter**

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: PASS with zero warnings

- [ ] **Step 3: Run a real replay smoke using the configured historical dataset**

Run: `cargo run -p pa-app --bin replay_analysis -- testdata/analysis_replay/sample_set.json baseline_a`

Expected: PASS with a JSON report that includes `schema_hit_rate`, `pipeline_variant`, and per-step outputs

- [ ] **Step 4: Update this plan file to mark completed checkboxes only after verification passes**

```markdown
- [x] **Step 1: Run the full workspace test suite**
- [x] **Step 2: Run the workspace linter**
- [x] **Step 3: Run a real replay smoke using the configured historical dataset**
```

- [ ] **Step 5: Commit the verified implementation result**

```bash
git add -A
git commit -m "feat: add layered analysis pipeline optimization flow"
```
