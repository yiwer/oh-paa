use chrono::{DateTime, TimeZone, Utc};
use pa_core::Timeframe;
use pa_orchestrator::{
    AnalysisBarState, AnalysisSnapshot, AnalysisTask, AnalysisTaskStatus, InsertTaskResult,
    OrchestrationRepository, PgOrchestrationRepository,
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
            dedupe_key: Some("pg:test:closed:shared:m15".to_string()),
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

fn utc(value: &str) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &DateTime::parse_from_rfc3339(value)
            .expect("fixture timestamp should be valid")
            .naive_utc(),
    )
}
