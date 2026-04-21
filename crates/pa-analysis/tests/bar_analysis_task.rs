use chrono::{TimeZone, Utc};
use pa_analysis::{
    bar_worker::BarAnalysisTask, repository::InMemoryAnalysisRepository, service::AnalysisService,
};
use pa_core::Timeframe;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn generates_one_bar_analysis_per_identity_when_called_twice() {
    let repository = InMemoryAnalysisRepository::default();
    let service = AnalysisService::new(&repository);
    let instrument_id = Uuid::nil();
    let task = BarAnalysisTask {
        instrument_id,
        timeframe: Timeframe::H1,
        bar_close_time: Utc.with_ymd_and_hms(2024, 1, 2, 10, 0, 0).unwrap(),
        analysis_version: "v1".to_string(),
        result_json: json!({
            "summary": "uptrend",
        }),
    };

    let first = service.generate_bar_analysis(task.clone()).await.unwrap();
    let second = service.generate_bar_analysis(task.clone()).await.unwrap();

    assert!(first.created);
    assert!(!second.created);
    assert_eq!(repository.bar_analyses().len(), 1);

    let stored = &repository.bar_analyses()[0];
    assert_eq!(stored.instrument_id, instrument_id);
    assert_eq!(stored.timeframe, Timeframe::H1);
    assert_eq!(stored.bar_close_time, task.bar_close_time);
    assert_eq!(stored.analysis_version, "v1");
    assert_eq!(stored.result_json, task.result_json);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generates_one_bar_analysis_per_identity_under_concurrent_requests() {
    let repository = Arc::new(InMemoryAnalysisRepository::default());
    let instrument_id = Uuid::nil();
    let task = BarAnalysisTask {
        instrument_id,
        timeframe: Timeframe::H1,
        bar_close_time: Utc.with_ymd_and_hms(2024, 1, 2, 10, 0, 0).unwrap(),
        analysis_version: "v1".to_string(),
        result_json: json!({
            "summary": "uptrend",
        }),
    };

    let first_repository = Arc::clone(&repository);
    let first_task = task.clone();
    let first = tokio::spawn(async move {
        let service = AnalysisService::new(first_repository.as_ref());
        service.generate_bar_analysis(first_task).await.unwrap()
    });

    let second_repository = Arc::clone(&repository);
    let second_task = task.clone();
    let second = tokio::spawn(async move {
        let service = AnalysisService::new(second_repository.as_ref());
        service.generate_bar_analysis(second_task).await.unwrap()
    });

    let first = first.await.unwrap();
    let second = second.await.unwrap();

    assert_ne!(first.created, second.created);
    assert_eq!(repository.bar_analyses().len(), 1);

    let stored = &repository.bar_analyses()[0];
    assert_eq!(stored.result_json, task.result_json);
}
