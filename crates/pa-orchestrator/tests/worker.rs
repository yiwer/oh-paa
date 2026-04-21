use chrono::{TimeZone, Utc};
use pa_core::Timeframe;
use pa_orchestrator::{
    AnalysisBarState, AnalysisSnapshot, AnalysisStepSpec, AnalysisTask, AnalysisTaskStatus,
    Executor, FixtureLlmClient, InMemoryOrchestrationRepository, ModelExecutionProfile,
    OrchestrationRepository, PromptResultSemantics, PromptTemplateSpec, StepExecutionBinding,
    StepRegistry, run_single_task,
};
use uuid::Uuid;

fn make_registry(output_json_schema: serde_json::Value) -> StepRegistry {
    StepRegistry::default()
        .with_step(AnalysisStepSpec {
            step_key: "shared_bar_analysis".to_string(),
            step_version: "v1".to_string(),
            task_type: "shared_bar_analysis".to_string(),
            input_schema_version: "v1".to_string(),
            output_schema_version: "v1".to_string(),
            output_json_schema,
            result_semantics: PromptResultSemantics::SharedAsset,
            bar_state_support: vec![AnalysisBarState::Closed],
            dependency_policy: "market_runtime_only".to_string(),
        })
        .unwrap()
        .with_prompt_template(PromptTemplateSpec {
            step_key: "shared_bar_analysis".to_string(),
            step_version: "v1".to_string(),
            system_prompt: "Return JSON only".to_string(),
            developer_instructions: vec![],
        })
        .unwrap()
        .with_execution_profile(ModelExecutionProfile {
            profile_key: "analysis_fixture_profile".to_string(),
            provider: "fixture".to_string(),
            model: "fixture-json".to_string(),
            max_tokens: 4096,
            timeout_secs: 60,
            max_retries: 1,
            retry_initial_backoff_ms: 200,
            supports_json_schema: true,
            supports_reasoning: false,
        })
        .unwrap()
        .with_binding(StepExecutionBinding {
            step_key: "shared_bar_analysis".to_string(),
            step_version: "v1".to_string(),
            execution_profile: "analysis_fixture_profile".to_string(),
        })
        .unwrap()
}

fn make_task_and_snapshot(max_attempts: u32) -> (AnalysisTask, AnalysisSnapshot) {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap();
    let created_at = Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 1).unwrap();

    (
        AnalysisTask {
            id: task_id,
            task_type: "shared_bar_analysis".to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id: Uuid::new_v4(),
            user_id: None,
            timeframe: Some(Timeframe::M15),
            bar_state: AnalysisBarState::Closed,
            bar_open_time: None,
            bar_close_time: Some(Utc.with_ymd_and_hms(2026, 4, 21, 4, 0, 0).unwrap()),
            trading_date: None,
            trigger_type: "event".to_string(),
            prompt_key: "shared_bar_analysis".to_string(),
            prompt_version: "v1".to_string(),
            snapshot_id,
            dedupe_key: Some("shared:bar:m15:2026-04-21T04:00:00Z".to_string()),
            attempt_count: 0,
            max_attempts,
            scheduled_at,
            started_at: None,
            finished_at: None,
            last_error_code: None,
            last_error_message: None,
        },
        AnalysisSnapshot {
            id: snapshot_id,
            task_id,
            input_json: serde_json::json!({
                "instrument_id": "x",
                "timeframe": "m15"
            }),
            input_hash: "abc123".to_string(),
            schema_version: "v1".to_string(),
            created_at,
        },
    )
}

#[tokio::test]
async fn claim_next_pending_task_transitions_to_running_atomically() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(3);
    repository
        .insert_task_with_snapshot(task, snapshot)
        .await
        .unwrap();

    let first_claim = repository.claim_next_pending_task().await.unwrap();
    let second_claim = repository.claim_next_pending_task().await.unwrap();

    assert!(first_claim.is_some());
    assert!(second_claim.is_none());

    let persisted = repository.only_task();
    assert_eq!(persisted.status, AnalysisTaskStatus::Running);
    assert!(persisted.started_at.is_some());
}

#[tokio::test]
async fn worker_success_path_persists_result_and_completes_task() {
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
    let registry = make_registry(serde_json::json!({
        "type": "object",
        "required": ["bullish_case", "bearish_case"],
        "properties": {
            "bullish_case": { "type": "object" },
            "bearish_case": { "type": "object" }
        }
    }));
    let executor = Executor::new(registry, FixtureLlmClient::with_json(output.clone()));

    let consumed = run_single_task(&repository, &executor).await.unwrap();
    assert!(consumed);

    let task = repository.only_task();
    assert_eq!(task.status, AnalysisTaskStatus::Succeeded);
    assert_eq!(task.attempt_count, 1);
    assert!(task.finished_at.is_some());
    assert_eq!(repository.results().len(), 1);
    assert_eq!(repository.results()[0].output_json, output);
    assert_eq!(repository.attempts().len(), 1);
    assert_eq!(repository.attempts()[0].status, "succeeded");
    assert_eq!(repository.dead_letters().len(), 0);
}

