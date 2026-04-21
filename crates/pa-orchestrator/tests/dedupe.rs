use chrono::{TimeZone, Utc};
use pa_core::Timeframe;
use pa_orchestrator::{
    AnalysisBarState, AnalysisSnapshot, AnalysisTask, AnalysisTaskStatus,
    InMemoryOrchestrationRepository, InsertTaskResult, OrchestrationRepository,
    build_shared_bar_dedupe_key,
};
use uuid::Uuid;

#[test]
fn closed_bar_dedupe_key_exists_and_open_bar_dedupe_key_does_not() {
    let instrument_id = Uuid::new_v4();
    let bar_close_time = Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap();

    let closed_key = build_shared_bar_dedupe_key(
        instrument_id,
        Timeframe::M15,
        bar_close_time,
        "shared_bar_analysis",
        "v1",
        AnalysisBarState::Closed,
    );

    let open_key = build_shared_bar_dedupe_key(
        instrument_id,
        Timeframe::M15,
        bar_close_time,
        "shared_bar_analysis",
        "v1",
        AnalysisBarState::Open,
    );

    assert!(closed_key.is_some());
    assert_eq!(open_key, None);
}

#[tokio::test]
async fn closed_bar_duplicate_task_is_suppressed_in_memory() {
    let repository = InMemoryOrchestrationRepository::default();
    let dedupe_key = Some("closed:shared:m15".to_string());
    let first = make_task_and_snapshot(dedupe_key.clone(), AnalysisBarState::Closed);
    let second = make_task_and_snapshot(dedupe_key, AnalysisBarState::Closed);

    let first_insert = repository
        .insert_task_with_snapshot(first.0.clone(), first.1.clone())
        .await
        .unwrap();
    assert_eq!(first_insert, InsertTaskResult::Inserted);

    let second_insert = repository
        .insert_task_with_snapshot(second.0, second.1)
        .await
        .unwrap();
    assert_eq!(
        second_insert,
        InsertTaskResult::DuplicateExistingTask(first.0.id)
    );

    let fetched = repository.fetch_next_pending_task().await.unwrap();
    assert_eq!(fetched, Some(first.0.clone()));
    assert_eq!(repository.fetch_next_pending_task().await.unwrap(), None);
}

#[tokio::test]
async fn open_bar_task_allows_repeated_insertions_in_memory() {
    let repository = InMemoryOrchestrationRepository::default();
    let first = make_task_and_snapshot(None, AnalysisBarState::Open);
    let second = make_task_and_snapshot(None, AnalysisBarState::Open);
    assert_eq!(first.0.bar_state, AnalysisBarState::Open);
    assert_eq!(second.0.bar_state, AnalysisBarState::Open);

    let first_insert = repository
        .insert_task_with_snapshot(first.0.clone(), first.1)
        .await
        .unwrap();
    let second_insert = repository
        .insert_task_with_snapshot(second.0.clone(), second.1)
        .await
        .unwrap();

    assert_eq!(first_insert, InsertTaskResult::Inserted);
    assert_eq!(second_insert, InsertTaskResult::Inserted);

    assert_eq!(
        repository.fetch_next_pending_task().await.unwrap(),
        Some(first.0)
    );
    assert_eq!(
        repository.fetch_next_pending_task().await.unwrap(),
        Some(second.0)
    );
    assert_eq!(repository.fetch_next_pending_task().await.unwrap(), None);
}

fn make_task_and_snapshot(
    dedupe_key: Option<String>,
    bar_state: AnalysisBarState,
) -> (AnalysisTask, AnalysisSnapshot) {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap();

    (
        AnalysisTask {
            id: task_id,
            task_type: "shared_bar_analysis".to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id: Uuid::new_v4(),
            user_id: None,
            timeframe: Some(Timeframe::M15),
            bar_state,
            bar_open_time: None,
            bar_close_time: Some(Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap()),
            trading_date: None,
            trigger_type: "event".to_string(),
            prompt_key: "shared_bar_analysis".to_string(),
            prompt_version: "v1".to_string(),
            snapshot_id,
            dedupe_key,
            attempt_count: 0,
            max_attempts: 3,
            scheduled_at,
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
        },
    )
}
