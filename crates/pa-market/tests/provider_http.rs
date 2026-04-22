use std::sync::{Arc, Mutex};

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::{DateTime, Utc};
use pa_core::Timeframe;
use pa_market::{
    MarketDataProvider,
    provider::providers::{EastMoneyProvider, TwelveDataProvider},
};
use serde::Deserialize;

#[tokio::test]
async fn twelvedata_fetch_klines_uses_live_http_contract() {
    let state = Arc::new(Mutex::new(Vec::<ObservedRequest>::new()));
    let server = TestServer::spawn(
        Router::new()
            .route("/time_series", get(twelvedata_time_series))
            .route("/quote", get(twelvedata_quote))
            .with_state(Arc::clone(&state)),
    )
    .await;
    let provider = TwelveDataProvider::new(server.base_url(), "test-key");

    let klines = provider
        .fetch_klines("BTC/USD", Timeframe::M15, 2)
        .await
        .expect("time series request should succeed");

    assert_eq!(klines.len(), 1);
    assert_eq!(klines[0].open_time, utc("2024-01-02T09:30:00Z"));
    assert_eq!(klines[0].close_time, utc("2024-01-02T09:45:00Z"));

    let requests = state.lock().expect("request log should lock");
    let request = requests
        .iter()
        .find(|request| request.path == "/time_series")
        .expect("time_series request should be recorded");

    assert_eq!(request.query_value("symbol"), Some("BTC/USD"));
    assert_eq!(request.query_value("interval"), Some("15min"));
    assert_eq!(request.query_value("outputsize"), Some("2"));
    assert_eq!(request.query_value("apikey"), Some("test-key"));
}

#[tokio::test]
async fn twelvedata_fetch_latest_tick_uses_quote_endpoint() {
    let state = Arc::new(Mutex::new(Vec::<ObservedRequest>::new()));
    let server = TestServer::spawn(
        Router::new()
            .route("/time_series", get(twelvedata_time_series))
            .route("/quote", get(twelvedata_quote))
            .with_state(Arc::clone(&state)),
    )
    .await;
    let provider = TwelveDataProvider::new(server.base_url(), "test-key");

    let tick = provider
        .fetch_latest_tick("BTC/USD")
        .await
        .expect("quote request should succeed");

    assert_eq!(tick.tick_time, utc("2024-01-02T09:30:05Z"));
    assert_eq!(tick.price.to_string(), "10.8");

    let requests = state.lock().expect("request log should lock");
    let request = requests
        .iter()
        .find(|request| request.path == "/quote")
        .expect("quote request should be recorded");

    assert_eq!(request.query_value("symbol"), Some("BTC/USD"));
    assert_eq!(request.query_value("apikey"), Some("test-key"));
}

#[tokio::test]
async fn eastmoney_fetch_klines_uses_live_http_contract() {
    let state = Arc::new(Mutex::new(Vec::<ObservedRequest>::new()));
    let server = TestServer::spawn(
        Router::new()
            .route("/api/qt/stock/kline/get", get(eastmoney_kline))
            .route("/api/qt/stock/get", get(eastmoney_quote))
            .with_state(Arc::clone(&state)),
    )
    .await;
    let provider = EastMoneyProvider::new(server.base_url());

    let klines = provider
        .fetch_klines("0.000001", Timeframe::M15, 2)
        .await
        .expect("eastmoney kline request should succeed");

    assert_eq!(klines.len(), 1);
    assert_eq!(klines[0].open_time, utc("2024-01-02T01:30:00Z"));
    assert_eq!(klines[0].close_time, utc("2024-01-02T01:45:00Z"));

    let requests = state.lock().expect("request log should lock");
    let request = requests
        .iter()
        .find(|request| request.path == "/api/qt/stock/kline/get")
        .expect("eastmoney kline request should be recorded");

    assert_eq!(request.query_value("secid"), Some("0.000001"));
    assert_eq!(request.query_value("klt"), Some("15"));
    assert_eq!(request.query_value("lmt"), Some("2"));
}

#[tokio::test]
async fn eastmoney_fetch_latest_tick_uses_quote_endpoint() {
    let state = Arc::new(Mutex::new(Vec::<ObservedRequest>::new()));
    let server = TestServer::spawn(
        Router::new()
            .route("/api/qt/stock/kline/get", get(eastmoney_kline))
            .route("/api/qt/stock/get", get(eastmoney_quote))
            .with_state(Arc::clone(&state)),
    )
    .await;
    let provider = EastMoneyProvider::new(server.base_url());

    let tick = provider
        .fetch_latest_tick("0.000001")
        .await
        .expect("eastmoney quote request should succeed");

    assert_eq!(tick.tick_time, utc("2024-01-02T09:30:05Z"));
    assert_eq!(tick.price.to_string(), "10.8");

    let requests = state.lock().expect("request log should lock");
    let request = requests
        .iter()
        .find(|request| request.path == "/api/qt/stock/get")
        .expect("eastmoney quote request should be recorded");

    assert_eq!(request.query_value("secid"), Some("0.000001"));
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
            "values": [
                {
                    "datetime": "2024-01-02T09:30:00Z",
                    "open": "10.1",
                    "high": "11.0",
                    "low": "10.0",
                    "close": "10.8",
                    "volume": "12345"
                }
            ]
        })),
    )
}

async fn twelvedata_quote(
    State(state): State<Arc<Mutex<Vec<ObservedRequest>>>>,
    Query(query): Query<RequestQuery>,
) -> impl IntoResponse {
    record_request(state, "/quote", query);

    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "price": "10.8",
            "volume": "2300",
            "timestamp": "2024-01-02T09:30:05Z"
        })),
    )
}

async fn eastmoney_kline(
    State(state): State<Arc<Mutex<Vec<ObservedRequest>>>>,
    Query(query): Query<RequestQuery>,
) -> impl IntoResponse {
    record_request(state, "/api/qt/stock/kline/get", query);

    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "data": {
                "klines": [
                    "2024-01-02 09:30,10.1,10.8,11.0,10.0,12345"
                ]
            }
        })),
    )
}

async fn eastmoney_quote(
    State(state): State<Arc<Mutex<Vec<ObservedRequest>>>>,
    Query(query): Query<RequestQuery>,
) -> impl IntoResponse {
    record_request(state, "/api/qt/stock/get", query);

    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "data": {
                "price": "10.8",
                "volume": "2300",
                "timestamp": "2024-01-02T09:30:05Z"
            }
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

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp should be valid")
        .with_timezone(&Utc)
}
