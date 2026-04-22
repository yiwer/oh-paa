use chrono::{TimeZone, Utc};
use pa_analysis::{AnalysisRepository, InMemoryAnalysisRepository, PaStateBar};
use pa_core::Timeframe;
use pa_orchestrator::AnalysisBarState;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn inserts_one_pa_state_bar_per_identity_when_called_twice() {
    let repository = InMemoryAnalysisRepository::default();
    let pa_state_bar = PaStateBar {
        instrument_id: Uuid::nil(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Closed,
        bar_open_time: Utc.with_ymd_and_hms(2026, 4, 21, 1, 45, 0).unwrap(),
        bar_close_time: Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap(),
        analysis_version: "v1".to_string(),
        state_json: json!({
            "decision_tree_state": {
                "trend_context": "uptrend"
            }
        }),
    };

    let first = repository
        .insert_pa_state_bar_if_absent(pa_state_bar.clone())
        .await
        .unwrap();
    let second = repository
        .insert_pa_state_bar_if_absent(pa_state_bar.clone())
        .await
        .unwrap();

    assert!(first);
    assert!(!second);
    assert_eq!(repository.pa_state_bars().len(), 1);

    let stored = &repository.pa_state_bars()[0];
    assert_eq!(stored.instrument_id, pa_state_bar.instrument_id);
    assert_eq!(stored.timeframe, pa_state_bar.timeframe);
    assert_eq!(stored.bar_state, pa_state_bar.bar_state);
    assert_eq!(stored.bar_open_time, pa_state_bar.bar_open_time);
    assert_eq!(stored.bar_close_time, pa_state_bar.bar_close_time);
    assert_eq!(stored.analysis_version, "v1");
    assert_eq!(stored.state_json, pa_state_bar.state_json);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inserts_open_pa_state_bar_repeatedly_under_concurrent_requests() {
    let repository = Arc::new(InMemoryAnalysisRepository::default());
    let pa_state_bar = PaStateBar {
        instrument_id: Uuid::nil(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Open,
        bar_open_time: Utc.with_ymd_and_hms(2026, 4, 21, 1, 45, 0).unwrap(),
        bar_close_time: Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap(),
        analysis_version: "v1".to_string(),
        state_json: json!({
            "signal_assessment": {
                "status": "forming"
            }
        }),
    };

    let first_repository = Arc::clone(&repository);
    let first_pa_state_bar = pa_state_bar.clone();
    let first = tokio::spawn(async move {
        first_repository
            .insert_pa_state_bar_if_absent(first_pa_state_bar)
            .await
            .unwrap()
    });

    let second_repository = Arc::clone(&repository);
    let second_pa_state_bar = pa_state_bar.clone();
    let second = tokio::spawn(async move {
        second_repository
            .insert_pa_state_bar_if_absent(second_pa_state_bar)
            .await
            .unwrap()
    });

    let first = first.await.unwrap();
    let second = second.await.unwrap();

    assert!(first);
    assert!(second);
    assert_eq!(repository.pa_state_bars().len(), 2);

    for stored in repository.pa_state_bars() {
        assert_eq!(stored.instrument_id, pa_state_bar.instrument_id);
        assert_eq!(stored.timeframe, pa_state_bar.timeframe);
        assert_eq!(stored.bar_state, pa_state_bar.bar_state);
        assert_eq!(stored.bar_open_time, pa_state_bar.bar_open_time);
        assert_eq!(stored.bar_close_time, pa_state_bar.bar_close_time);
        assert_eq!(stored.analysis_version, pa_state_bar.analysis_version);
        assert_eq!(stored.state_json, pa_state_bar.state_json);
    }
}
