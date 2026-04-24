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
            "skipping pg_repository_inserts_task_snapshot_and_queries_task_views: PA_DATABASE_URL not set"
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

    cleanup_task_graph(&pool, task.id).await;
    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

async fn test_pool() -> Option<PgPool> {
    let database_url = std::env::var("PA_DATABASE_URL")
        .ok()
        .or_else(|| std::env::var("DATABASE_URL").ok())?;

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
