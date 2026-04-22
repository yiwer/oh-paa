use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use chrono::{DateTime, Utc};
use pa_api::{AppState, MarketRuntime, app_router};
use pa_core::{AppError, Timeframe};
use pa_instrument::InstrumentRepository;
use pa_market::{
    CanonicalKlineRepository, MarketDataProvider, PgCanonicalKlineRepository, ProviderKline,
    ProviderRouter, ProviderTick,
};
use pa_orchestrator::{AnalysisResult, InMemoryOrchestrationRepository, OrchestrationRepository};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn healthz_and_phase2_analysis_routes_are_wired() {
    let app = app_router(AppState::fixture());

    let healthz = request(&app, "/healthz").await;
    assert_eq!(healthz.status(), StatusCode::OK);
    assert_eq!(response_text(healthz).await, "ok");

    let shared_bar = request_json(
        &app,
        Method::POST,
        "/analysis/shared/bar",
        r#"{
            "instrument_id":"00000000-0000-0000-0000-000000000001",
            "timeframe":"15m",
            "bar_state":"closed",
            "bar_open_time":"2026-04-21T01:45:00Z",
            "bar_close_time":"2026-04-21T02:00:00Z",
            "shared_pa_state_json":{"bar_identity":{"tag":"fixture-pa-state"}},
            "recent_pa_states_json":[]
        }"#,
    )
    .await;
    assert_eq!(shared_bar.status(), StatusCode::ACCEPTED);
    let shared_bar_json = response_json(shared_bar).await;
    let task_id = shared_bar_json["task_id"]
        .as_str()
        .expect("shared bar task_id should exist")
        .to_string();
    assert_eq!(shared_bar_json["status"], "pending");
    assert_eq!(shared_bar_json["dedupe_hit"], false);

    let task = request(&app, &format!("/analysis/tasks/{task_id}")).await;
    assert_eq!(task.status(), StatusCode::OK);
    let task_json = response_json(task).await;
    assert_eq!(task_json["task_id"], task_id);
    assert_eq!(task_json["task_type"], "shared_bar_analysis");

    let manual_user = request_json(
        &app,
        Method::POST,
        "/user/analysis/manual",
        r#"{
            "user_id":"00000000-0000-0000-0000-000000000101",
            "instrument_id":"00000000-0000-0000-0000-000000000001",
            "timeframe":"15m",
            "bar_state":"closed",
            "bar_open_time":"2026-04-21T01:45:00Z",
            "bar_close_time":"2026-04-21T02:00:00Z",
            "trading_date":"2026-04-21",
            "positions_json":[],
            "subscriptions_json":[],
            "shared_bar_analysis_json":{"bullish_case":{},"bearish_case":{}},
            "shared_daily_context_json":{"decision_tree_nodes":{}}
        }"#,
    )
    .await;
    assert_eq!(manual_user.status(), StatusCode::ACCEPTED);
    let manual_user_json = response_json(manual_user).await;
    assert_eq!(manual_user_json["status"], "pending");
    assert_eq!(manual_user_json["dedupe_hit"], false);
}

