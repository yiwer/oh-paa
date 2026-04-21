use chrono::NaiveDate;
use pa_analysis::{
    daily_context_worker::DailyContextTask, repository::InMemoryAnalysisRepository,
    service::AnalysisService,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn generates_one_daily_context_per_identity_when_called_twice() {
    let repository = InMemoryAnalysisRepository::default();
    let service = AnalysisService::new(&repository);
    let instrument_id = Uuid::nil();
    let task = DailyContextTask {
        instrument_id,
        trading_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
        analysis_version: "v1".to_string(),
        context_json: json!({
            "session_bias": "neutral",
        }),
    };

    let first = service.generate_daily_context(task.clone()).await.unwrap();
    let second = service.generate_daily_context(task.clone()).await.unwrap();

    assert!(first.created);
    assert!(!second.created);
    assert_eq!(repository.daily_contexts().len(), 1);

    let stored = &repository.daily_contexts()[0];
    assert_eq!(stored.instrument_id, instrument_id);
    assert_eq!(stored.trading_date, task.trading_date);
    assert_eq!(stored.analysis_version, "v1");
    assert_eq!(stored.context_json, task.context_json);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generates_one_daily_context_per_identity_under_concurrent_requests() {
    let repository = Arc::new(InMemoryAnalysisRepository::default());
    let instrument_id = Uuid::nil();
    let task = DailyContextTask {
        instrument_id,
        trading_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
        analysis_version: "v1".to_string(),
        context_json: json!({
            "session_bias": "neutral",
        }),
    };

    let first_repository = Arc::clone(&repository);
    let first_task = task.clone();
    let first = tokio::spawn(async move {
        let service = AnalysisService::new(first_repository.as_ref());
        service.generate_daily_context(first_task).await.unwrap()
    });

    let second_repository = Arc::clone(&repository);
    let second_task = task.clone();
    let second = tokio::spawn(async move {
        let service = AnalysisService::new(second_repository.as_ref());
        service.generate_daily_context(second_task).await.unwrap()
    });

    let first = first.await.unwrap();
    let second = second.await.unwrap();

    assert_ne!(first.created, second.created);
    assert_eq!(repository.daily_contexts().len(), 1);

    let stored = &repository.daily_contexts()[0];
    assert_eq!(stored.context_json, task.context_json);
}
