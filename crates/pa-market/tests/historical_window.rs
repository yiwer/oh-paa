use std::sync::{Arc, Mutex};

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use pa_instrument::InstrumentMarketDataContext;
use pa_market::{
    CanonicalKlineRow, HistoricalKlineQuery, MarketDataProvider, aggregate_replay_window_rows,
    provider::providers::TwelveDataProvider,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

#[tokio::test]
async fn twelvedata_fetch_klines_window_uses_explicit_bounds_ascending_order_and_close_boundary() {
    let state = Arc::new(Mutex::new(Vec::<ObservedRequest>::new()));
    let server = TestServer::spawn(
        Router::new()
            .route("/time_series", get(twelvedata_time_series))
            .with_state(Arc::clone(&state)),
    )
    .await;
    let provider = TwelveDataProvider::new(server.base_url(), "test-key");

    let klines = provider
        .fetch_klines_window(HistoricalKlineQuery {
            provider_symbol: "BTC/USD".to_string(),
            timeframe: Timeframe::M15,
            start_open_time: Some(utc("2024-01-02T09:30:00Z")),
            end_close_time: Some(utc("2024-01-02T10:00:00Z")),
            limit: Some(200),
        })
        .await
        .expect("window query should succeed");

    assert_eq!(klines.len(), 2);
    assert_eq!(klines[0].open_time, utc("2024-01-02T09:30:00Z"));
    assert_eq!(klines[0].close_time, utc("2024-01-02T09:45:00Z"));
    assert_eq!(klines[1].open_time, utc("2024-01-02T09:45:00Z"));
    assert_eq!(klines[1].close_time, utc("2024-01-02T10:00:00Z"));

    let requests = state.lock().expect("request log should lock");
    let request = requests
        .iter()
        .find(|request| request.path == "/time_series")
        .expect("time_series request should be recorded");

    assert_eq!(request.query_value("symbol"), Some("BTC/USD"));
    assert_eq!(request.query_value("interval"), Some("15min"));
    assert_eq!(request.query_value("order"), Some("asc"));
    assert_eq!(request.query_value("timezone"), Some("UTC"));
    assert_eq!(request.query_value("outputsize"), None);
    assert_eq!(
        request.query_value("start_date"),
        Some("2024-01-02T09:30:00+00:00")
    );
    assert_eq!(
        request.query_value("end_date"),
        Some("2024-01-02T10:00:00+00:00")
    );
    assert_eq!(request.query_value("apikey"), Some("test-key"));
}

#[test]
fn aggregate_replay_window_rows_builds_complete_hour_from_contiguous_15m_rows() {
    let ctx = ctx_for_test();
    let instrument_id = ctx.instrument.id;
    let rows = vec![
        row(
            instrument_id,
            "2024-01-02T00:00:00Z",
            "10.0",
            "10.1",
            "9.9",
            "10.0",
        ),
        row(
            instrument_id,
            "2024-01-02T00:15:00Z",
            "10.0",
            "10.3",
            "9.95",
            "10.2",
        ),
        row(
            instrument_id,
            "2024-01-02T00:30:00Z",
            "10.2",
            "10.4",
            "10.1",
            "10.35",
        ),
        row(
            instrument_id,
            "2024-01-02T00:45:00Z",
            "10.35",
            "10.5",
            "10.2",
            "10.45",
        ),
    ];

    let aggregated = aggregate_replay_window_rows(&rows, &ctx, Timeframe::M15, Timeframe::H1)
        .expect("in-memory replay aggregation should succeed");

    assert_eq!(aggregated.len(), 1);
    assert_eq!(aggregated[0].open_time, utc("2024-01-02T00:00:00Z"));
    assert_eq!(aggregated[0].close_time, utc("2024-01-02T01:00:00Z"));
    assert_eq!(aggregated[0].open, decimal("10.0"));
    assert_eq!(aggregated[0].high, decimal("10.5"));
    assert_eq!(aggregated[0].low, decimal("9.9"));
    assert_eq!(aggregated[0].close, decimal("10.45"));
    assert_eq!(aggregated[0].volume, Some(decimal("400")));
    assert_eq!(aggregated[0].child_bar_count, 4);
    assert_eq!(aggregated[0].expected_child_bar_count, 4);
    assert!(aggregated[0].complete);
}

#[test]
fn aggregate_replay_window_rows_rejects_duplicate_child_open_time() {
    let ctx = ctx_for_test();
    let instrument_id = ctx.instrument.id;
    let rows = vec![
        row(
            instrument_id,
            "2024-01-02T00:00:00Z",
            "10.0",
            "10.1",
            "9.9",
            "10.0",
        ),
        row(
            instrument_id,
            "2024-01-02T00:15:00Z",
            "10.0",
            "10.3",
            "9.95",
            "10.2",
        ),
        row(
            instrument_id,
            "2024-01-02T00:15:00Z",
            "10.2",
            "10.4",
            "10.1",
            "10.35",
        ),
        row(
            instrument_id,
            "2024-01-02T00:30:00Z",
            "10.35",
            "10.5",
            "10.2",
            "10.45",
        ),
    ];

    let error = aggregate_replay_window_rows(&rows, &ctx, Timeframe::M15, Timeframe::H1)
        .expect_err("duplicate child rows should be rejected");

    match error {
        AppError::Validation { message, .. } => {
            assert!(message.contains("duplicate child row open_time"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[derive(Debug, Clone)]
struct ObservedRequest {
    path: String,
    query: Vec<(String, String)>,
}

impl ObservedRequest {
    fn query_value(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(query_key, _)| query_key == key)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Deserialize)]
struct RequestQuery {
    #[serde(flatten)]
    values: std::collections::HashMap<String, String>,
}

#[derive(Debug)]
struct TestServer {
    base_url: String,
}

impl TestServer {
    async fn spawn(app: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("listener address should exist");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test server should run");
        });

        Self {
            base_url: format!("http://{}", address),
        }
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }
}

async fn twelvedata_time_series(
    State(state): State<Arc<Mutex<Vec<ObservedRequest>>>>,
    Query(query): Query<RequestQuery>,
) -> impl IntoResponse {
    record_request(state, "/time_series", query);

    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "status": "ok",
            "values": [
                {
                    "datetime": "2024-01-02T09:30:00Z",
                    "open": "10.1",
                    "high": "11.0",
                    "low": "10.0",
                    "close": "10.8",
                    "volume": "12345"
                },
                {
                    "datetime": "2024-01-02T09:45:00Z",
                    "open": "10.8",
                    "high": "11.1",
                    "low": "10.7",
                    "close": "11.0",
                    "volume": "12340"
                },
                {
                    "datetime": "2024-01-02T10:00:00Z",
                    "open": "11.0",
                    "high": "11.2",
                    "low": "10.9",
                    "close": "11.1",
                    "volume": "12000"
                }
            ]
        })),
    )
}

