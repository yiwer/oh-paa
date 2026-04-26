use chrono::{DateTime, TimeZone, Utc};
use pa_core::Timeframe;
use pa_orchestrator::{
    AnalysisAttempt, AnalysisBarState, AnalysisDeadLetter, AnalysisResult, AnalysisSnapshot,
    AnalysisTask, AnalysisTaskStatus, InsertTaskResult, OrchestrationRepository,
    PgOrchestrationRepository,
};
use sqlx::PgPool;
use std::path::Path;
use uuid::Uuid;

#[tokio::test]
async fn pg_repository_inserts_task_snapshot_and_queries_task_views() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_inserts_task_snapshot_and_queries_task_views: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (task, snapshot) = fixture_task_and_snapshot(instrument_id);

    let insert_result = repository
        .insert_task_with_snapshot(task.clone(), snapshot.clone())
        .await
        .expect("task and snapshot insert should succeed");

    assert_eq!(insert_result, InsertTaskResult::Inserted);
    assert_eq!(repository.task(task.id).await.unwrap(), Some(task.clone()));
    assert_eq!(
        repository.load_snapshot(snapshot.id).await.unwrap(),
        snapshot.clone()
    );
    assert!(repository.attempts_for_task(task.id).await.unwrap().is_empty());
    assert_eq!(repository.result_for_task(task.id).await.unwrap(), None);
    assert_eq!(repository.dead_letter_for_task(task.id).await.unwrap(), None);

    let seeded_attempt = seed_attempt(&pool, task.id).await;
    let seeded_result = seed_result(&pool, &task).await;
    let seeded_dead_letter = seed_dead_letter(&pool, task.id, seeded_attempt.id).await;

    assert_eq!(
        repository.attempts_for_task(task.id).await.unwrap(),
        vec![seeded_attempt]
    );
    assert_eq!(
        repository.result_for_task(task.id).await.unwrap(),
        Some(seeded_result)
    );
    assert_eq!(
        repository.dead_letter_for_task(task.id).await.unwrap(),
        Some(seeded_dead_letter)
    );

    cleanup_task_graph(&pool, task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_claim_next_pending_task_transitions_one_row_to_running() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_claim_next_pending_task_transitions_one_row_to_running: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (first_task, first_snapshot) = fixture_task_and_snapshot(instrument_id);
    let (mut second_task, second_snapshot) = fixture_task_and_snapshot_with_dedupe(
        instrument_id,
        "pg:test:closed:shared:m15:second",
    );
    second_task.scheduled_at = utc("2026-04-21T10:05:00Z");
    second_task.status = AnalysisTaskStatus::RetryWaiting;

    repository
        .insert_task_with_snapshot(first_task.clone(), first_snapshot)
        .await
        .expect("first task insert should succeed");
    repository
        .insert_task_with_snapshot(second_task.clone(), second_snapshot)
        .await
        .expect("second task insert should succeed");

    let claimed = repository
        .claim_next_pending_task()
        .await
        .expect("claim should succeed")
        .expect("one task should be claimed");

    assert_eq!(claimed.id, first_task.id);
    assert_eq!(claimed.status, AnalysisTaskStatus::Running);
    assert!(claimed.started_at.is_some());

    let persisted_first = repository
        .task(first_task.id)
        .await
        .expect("query should succeed")
        .expect("first task should exist");
    let persisted_second = repository
        .task(second_task.id)
        .await
        .expect("query should succeed")
        .expect("second task should exist");

    assert_eq!(persisted_first.status, AnalysisTaskStatus::Running);
    assert!(persisted_first.started_at.is_some());
    assert_eq!(persisted_second.status, AnalysisTaskStatus::RetryWaiting);
    assert!(persisted_second.started_at.is_none());

    cleanup_task_graph(&pool, first_task.id).await;
    cleanup_task_graph(&pool, second_task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_persist_success_outcome_writes_attempt_result_and_task_state() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_persist_success_outcome_writes_attempt_result_and_task_state: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (task, snapshot) = fixture_task_and_snapshot(instrument_id);
    repository
        .insert_task_with_snapshot(task.clone(), snapshot)
        .await
        .expect("task insert should succeed");
    let claimed = repository
        .claim_next_pending_task()
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");
    assert_eq!(claimed.status, AnalysisTaskStatus::Running);

    let attempt = fixture_attempt(task.id, 1, "succeeded");
    let result = fixture_result(&task, serde_json::json!({"summary": "analysis complete"}));

    repository
        .persist_success_outcome(task.id, attempt.clone(), result.clone())
        .await
        .expect("success outcome should persist");

    let persisted_task = repository
        .task(task.id)
        .await
        .expect("query should succeed")
        .expect("task should exist");
    let persisted_attempts = repository
        .attempts_for_task(task.id)
        .await
        .expect("attempt query should succeed");
    let persisted_result = repository
        .result_for_task(task.id)
        .await
        .expect("result query should succeed");

    assert_eq!(persisted_task.status, AnalysisTaskStatus::Succeeded);
    assert_eq!(persisted_task.attempt_count, 1);
    assert!(persisted_task.finished_at.is_some());
    assert_eq!(persisted_task.last_error_code, None);
    assert_eq!(persisted_task.last_error_message, None);
    assert_eq!(persisted_attempts, vec![attempt]);
    assert_eq!(persisted_result, Some(result));

    cleanup_task_graph(&pool, task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_recovers_stale_running_tasks_to_retry_waiting() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_recovers_stale_running_tasks_to_retry_waiting: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (mut stale_task, stale_snapshot) = fixture_task_and_snapshot(instrument_id);
    stale_task.status = AnalysisTaskStatus::Running;
    stale_task.started_at = Some(utc("2026-04-21T08:00:00Z"));
    let (mut fresh_task, fresh_snapshot) = fixture_task_and_snapshot_with_dedupe(
        instrument_id,
        "pg:test:closed:shared:m15:fresh",
    );
    fresh_task.status = AnalysisTaskStatus::Running;
    fresh_task.started_at = Some(utc("2026-04-21T11:30:00Z"));

    repository
        .insert_task_with_snapshot(stale_task.clone(), stale_snapshot)
        .await
        .expect("stale task insert should succeed");
    repository
        .insert_task_with_snapshot(fresh_task.clone(), fresh_snapshot)
        .await
        .expect("fresh task insert should succeed");

    let recovered = repository
        .recover_stale_running_tasks(
            utc("2026-04-21T10:00:00Z"),
            "startup_recovery",
            "Recovered stale running task on startup",
        )
        .await
        .expect("recovery should succeed");

    assert_eq!(recovered, 1);

    let persisted_stale = repository
        .task(stale_task.id)
        .await
        .expect("query should succeed")
        .expect("stale task should exist");
    let persisted_fresh = repository
        .task(fresh_task.id)
        .await
        .expect("query should succeed")
        .expect("fresh task should exist");

    assert_eq!(persisted_stale.status, AnalysisTaskStatus::RetryWaiting);
    assert_eq!(
        persisted_stale.last_error_code.as_deref(),
        Some("startup_recovery")
    );
    assert_eq!(
        persisted_stale.last_error_message.as_deref(),
        Some("Recovered stale running task on startup")
    );
    assert_eq!(persisted_fresh.status, AnalysisTaskStatus::Running);
    assert_eq!(persisted_fresh.last_error_code, None);
    assert_eq!(persisted_fresh.last_error_message, None);

    cleanup_task_graph(&pool, stale_task.id).await;
    cleanup_task_graph(&pool, fresh_task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_release_claimed_task_returns_running_task_to_pending() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_release_claimed_task_returns_running_task_to_pending: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (task, snapshot) = fixture_task_and_snapshot(instrument_id);
    repository
        .insert_task_with_snapshot(task.clone(), snapshot)
        .await
        .expect("task insert should succeed");
    repository
        .claim_next_pending_task()
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    repository
        .release_claimed_task(task.id, "snapshot load failed")
        .await
        .expect("release should succeed");

    let persisted_task = repository
        .task(task.id)
        .await
        .expect("query should succeed")
        .expect("task should exist");

    assert_eq!(persisted_task.status, AnalysisTaskStatus::Pending);
    assert!(persisted_task.started_at.is_some());
    assert_eq!(
        persisted_task.last_error_code.as_deref(),
        Some("claim_released")
    );
    assert_eq!(
        persisted_task.last_error_message.as_deref(),
        Some("snapshot load failed")
    );

    cleanup_task_graph(&pool, task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_persist_schema_validation_failure_marks_task_failed() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_persist_schema_validation_failure_marks_task_failed: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (task, snapshot) = fixture_task_and_snapshot(instrument_id);
    repository
        .insert_task_with_snapshot(task.clone(), snapshot)
        .await
        .expect("task insert should succeed");
    repository
        .claim_next_pending_task()
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    let mut attempt = fixture_attempt(task.id, 1, "schema_validation_failed");
    attempt.error_type = Some("validation".to_string());
    attempt.error_message = Some("missing bearish_case".to_string());

    repository
        .persist_schema_validation_failure_outcome(task.id, attempt.clone(), "missing bearish_case")
        .await
        .expect("schema validation failure should persist");

    let persisted_task = repository
        .task(task.id)
        .await
        .expect("query should succeed")
        .expect("task should exist");
    let persisted_attempts = repository
        .attempts_for_task(task.id)
        .await
        .expect("attempt query should succeed");

    assert_eq!(persisted_task.status, AnalysisTaskStatus::Failed);
    assert_eq!(persisted_task.attempt_count, 1);
    assert!(persisted_task.finished_at.is_some());
    assert_eq!(
        persisted_task.last_error_code.as_deref(),
        Some("terminal_error")
    );
    assert_eq!(
        persisted_task.last_error_message.as_deref(),
        Some("missing bearish_case")
    );
    assert_eq!(persisted_attempts, vec![attempt]);

    cleanup_task_graph(&pool, task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_persist_outbound_retry_marks_task_retry_waiting() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_persist_outbound_retry_marks_task_retry_waiting: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (task, snapshot) = fixture_task_and_snapshot(instrument_id);
    repository
        .insert_task_with_snapshot(task.clone(), snapshot)
        .await
        .expect("task insert should succeed");
    repository
        .claim_next_pending_task()
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    let mut attempt = fixture_attempt(task.id, 1, "outbound_failed");
    attempt.error_type = Some("provider".to_string());
    attempt.error_message = Some("provider request timed out".to_string());

    repository
        .persist_outbound_retry_outcome(task.id, attempt.clone(), "provider request timed out")
        .await
        .expect("retry outcome should persist");

    let persisted_task = repository
        .task(task.id)
        .await
        .expect("query should succeed")
        .expect("task should exist");
    let persisted_attempts = repository
        .attempts_for_task(task.id)
        .await
        .expect("attempt query should succeed");

    assert_eq!(persisted_task.status, AnalysisTaskStatus::RetryWaiting);
    assert_eq!(persisted_task.attempt_count, 1);
    assert_eq!(persisted_task.finished_at, None);
    assert_eq!(
        persisted_task.last_error_code.as_deref(),
        Some("retryable_error")
    );
    assert_eq!(
        persisted_task.last_error_message.as_deref(),
        Some("provider request timed out")
    );
    assert_eq!(persisted_attempts, vec![attempt]);

    cleanup_task_graph(&pool, task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_persist_outbound_terminal_failure_marks_task_failed() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_persist_outbound_terminal_failure_marks_task_failed: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (task, snapshot) = fixture_task_and_snapshot(instrument_id);
    repository
        .insert_task_with_snapshot(task.clone(), snapshot)
        .await
        .expect("task insert should succeed");
    repository
        .claim_next_pending_task()
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    let mut attempt = fixture_attempt(task.id, 1, "outbound_failed");
    attempt.error_type = Some("provider".to_string());
    attempt.error_message = Some("provider rejected request".to_string());

    repository
        .persist_outbound_terminal_failure_outcome(
            task.id,
            attempt.clone(),
            "provider rejected request",
        )
        .await
        .expect("terminal failure outcome should persist");

    let persisted_task = repository
        .task(task.id)
        .await
        .expect("query should succeed")
        .expect("task should exist");
    let persisted_attempts = repository
        .attempts_for_task(task.id)
        .await
        .expect("attempt query should succeed");

    assert_eq!(persisted_task.status, AnalysisTaskStatus::Failed);
    assert_eq!(persisted_task.attempt_count, 1);
    assert!(persisted_task.finished_at.is_some());
    assert_eq!(
        persisted_task.last_error_code.as_deref(),
        Some("terminal_error")
    );
    assert_eq!(
        persisted_task.last_error_message.as_deref(),
        Some("provider rejected request")
    );
    assert_eq!(persisted_attempts, vec![attempt]);

    cleanup_task_graph(&pool, task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_persist_outbound_dead_letter_writes_attempt_and_dead_letter() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_persist_outbound_dead_letter_writes_attempt_and_dead_letter: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgOrchestrationRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let (task, snapshot) = fixture_task_and_snapshot(instrument_id);
    repository
        .insert_task_with_snapshot(task.clone(), snapshot.clone())
        .await
        .expect("task insert should succeed");
    repository
        .claim_next_pending_task()
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    let mut attempt = fixture_attempt(task.id, 1, "outbound_failed");
    attempt.error_type = Some("provider".to_string());
    attempt.error_message = Some("max retries exceeded".to_string());
    let dead_letter = fixture_dead_letter(task.id, attempt.id, snapshot.input_json.clone());

    repository
        .persist_outbound_dead_letter_outcome(task.id, attempt.clone(), dead_letter.clone())
        .await
        .expect("dead letter outcome should persist");

    let persisted_task = repository
        .task(task.id)
        .await
        .expect("query should succeed")
        .expect("task should exist");
    let persisted_attempts = repository
        .attempts_for_task(task.id)
        .await
        .expect("attempt query should succeed");
    let persisted_dead_letter = repository
        .dead_letter_for_task(task.id)
        .await
        .expect("dead letter query should succeed");

    assert_eq!(persisted_task.status, AnalysisTaskStatus::DeadLetter);
    assert_eq!(persisted_task.attempt_count, 1);
    assert!(persisted_task.finished_at.is_some());
    assert_eq!(
        persisted_task.last_error_code.as_deref(),
        Some(dead_letter.final_error_type.as_str())
    );
    assert_eq!(
        persisted_task.last_error_message.as_deref(),
        Some(dead_letter.final_error_message.as_str())
    );
    assert_eq!(persisted_attempts, vec![attempt]);
    assert_eq!(persisted_dead_letter, Some(dead_letter));

    cleanup_task_graph(&pool, task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

async fn test_pool() -> Option<PgPool> {
    let database_url = test_database_url_from(|key| std::env::var(key).ok())?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("test database should connect");
    sqlx::migrate::Migrator::new(Path::new("../../migrations"))
        .await
        .expect("test migrator should load")
        .run(&pool)
        .await
        .expect("test migrations should apply");

    Some(pool)
}

fn test_database_url_from(get_env: impl Fn(&str) -> Option<String>) -> Option<String> {
    get_env("PA_TEST_DATABASE_URL")
}

async fn ensure_market_and_instrument(pool: &PgPool, market_id: Uuid, instrument_id: Uuid) {
    sqlx::query(
        r#"
        INSERT INTO markets (id, code, name, timezone)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(market_id)
    .bind(format!("MKT-{}", market_id.simple()))
    .bind("Test Market")
    .bind("UTC")
    .execute(pool)
    .await
    .expect("market seed should insert");

    sqlx::query(
        r#"
        INSERT INTO instruments (id, market_id, symbol, name, instrument_type)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(instrument_id)
    .bind(market_id)
    .bind(format!("SYM-{}", instrument_id.simple()))
    .bind("Test Instrument")
    .bind("crypto")
    .execute(pool)
    .await
    .expect("instrument seed should insert");
}

async fn cleanup_task_graph(pool: &PgPool, task_id: Uuid) {
    sqlx::query("DELETE FROM analysis_tasks WHERE id = $1")
        .bind(task_id)
        .execute(pool)
        .await
        .expect("task cleanup should succeed");
}

async fn cleanup_market_and_instrument(pool: &PgPool, market_id: Uuid, instrument_id: Uuid) {
    sqlx::query("DELETE FROM instruments WHERE id = $1")
        .bind(instrument_id)
        .execute(pool)
        .await
        .expect("instrument cleanup should succeed");
    sqlx::query("DELETE FROM markets WHERE id = $1")
        .bind(market_id)
        .execute(pool)
        .await
        .expect("market cleanup should succeed");
}

async fn seed_attempt(pool: &PgPool, task_id: Uuid) -> pa_orchestrator::AnalysisAttempt {
    let attempt = pa_orchestrator::AnalysisAttempt {
        id: Uuid::new_v4(),
        task_id,
        attempt_no: 1,
        worker_id: "worker-1".to_string(),
        llm_provider: "openai".to_string(),
        model: "gpt-test".to_string(),
        request_payload_json: serde_json::json!({
            "messages": [{"role": "user", "content": "analyze"}]
        }),
        raw_response_json: Some(serde_json::json!({
            "id": "resp_123",
            "output_text": "ok"
        })),
        parsed_output_json: Some(serde_json::json!({
            "sentiment": "bullish"
        })),
        status: "succeeded".to_string(),
        error_type: None,
        error_message: None,
        started_at: utc("2026-04-21T10:01:00Z"),
        finished_at: Some(utc("2026-04-21T10:01:04Z")),
    };

    sqlx::query(
        r#"
        INSERT INTO analysis_attempts (
            id,
            task_id,
            attempt_no,
            worker_id,
            llm_provider,
            model,
            request_payload_json,
            raw_response_json,
            parsed_output_json,
            status,
            error_type,
            error_message,
            started_at,
            finished_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(attempt.id)
    .bind(attempt.task_id)
    .bind(i32::try_from(attempt.attempt_no).expect("attempt_no should fit in i32"))
    .bind(&attempt.worker_id)
    .bind(&attempt.llm_provider)
    .bind(&attempt.model)
    .bind(&attempt.request_payload_json)
    .bind(&attempt.raw_response_json)
    .bind(&attempt.parsed_output_json)
    .bind(&attempt.status)
    .bind(&attempt.error_type)
    .bind(&attempt.error_message)
    .bind(attempt.started_at)
    .bind(attempt.finished_at)
    .execute(pool)
    .await
    .expect("attempt seed should insert");

    attempt
}

async fn seed_result(
    pool: &PgPool,
    task: &AnalysisTask,
) -> pa_orchestrator::AnalysisResult {
    let result = pa_orchestrator::AnalysisResult {
        id: Uuid::new_v4(),
        task_id: task.id,
        task_type: task.task_type.clone(),
        instrument_id: task.instrument_id,
        user_id: task.user_id,
        timeframe: task.timeframe,
        bar_state: task.bar_state,
        bar_open_time: task.bar_open_time,
        bar_close_time: task.bar_close_time,
        trading_date: task.trading_date,
        prompt_key: task.prompt_key.clone(),
        prompt_version: task.prompt_version.clone(),
        output_json: serde_json::json!({
            "summary": "analysis complete",
            "score": 0.82
        }),
        created_at: utc("2026-04-21T10:02:00Z"),
    };

    sqlx::query(
        r#"
        INSERT INTO analysis_results (
            id,
            task_id,
            task_type,
            instrument_id,
            user_id,
            timeframe,
            bar_state,
            bar_open_time,
            bar_close_time,
            trading_date,
            prompt_key,
            prompt_version,
            output_json,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(result.id)
    .bind(result.task_id)
    .bind(&result.task_type)
    .bind(result.instrument_id)
    .bind(result.user_id)
    .bind(result.timeframe.map(|value| value.to_string()))
    .bind(result.bar_state.as_str())
    .bind(result.bar_open_time)
    .bind(result.bar_close_time)
    .bind(result.trading_date)
    .bind(&result.prompt_key)
    .bind(&result.prompt_version)
    .bind(&result.output_json)
    .bind(result.created_at)
    .execute(pool)
    .await
    .expect("result seed should insert");

    result
}

async fn seed_dead_letter(
    pool: &PgPool,
    task_id: Uuid,
    last_attempt_id: Uuid,
) -> pa_orchestrator::AnalysisDeadLetter {
    let dead_letter = pa_orchestrator::AnalysisDeadLetter {
        id: Uuid::new_v4(),
        task_id,
        final_error_type: "schema_validation".to_string(),
        final_error_message: "model response did not match schema".to_string(),
        last_attempt_id: Some(last_attempt_id),
        archived_snapshot_json: serde_json::json!({
            "task_id": task_id,
            "reason": "schema_validation"
        }),
        created_at: utc("2026-04-21T10:03:00Z"),
    };

    sqlx::query(
        r#"
        INSERT INTO analysis_dead_letters (
            id,
            task_id,
            final_error_type,
            final_error_message,
            last_attempt_id,
            archived_snapshot_json,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(dead_letter.id)
    .bind(dead_letter.task_id)
    .bind(&dead_letter.final_error_type)
    .bind(&dead_letter.final_error_message)
    .bind(dead_letter.last_attempt_id)
    .bind(&dead_letter.archived_snapshot_json)
    .bind(dead_letter.created_at)
    .execute(pool)
    .await
    .expect("dead letter seed should insert");

    dead_letter
}

#[test]
fn test_pool_requires_explicit_test_database_url() {
    assert_eq!(test_database_url_from(|_| None), None);
    assert_eq!(
        test_database_url_from(|key| match key {
            "PA_TEST_DATABASE_URL" => Some("postgres://localhost/test_db".to_string()),
            _ => None,
        }),
        Some("postgres://localhost/test_db".to_string())
    );
}

fn fixture_task_and_snapshot(instrument_id: Uuid) -> (AnalysisTask, AnalysisSnapshot) {
    fixture_task_and_snapshot_with_dedupe(instrument_id, "pg:test:closed:shared:m15")
}

fn fixture_task_and_snapshot_with_dedupe(
    instrument_id: Uuid,
    dedupe_key: &str,
) -> (AnalysisTask, AnalysisSnapshot) {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();

    (
        AnalysisTask {
            id: task_id,
            task_type: "shared_bar_analysis".to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id,
            user_id: None,
            timeframe: Some(Timeframe::M15),
            bar_state: AnalysisBarState::Closed,
            bar_open_time: Some(utc("2026-04-21T01:45:00Z")),
            bar_close_time: Some(utc("2026-04-21T02:00:00Z")),
            trading_date: None,
            trigger_type: "event".to_string(),
            prompt_key: "shared_bar_analysis".to_string(),
            prompt_version: "v1".to_string(),
            snapshot_id,
            dedupe_key: Some(dedupe_key.to_string()),
            attempt_count: 0,
            max_attempts: 3,
            scheduled_at: utc("2026-04-21T10:00:00Z"),
            started_at: None,
            finished_at: None,
            last_error_code: None,
            last_error_message: None,
        },
        AnalysisSnapshot {
            id: snapshot_id,
            task_id,
            input_json: serde_json::json!({
                "instrument_id": instrument_id,
                "timeframe": "15m",
                "bar_state": "closed"
            }),
            input_hash: "abc123".to_string(),
            schema_version: "v1".to_string(),
            created_at: utc("2026-04-21T10:00:01Z"),
        },
    )
}

fn fixture_attempt(task_id: Uuid, attempt_no: u32, status: &str) -> AnalysisAttempt {
    AnalysisAttempt {
        id: Uuid::new_v4(),
        task_id,
        attempt_no,
        worker_id: "worker-1".to_string(),
        llm_provider: "openai".to_string(),
        model: "gpt-test".to_string(),
        request_payload_json: serde_json::json!({
            "messages": [{"role": "user", "content": "analyze"}]
        }),
        raw_response_json: Some(serde_json::json!({
            "id": "resp_123",
            "output_text": "ok"
        })),
        parsed_output_json: Some(serde_json::json!({
            "sentiment": "bullish"
        })),
        status: status.to_string(),
        error_type: None,
        error_message: None,
        started_at: utc("2026-04-21T10:01:00Z"),
        finished_at: Some(utc("2026-04-21T10:01:04Z")),
    }
}

fn fixture_result(task: &AnalysisTask, output_json: serde_json::Value) -> AnalysisResult {
    AnalysisResult {
        id: Uuid::new_v4(),
        task_id: task.id,
        task_type: task.task_type.clone(),
        instrument_id: task.instrument_id,
        user_id: task.user_id,
        timeframe: task.timeframe,
        bar_state: task.bar_state,
        bar_open_time: task.bar_open_time,
        bar_close_time: task.bar_close_time,
        trading_date: task.trading_date,
        prompt_key: task.prompt_key.clone(),
        prompt_version: task.prompt_version.clone(),
        output_json,
        created_at: utc("2026-04-21T10:02:00Z"),
    }
}

fn fixture_dead_letter(
    task_id: Uuid,
    last_attempt_id: Uuid,
    archived_snapshot_json: serde_json::Value,
) -> AnalysisDeadLetter {
    AnalysisDeadLetter {
        id: Uuid::new_v4(),
        task_id,
        final_error_type: "provider".to_string(),
        final_error_message: "max retries exceeded".to_string(),
        last_attempt_id: Some(last_attempt_id),
        archived_snapshot_json,
        created_at: utc("2026-04-21T10:03:00Z"),
    }
}

fn utc(value: &str) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &DateTime::parse_from_rfc3339(value)
            .expect("fixture timestamp should be valid")
            .naive_utc(),
    )
}