#[tokio::test]
async fn worker_retryable_failure_returns_task_to_retry_waiting() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(3);
    repository
        .insert_task_with_snapshot(task, snapshot)
        .await
        .unwrap();

    let registry = make_registry(serde_json::json!({"type": "object"}));
    let executor = Executor::new(
        registry,
        FixtureLlmClient::with_provider_error("provider request timed out"),
    );

    let consumed = run_single_task(&repository, &executor).await.unwrap();
    assert!(consumed);

    let task = repository.only_task();
    assert_eq!(task.status, AnalysisTaskStatus::RetryWaiting);
    assert_eq!(task.attempt_count, 1);
    assert!(task.last_error_message.is_some());
    assert_eq!(repository.results().len(), 0);
    assert_eq!(repository.attempts().len(), 1);
    assert_eq!(repository.attempts()[0].status, "outbound_failed");
    assert_eq!(repository.dead_letters().len(), 0);
}

#[tokio::test]
async fn worker_can_reclaim_retry_waiting_tasks() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(2);
    repository
        .insert_task_with_snapshot(task, snapshot)
        .await
        .unwrap();

    let registry = make_registry(serde_json::json!({"type": "object"}));
    let executor = Executor::new(
        registry,
        FixtureLlmClient::with_provider_error("provider request timed out"),
    );

    let first = run_single_task(&repository, &executor).await.unwrap();
    let second = run_single_task(&repository, &executor).await.unwrap();
    assert!(first);
    assert!(second);

    let task = repository.only_task();
    assert_eq!(task.status, AnalysisTaskStatus::DeadLetter);
    assert_eq!(task.attempt_count, 2);
    assert_eq!(repository.attempts().len(), 2);
    assert_eq!(repository.dead_letters().len(), 1);
}

#[tokio::test]
async fn worker_retry_exhaustion_moves_task_to_dead_letter() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(1);
    repository
        .insert_task_with_snapshot(task, snapshot)
        .await
        .unwrap();

    let registry = make_registry(serde_json::json!({"type": "object"}));
    let executor = Executor::new(
        registry,
        FixtureLlmClient::with_provider_error("provider request timed out"),
    );

    let consumed = run_single_task(&repository, &executor).await.unwrap();
    assert!(consumed);

    let task = repository.only_task();
    assert_eq!(task.status, AnalysisTaskStatus::DeadLetter);
    assert_eq!(task.attempt_count, 1);
    assert_eq!(repository.results().len(), 0);
    assert_eq!(repository.attempts().len(), 1);
    assert_eq!(repository.dead_letters().len(), 1);
    assert_eq!(repository.dead_letters()[0].task_id, task.id);
    assert_eq!(
        repository.dead_letters()[0].last_attempt_id,
        Some(repository.attempts()[0].id)
    );
}

#[tokio::test]
async fn worker_non_retryable_validation_failure_marks_terminal_failed() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(3);
    repository
        .insert_task_with_snapshot(task, snapshot)
        .await
        .unwrap();

    let registry = make_registry(serde_json::json!({
        "type": "object",
        "required": ["bullish_case", "bearish_case"],
        "properties": {
            "bullish_case": { "type": "object" },
            "bearish_case": { "type": "object" }
        }
    }));
    let executor = Executor::new(
        registry,
        FixtureLlmClient::with_json(serde_json::json!({"bullish_case": {}})),
    );

    let consumed = run_single_task(&repository, &executor).await.unwrap();
    assert!(consumed);

    let task = repository.only_task();
    assert_eq!(task.status, AnalysisTaskStatus::Failed);
    assert_eq!(task.attempt_count, 1);
    assert!(task.last_error_message.is_some());
    assert_eq!(repository.results().len(), 0);
    assert_eq!(repository.attempts().len(), 1);
    assert_eq!(repository.attempts()[0].status, "schema_validation_failed");
    assert_eq!(repository.dead_letters().len(), 0);
}

#[tokio::test]
async fn worker_releases_claim_when_snapshot_load_fails() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(3);
    let snapshot_id = snapshot.id;
    repository
        .insert_task_with_snapshot(task, snapshot)
        .await
        .unwrap();
    repository.remove_snapshot(snapshot_id);

    let registry = make_registry(serde_json::json!({"type": "object"}));
    let executor = Executor::new(registry, FixtureLlmClient::with_json(serde_json::json!({})));

    let err = run_single_task(&repository, &executor).await.unwrap_err();
    assert!(err.to_string().contains("snapshot not found"));

    let task = repository.only_task();
    assert_eq!(task.status, AnalysisTaskStatus::Pending);
    assert_eq!(task.attempt_count, 0);
    assert_eq!(repository.attempts().len(), 0);
}

#[tokio::test]
async fn worker_outcome_persist_failure_does_not_leave_partial_side_effects() {
    let repository = InMemoryOrchestrationRepository::default();
    let (task, snapshot) = make_task_and_snapshot(3);
    repository
        .insert_task_with_snapshot(task, snapshot)
        .await
        .unwrap();
    repository.fail_next_outcome_persist();

    let output = serde_json::json!({
        "bullish_case": {"entry": "breakout"},
        "bearish_case": {"entry": "pullback"}
    });
    let registry = make_registry(serde_json::json!({
        "type": "object",
        "required": ["bullish_case", "bearish_case"],
        "properties": {
            "bullish_case": { "type": "object" },
            "bearish_case": { "type": "object" }
        }
    }));
    let executor = Executor::new(registry, FixtureLlmClient::with_json(output));

    let err = run_single_task(&repository, &executor).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("in-memory injected outcome persist failure")
    );

    let task = repository.only_task();
    assert_eq!(task.status, AnalysisTaskStatus::Pending);
    assert_eq!(task.attempt_count, 0);
    assert_eq!(repository.attempts().len(), 0);
    assert_eq!(repository.results().len(), 0);
    assert_eq!(repository.dead_letters().len(), 0);
}