fn record_request(state: Arc<Mutex<Vec<ObservedRequest>>>, path: &str, query: RequestQuery) {
    let mut requests = state.lock().expect("request log should lock");
    let mut query_pairs = query.values.into_iter().collect::<Vec<_>>();
    query_pairs.sort_by(|left, right| left.0.cmp(&right.0));
    requests.push(ObservedRequest {
        path: path.to_string(),
        query: query_pairs,
    });
}

fn row(
    instrument_id: Uuid,
    open_time: &str,
    open: &str,
    high: &str,
    low: &str,
    close: &str,
) -> CanonicalKlineRow {
    let open_time = utc(open_time);
    CanonicalKlineRow {
        instrument_id,
        timeframe: Timeframe::M15,
        open_time,
        close_time: open_time + chrono::Duration::minutes(15),
        open: decimal(open),
        high: decimal(high),
        low: decimal(low),
        close: decimal(close),
        volume: Some(decimal("100")),
        source_provider: "twelvedata".to_string(),
    }
}

fn decimal(value: &str) -> Decimal {
    value.parse().expect("fixture decimal should parse")
}

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp should be valid")
        .with_timezone(&Utc)
}

fn ctx_for_test() -> InstrumentMarketDataContext {
    InstrumentMarketDataContext::fixture(
        "continuous-utc",
        "UTC",
        "000001",
        "primary",
        Some("fallback"),
        "primary",
        Some("fallback"),
    )
}
