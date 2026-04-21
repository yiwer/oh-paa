use chrono::{NaiveDate, TimeZone, Utc};
use pa_analysis::{BarAnalysis, DailyMarketContext};
use pa_core::{AppError, Timeframe};
use pa_user::{
    models::{
        ManualUserAnalysisRequest, PositionSide, PositionSnapshot, UserAnalysisReport,
        UserSubscription,
    },
    repository::{InMemorySharedAnalysisLookup, InMemoryUserRepository},
    service::UserAnalysisService,
};
use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn manual_user_analysis_includes_shared_outputs_and_user_position_context() {
    let user_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    let user_repository = InMemoryUserRepository::new(
        vec![UserSubscription {
            user_id,
            instrument_id,
            enabled: true,
        }],
        vec![PositionSnapshot {
            user_id,
            instrument_id,
            side: PositionSide::Long,
            quantity: Decimal::new(15, 1),
            average_cost: Decimal::new(43125, 2),
        }],
    );

    let shared_analysis = InMemorySharedAnalysisLookup::new(
        vec![BarAnalysis {
            instrument_id,
            timeframe: Timeframe::H1,
            bar_close_time: Utc.with_ymd_and_hms(2024, 1, 2, 10, 0, 0).unwrap(),
            analysis_version: "v1".to_string(),
            result_json: json!({
                "summary": "bullish continuation",
                "confidence": 0.74,
            }),
        }],
        vec![DailyMarketContext {
            instrument_id,
            trading_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
            analysis_version: "v1".to_string(),
            context_json: json!({
                "session_bias": "risk_on",
                "macro_theme": "usd_softness",
            }),
        }],
    );

    let service = UserAnalysisService::new(&user_repository, &shared_analysis);
    let request = ManualUserAnalysisRequest {
        user_id,
        instrument_id,
        timeframe: Timeframe::H1,
        bar_close_time: Utc.with_ymd_and_hms(2024, 1, 2, 10, 0, 0).unwrap(),
        trading_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
        analysis_version: "v1".to_string(),
    };

    let report = service.run_manual_analysis(request).await.unwrap();

    assert_eq!(
        report,
        UserAnalysisReport {
            user_id,
            instrument_id,
            subscriptions: vec![UserSubscription {
                user_id,
                instrument_id,
                enabled: true,
            }],
            positions: vec![PositionSnapshot {
                user_id,
                instrument_id,
                side: PositionSide::Long,
                quantity: Decimal::new(15, 1),
                average_cost: Decimal::new(43125, 2),
            }],
            bar_analysis: json!({
                "summary": "bullish continuation",
                "confidence": 0.74,
            }),
            daily_market_context: json!({
                "session_bias": "risk_on",
                "macro_theme": "usd_softness",
            }),
        }
    );
    assert_eq!(
        report.analysis_payload(),
        json!({
            "user_id": user_id,
            "instrument_id": instrument_id,
            "subscriptions": [{
                "enabled": true,
                "instrument_id": instrument_id,
                "user_id": user_id,
            }],
            "positions": [{
                "average_cost": "431.25",
                "instrument_id": instrument_id,
                "quantity": "1.5",
                "side": "long",
                "user_id": user_id,
            }],
            "bar_analysis": {
                "summary": "bullish continuation",
                "confidence": 0.74,
            },
            "daily_market_context": {
                "session_bias": "risk_on",
                "macro_theme": "usd_softness",
            }
        })
    );
}

#[tokio::test]
async fn manual_user_analysis_returns_identifying_error_when_bar_analysis_is_missing() {
    let user_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();
    let request = ManualUserAnalysisRequest {
        user_id,
        instrument_id,
        timeframe: Timeframe::H1,
        bar_close_time: Utc.with_ymd_and_hms(2024, 1, 2, 10, 0, 0).unwrap(),
        trading_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
        analysis_version: "v1".to_string(),
    };
    let user_repository = InMemoryUserRepository::new(
        vec![UserSubscription {
            user_id,
            instrument_id,
            enabled: true,
        }],
        vec![PositionSnapshot {
            user_id,
            instrument_id,
            side: PositionSide::Long,
            quantity: Decimal::new(15, 1),
            average_cost: Decimal::new(43125, 2),
        }],
    );
    let shared_analysis = InMemorySharedAnalysisLookup::new(
        Vec::new(),
        vec![DailyMarketContext {
            instrument_id,
            trading_date: request.trading_date,
            analysis_version: request.analysis_version.clone(),
            context_json: json!({
                "session_bias": "risk_on",
            }),
        }],
    );

    let error = UserAnalysisService::new(&user_repository, &shared_analysis)
        .run_manual_analysis(request.clone())
        .await
        .unwrap_err();

    match error {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("missing shared bar analysis"));
            assert!(message.contains(&request.instrument_id.to_string()));
            assert!(message.contains(request.timeframe.as_str()));
            assert!(message.contains(&request.bar_close_time.to_rfc3339()));
            assert!(message.contains(&request.analysis_version));
        }
        other => panic!("expected analysis error, got {other:?}"),
    }
}

#[tokio::test]
async fn manual_user_analysis_returns_identifying_error_when_daily_context_is_missing() {
    let user_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();
    let request = ManualUserAnalysisRequest {
        user_id,
        instrument_id,
        timeframe: Timeframe::H1,
        bar_close_time: Utc.with_ymd_and_hms(2024, 1, 2, 10, 0, 0).unwrap(),
        trading_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
        analysis_version: "v1".to_string(),
    };
    let user_repository = InMemoryUserRepository::new(
        vec![UserSubscription {
            user_id,
            instrument_id,
            enabled: true,
        }],
        vec![PositionSnapshot {
            user_id,
            instrument_id,
            side: PositionSide::Long,
            quantity: Decimal::new(15, 1),
            average_cost: Decimal::new(43125, 2),
        }],
    );
    let shared_analysis = InMemorySharedAnalysisLookup::new(
        vec![BarAnalysis {
            instrument_id,
            timeframe: request.timeframe,
            bar_close_time: request.bar_close_time,
            analysis_version: request.analysis_version.clone(),
            result_json: json!({
                "summary": "bullish continuation",
            }),
        }],
        Vec::new(),
    );

    let error = UserAnalysisService::new(&user_repository, &shared_analysis)
        .run_manual_analysis(request.clone())
        .await
        .unwrap_err();

    match error {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("missing shared daily market context"));
            assert!(message.contains(&request.instrument_id.to_string()));
            assert!(message.contains(request.timeframe.as_str()));
            assert!(message.contains(&request.trading_date.to_string()));
            assert!(message.contains(&request.analysis_version));
        }
        other => panic!("expected analysis error, got {other:?}"),
    }
}