#[tokio::test]
async fn admin_backfill_and_market_reads_flow_through_runtime() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping admin_backfill_and_market_reads_flow_through_runtime: PA_DATABASE_URL not set"
        );
        return;
    };
    let fixture = seed_runtime_fixture(&pool).await;
    let app = market_runtime_app(pool.clone());

    let backfill = request_json(
        &app,
        Method::POST,
        "/admin/market/backfill",
        &format!(
            r#"{{
                "instrument_id":"{}",
                "timeframe":"15m",
                "limit":4
            }}"#,
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(backfill.status(), StatusCode::ACCEPTED);
    let backfill_json = response_json(backfill).await;
    assert_eq!(backfill_json["primary_provider"], "primary");
    assert_eq!(backfill_json["fallback_provider"], "fallback");
    assert_eq!(backfill_json["fallback_provider_symbol"], "BBB");

    let canonical = request(
        &app,
        &format!(
            "/market/canonical?instrument_id={}&timeframe=15m&limit=10",
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(canonical.status(), StatusCode::OK);
    let canonical_json = response_json(canonical).await;
    let canonical_rows = canonical_json["rows"]
        .as_array()
        .expect("canonical rows should be an array");
    assert_eq!(canonical_rows.len(), 4);
    assert_eq!(canonical_rows[0]["source_provider"], "fallback");

    let aggregated = request(
        &app,
        &format!(
            "/market/aggregated?instrument_id={}&source_timeframe=15m&target_timeframe=1h&limit=10",
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(aggregated.status(), StatusCode::OK);
    let aggregated_json = response_json(aggregated).await;
    let aggregated_rows = aggregated_json["rows"]
        .as_array()
        .expect("aggregated rows should be an array");
    assert_eq!(aggregated_rows.len(), 1);
    assert_eq!(aggregated_rows[0]["complete"], true);
    assert_eq!(aggregated_rows[0]["child_bar_count"], 4);
    assert_eq!(aggregated_rows[0]["expected_child_bar_count"], 4);
    assert_eq!(aggregated_rows[0]["source_provider"], "fallback");

    let session_profile = request(
        &app,
        &format!(
            "/market/session-profile?instrument_id={}",
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(session_profile.status(), StatusCode::OK);
    let session_profile_json = response_json(session_profile).await;
    assert_eq!(session_profile_json["session_kind"], "continuous_utc");
    assert_eq!(session_profile_json["market_code"], "crypto");

    let tick = request(
        &app,
        &format!("/market/tick?instrument_id={}", fixture.instrument_id),
    )
    .await;
    assert_eq!(tick.status(), StatusCode::OK);
    let tick_json = response_json(tick).await;
    assert_eq!(tick_json["provider"], "fallback");
    assert_eq!(tick_json["market_open"], true);
    assert_eq!(tick_json["tick"]["price"], "11.0");

    let open_bar = request(
        &app,
        &format!(
            "/market/open-bar?instrument_id={}&timeframe=1h",
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(open_bar.status(), StatusCode::OK);
    let open_bar_json = response_json(open_bar).await;
    assert_eq!(open_bar_json["market_open"], true);
    assert_eq!(
        open_bar_json["row"]["open_time"],
        "2024-01-02T10:00:00+00:00"
    );
    assert_eq!(
        open_bar_json["row"]["close_time"],
        "2024-01-02T11:00:00+00:00"
    );
    assert_eq!(open_bar_json["row"]["open"], "10.8");
    assert_eq!(open_bar_json["row"]["close"], "11.0");
    assert_eq!(open_bar_json["row"]["child_bar_count"], 0);

    cleanup_runtime_fixture(&pool, &fixture).await;
}

#[tokio::test]
async fn analysis_routes_can_assemble_inputs_from_market_runtime_and_shared_results() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping analysis_routes_can_assemble_inputs_from_market_runtime_and_shared_results: PA_DATABASE_URL not set"
        );
        return;
    };
    let fixture = seed_runtime_fixture(&pool).await;
    let orchestration_repository = Arc::new(InMemoryOrchestrationRepository::default());
    let app =
        market_runtime_app_with_repository(pool.clone(), Arc::clone(&orchestration_repository));

    let backfill = request_json(
        &app,
        Method::POST,
        "/admin/market/backfill",
        &format!(
            r#"{{
                "instrument_id":"{}",
                "timeframe":"15m",
                "limit":4
            }}"#,
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(backfill.status(), StatusCode::ACCEPTED);

    let shared_pa_state = request_json(
        &app,
        Method::POST,
        "/analysis/shared/pa-state",
        &format!(
            r#"{{
                "instrument_id":"{}",
                "timeframe":"1h",
                "bar_state":"open"
            }}"#,
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(shared_pa_state.status(), StatusCode::ACCEPTED);
    let shared_pa_state_json = response_json(shared_pa_state).await;
    let shared_pa_state_task_id = shared_pa_state_json["task_id"]
        .as_str()
        .expect("shared pa state task id should exist")
        .parse::<Uuid>()
        .expect("shared pa state task id should parse");
    let shared_pa_state_task = orchestration_repository
        .task(shared_pa_state_task_id)
        .expect("shared pa state task should exist");
    let shared_pa_state_snapshot = orchestration_repository
        .load_snapshot(shared_pa_state_task.snapshot_id)
        .await
        .expect("shared pa state snapshot should load");
    assert_eq!(shared_pa_state_snapshot.input_json["timeframe"], "1h");
    assert_eq!(shared_pa_state_snapshot.input_json["bar_state"], "open");
    assert_eq!(
        shared_pa_state_snapshot.input_json["bar_open_time"],
        "2024-01-02T10:00:00+00:00"
    );
    assert_eq!(
        shared_pa_state_snapshot.input_json["bar_close_time"],
        "2024-01-02T11:00:00+00:00"
    );
    assert_eq!(
        shared_pa_state_snapshot.input_json["bar_json"]["close"],
        "11.0"
    );
    assert_eq!(
        shared_pa_state_snapshot.input_json["market_context_json"]["market"]["market_code"],
        "crypto"
    );

    orchestration_repository
        .insert_result_and_complete(AnalysisResult::from_task(
            &shared_pa_state_task,
            serde_json::json!({
                "bar_identity": {"tag": "runtime-open-pa-state"},
                "market_session_context": {},
                "bar_observation": {},
                "bar_shape": {},
                "location_context": {},
                "multi_timeframe_alignment": {},
                "support_resistance_map": {},
                "signal_assessment": {},
                "decision_tree_state": {
                    "trend_context": {},
                    "location_context": {},
                    "signal_quality": {},
                    "confirmation_state": {},
                    "invalidation_conditions": {},
                    "bias_balance": {}
                },
                "evidence_log": {}
            }),
        ))
        .await
        .expect("shared pa state result should persist");

    let shared_bar = request_json(
        &app,
        Method::POST,
        "/analysis/shared/bar",
        &format!(
            r#"{{
                "instrument_id":"{}",
                "timeframe":"1h",
                "bar_state":"open"
            }}"#,
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(shared_bar.status(), StatusCode::ACCEPTED);
    let shared_bar_json = response_json(shared_bar).await;
    let shared_bar_task_id = shared_bar_json["task_id"]
        .as_str()
        .expect("shared bar task id should exist")
        .parse::<Uuid>()
        .expect("shared bar task id should parse");
    let shared_bar_task = orchestration_repository
        .task(shared_bar_task_id)
        .expect("shared bar task should exist");
    let shared_bar_snapshot = orchestration_repository
        .load_snapshot(shared_bar_task.snapshot_id)
        .await
        .expect("shared bar snapshot should load");
    assert_eq!(shared_bar_snapshot.input_json["timeframe"], "1h");
    assert_eq!(shared_bar_snapshot.input_json["bar_state"], "open");
    assert_eq!(
        shared_bar_snapshot.input_json["bar_open_time"],
        "2024-01-02T10:00:00+00:00"
    );
    assert_eq!(
        shared_bar_snapshot.input_json["bar_close_time"],
        "2024-01-02T11:00:00+00:00"
    );
    assert_eq!(
        shared_bar_snapshot.input_json["shared_pa_state_json"]["bar_identity"]["tag"],
        "runtime-open-pa-state"
    );
    assert_eq!(
        shared_bar_snapshot.input_json["recent_pa_states_json"][0]["bar_identity"]["tag"],
        "runtime-open-pa-state"
    );

    orchestration_repository
        .insert_result_and_complete(AnalysisResult::from_task(
            &shared_bar_task,
            serde_json::json!({
                "bar_identity": {"tag": "runtime-open-bar"},
                "bar_summary": {"tag": "runtime-open-bar"},
                "market_story": {},
                "bullish_case": {"summary": "test"},
                "bearish_case": {"summary": "test"},
                "two_sided_balance": {},
                "key_levels": {},
                "signal_bar_verdict": {},
                "continuation_path": {},
                "reversal_path": {},
                "invalidation_map": {},
                "follow_through_checkpoints": {}
            }),
        ))
        .await
        .expect("shared bar result should persist");

    let shared_daily = request_json(
        &app,
        Method::POST,
        "/analysis/shared/daily",
        &format!(
            r#"{{
                "instrument_id":"{}"
            }}"#,
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(shared_daily.status(), StatusCode::ACCEPTED);
    let shared_daily_json = response_json(shared_daily).await;
    let shared_daily_task_id = shared_daily_json["task_id"]
        .as_str()
        .expect("shared daily task id should exist")
        .parse::<Uuid>()
        .expect("shared daily task id should parse");
    let shared_daily_task = orchestration_repository
        .task(shared_daily_task_id)
        .expect("shared daily task should exist");
    let shared_daily_snapshot = orchestration_repository
        .load_snapshot(shared_daily_task.snapshot_id)
        .await
        .expect("shared daily snapshot should load");
    assert_eq!(
        shared_daily_snapshot.input_json["recent_shared_bar_analyses_json"][0]["bar_summary"]["tag"],
        "runtime-open-bar"
    );
    assert_eq!(
        shared_daily_snapshot.input_json["recent_pa_states_json"][0]["bar_identity"]["tag"],
        "runtime-open-pa-state"
    );
    assert_eq!(
        shared_daily_snapshot.input_json["market_background_json"]["market"]["market_code"],
        "crypto"
    );
    assert_eq!(
        shared_daily_snapshot.input_json["multi_timeframe_structure_json"]["15m"]["timeframe"],
        "15m"
    );
    assert_eq!(
        shared_daily_snapshot.input_json["multi_timeframe_structure_json"]["1h"]["timeframe"],
        "1h"
    );
    assert_eq!(
        shared_daily_snapshot.input_json["multi_timeframe_structure_json"]["1d"]["timeframe"],
        "1d"
    );

    orchestration_repository
        .insert_result_and_complete(AnalysisResult::from_task(
            &shared_daily_task,
            serde_json::json!({
                "context_identity": {},
                "market_background": {"summary": "runtime daily"},
                "dominant_structure": {},
                "intraday_vs_higher_timeframe_state": {},
                "key_support_levels": {},
                "key_resistance_levels": {},
                "signal_bars": {},
                "candle_pattern_map": {},
                "decision_tree_nodes": {
                    "trend_context": {},
                    "location_context": {},
                    "signal_quality": {},
                    "confirmation_state": {},
                    "invalidation_conditions": {},
                    "path_of_least_resistance": {}
                },
                "liquidity_context": {},
                "scenario_map": {},
                "risk_notes": {},
                "session_playbook": {}
            }),
        ))
        .await
        .expect("shared daily result should persist");

    let manual_user = request_json(
        &app,
        Method::POST,
        "/user/analysis/manual",
        &format!(
            r#"{{
                "user_id":"{}",
                "instrument_id":"{}",
                "timeframe":"1h",
                "bar_state":"open",
                "positions_json":[],
                "subscriptions_json":[]
            }}"#,
            Uuid::new_v4(),
            fixture.instrument_id
        ),
    )
    .await;
    assert_eq!(manual_user.status(), StatusCode::ACCEPTED);
    let manual_user_json = response_json(manual_user).await;
    let manual_user_task_id = manual_user_json["task_id"]
        .as_str()
        .expect("manual user task id should exist")
        .parse::<Uuid>()
        .expect("manual user task id should parse");
    let manual_user_task = orchestration_repository
        .task(manual_user_task_id)
        .expect("manual user task should exist");
    let manual_user_snapshot = orchestration_repository
        .load_snapshot(manual_user_task.snapshot_id)
        .await
        .expect("manual user snapshot should load");
    assert_eq!(manual_user_snapshot.input_json["timeframe"], "1h");
    assert_eq!(manual_user_snapshot.input_json["bar_state"], "open");
    assert_eq!(
        manual_user_snapshot.input_json["bar_open_time"],
        "2024-01-02T10:00:00+00:00"
    );
    assert_eq!(
        manual_user_snapshot.input_json["shared_bar_analysis_json"]["bar_summary"]["tag"],
        "runtime-open-bar"
    );
    assert_eq!(
        manual_user_snapshot.input_json["shared_daily_context_json"]["market_background"]["summary"],
        "runtime daily"
    );

    cleanup_runtime_fixture(&pool, &fixture).await;
}

fn market_runtime_app(pool: PgPool) -> axum::Router {
    market_runtime_app_with_repository(pool, Arc::new(InMemoryOrchestrationRepository::default()))
}

fn market_runtime_app_with_repository(
    pool: PgPool,
    orchestration_repository: Arc<InMemoryOrchestrationRepository>,
) -> axum::Router {
    let instrument_repository = InstrumentRepository::new(pool.clone());
    let canonical_repository: Arc<dyn CanonicalKlineRepository> =
        Arc::new(PgCanonicalKlineRepository::new(pool));
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(FailingProvider));
    provider_router.insert(Arc::new(FallbackProvider));
    let runtime = Arc::new(MarketRuntime::new(
        instrument_repository,
        canonical_repository,
        Arc::new(provider_router),
    ));
    let state = AppState::with_dependencies("127.0.0.1:0", orchestration_repository, Some(runtime));

    app_router(state)
}

async fn request(app: &axum::Router, uri: &str) -> axum::response::Response {
    request_with_body(app, Method::GET, uri, Body::empty()).await
}

async fn request_json(
    app: &axum::Router,
    method: Method,
    uri: &str,
    body: &str,
) -> axum::response::Response {
    request_with_body(app, method, uri, Body::from(body.to_string())).await
}

async fn request_with_body(
    app: &axum::Router,
    method: Method,
    uri: &str,
    body: Body,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(body)
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
}

async fn response_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");

    String::from_utf8(bytes.to_vec()).expect("body should be utf-8")
}

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_str(&response_text(response).await).expect("body should be valid json")
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

struct RuntimeFixture {
    market_id: Uuid,
    instrument_id: Uuid,
}

async fn seed_runtime_fixture(pool: &PgPool) -> RuntimeFixture {
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

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
    .bind("Runtime Instrument")
    .bind("crypto")
    .execute(pool)
    .await
    .expect("instrument seed should insert");

    for (provider, provider_symbol) in [("primary", "AAA"), ("fallback", "BBB")] {
        sqlx::query(
            r#"
            INSERT INTO instrument_symbol_bindings (id, instrument_id, provider, provider_symbol)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(instrument_id)
        .bind(provider)
        .bind(provider_symbol)
        .execute(pool)
        .await
        .expect("binding seed should insert");
    }

    sqlx::query(
        r#"
        INSERT INTO provider_policies (
            id, scope_type, market_id, instrument_id, kline_primary, kline_fallback, tick_primary, tick_fallback
        ) VALUES ($1, 'market', $2, NULL, $3, $4, $5, $6)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(market_id)
    .bind("primary")
    .bind(Some("fallback"))
    .bind("primary")
    .bind(Some("fallback"))
    .execute(pool)
    .await
    .expect("policy seed should insert");

    RuntimeFixture {
        market_id,
        instrument_id,
    }
}

async fn cleanup_runtime_fixture(pool: &PgPool, fixture: &RuntimeFixture) {
    sqlx::query("DELETE FROM provider_policies WHERE market_id = $1")
        .bind(fixture.market_id)
        .execute(pool)
        .await
        .expect("policy cleanup should succeed");
    sqlx::query("DELETE FROM instrument_symbol_bindings WHERE instrument_id = $1")
        .bind(fixture.instrument_id)
        .execute(pool)
        .await
        .expect("binding cleanup should succeed");
    sqlx::query("DELETE FROM instruments WHERE id = $1")
        .bind(fixture.instrument_id)
        .execute(pool)
        .await
        .expect("instrument cleanup should succeed");
    sqlx::query("DELETE FROM markets WHERE id = $1")
        .bind(fixture.market_id)
        .execute(pool)
        .await
        .expect("market cleanup should succeed");
}

struct FailingProvider;

#[async_trait]
impl MarketDataProvider for FailingProvider {
    fn name(&self) -> &'static str {
        "primary"
    }

    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        Err(AppError::Provider {
            message: "primary provider is intentionally failing in smoke test".into(),
            source: None,
        })
    }

    async fn fetch_latest_tick(&self, _provider_symbol: &str) -> Result<ProviderTick, AppError> {
        Err(AppError::Provider {
            message: "primary provider is intentionally failing in smoke test".into(),
            source: None,
        })
    }

    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

struct FallbackProvider;

#[async_trait]
impl MarketDataProvider for FallbackProvider {
    fn name(&self) -> &'static str {
        "fallback"
    }

    async fn fetch_klines(
        &self,
        provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        if provider_symbol != "BBB" {
            return Err(AppError::Validation {
                message: format!("unexpected fallback provider symbol: {provider_symbol}"),
                source: None,
            });
        }

        Ok(vec![
            bar("2024-01-02T09:00:00Z", "10.0", "10.2", "9.9", "10.1"),
            bar("2024-01-02T09:15:00Z", "10.1", "10.4", "10.0", "10.3"),
            bar("2024-01-02T09:30:00Z", "10.3", "10.6", "10.2", "10.5"),
            bar("2024-01-02T09:45:00Z", "10.5", "10.9", "10.4", "10.8"),
        ])
    }

    async fn fetch_latest_tick(&self, provider_symbol: &str) -> Result<ProviderTick, AppError> {
        if provider_symbol != "BBB" {
            return Err(AppError::Validation {
                message: format!("unexpected fallback provider symbol: {provider_symbol}"),
                source: None,
            });
        }

        Ok(ProviderTick {
            price: "11.0".parse().expect("price should parse"),
            size: Some("1.5".parse().expect("size should parse")),
            tick_time: utc("2024-01-02T10:05:00Z"),
        })
    }

    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

fn bar(open_time: &str, open: &str, high: &str, low: &str, close: &str) -> ProviderKline {
    let open_time = utc(open_time);

    ProviderKline {
        open_time,
        close_time: open_time + chrono::Duration::minutes(15),
        open: open.parse().expect("open should be decimal"),
        high: high.parse().expect("high should be decimal"),
        low: low.parse().expect("low should be decimal"),
        close: close.parse().expect("close should be decimal"),
        volume: Some("100".parse().expect("volume should be decimal")),
    }
}

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp should be valid")
        .with_timezone(&Utc)
}
