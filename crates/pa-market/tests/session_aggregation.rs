use chrono::{DateTime, Utc};
use pa_core::Timeframe;
use pa_market::{
    AggregateCanonicalKlinesRequest, CanonicalKlineQuery, CanonicalKlineRepository,
    CanonicalKlineRow, InMemoryCanonicalKlineRepository, aggregate_canonical_klines,
};
use rust_decimal::Decimal;
use uuid::Uuid;

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp should be valid")
        .with_timezone(&Utc)
}

fn decimal(value: &str) -> Decimal {
    value.parse().expect("fixture decimal should parse")
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
        source_provider: "eastmoney".to_string(),
    }
}

async fn insert_rows(
    repository: &InMemoryCanonicalKlineRepository,
    rows: Vec<CanonicalKlineRow>,
) {
    for row in rows {
        repository
            .upsert_canonical_kline(row)
            .await
            .expect("fixture row should insert");
    }
}

#[tokio::test]
async fn cn_a_aggregation_from_15m_to_1h_respects_session_boundaries() {
    let repository = InMemoryCanonicalKlineRepository::default();
    let instrument_id = Uuid::new_v4();
    insert_rows(
        &repository,
        vec![
            row(instrument_id, "2024-01-02T01:30:00Z", "10.0", "10.2", "9.9", "10.1"),
            row(instrument_id, "2024-01-02T01:45:00Z", "10.1", "10.3", "10.0", "10.2"),
            row(instrument_id, "2024-01-02T02:00:00Z", "10.2", "10.4", "10.1", "10.3"),
            row(instrument_id, "2024-01-02T02:15:00Z", "10.3", "10.5", "10.2", "10.4"),
            row(instrument_id, "2024-01-02T02:30:00Z", "10.4", "10.6", "10.3", "10.5"),
            row(instrument_id, "2024-01-02T02:45:00Z", "10.5", "10.7", "10.4", "10.6"),
            row(instrument_id, "2024-01-02T03:00:00Z", "10.6", "10.8", "10.5", "10.7"),
            row(instrument_id, "2024-01-02T03:15:00Z", "10.7", "10.9", "10.6", "10.8"),
            row(instrument_id, "2024-01-02T05:00:00Z", "10.8", "11.0", "10.7", "10.9"),
            row(instrument_id, "2024-01-02T05:15:00Z", "10.9", "11.1", "10.8", "11.0"),
            row(instrument_id, "2024-01-02T05:30:00Z", "11.0", "11.2", "10.9", "11.1"),
            row(instrument_id, "2024-01-02T05:45:00Z", "11.1", "11.3", "11.0", "11.2"),
            row(instrument_id, "2024-01-02T06:00:00Z", "11.2", "11.4", "11.1", "11.3"),
            row(instrument_id, "2024-01-02T06:15:00Z", "11.3", "11.5", "11.2", "11.4"),
            row(instrument_id, "2024-01-02T06:30:00Z", "11.4", "11.6", "11.3", "11.5"),
            row(instrument_id, "2024-01-02T06:45:00Z", "11.5", "11.7", "11.4", "11.6"),
        ],
    )
    .await;

    let rows = aggregate_canonical_klines(
        &repository,
        AggregateCanonicalKlinesRequest {
            instrument_id,
            source_timeframe: Timeframe::M15,
            target_timeframe: Timeframe::H1,
            start_open_time: None,
            end_open_time: None,
            limit: 16,
            market_code: Some("cn-a".to_string()),
            market_timezone: Some("Asia/Shanghai".to_string()),
        },
    )
    .await
    .expect("cn-a aggregation should succeed");

    let open_times = rows.iter().map(|row| row.open_time).collect::<Vec<_>>();
    assert_eq!(
        open_times,
        vec![
            utc("2024-01-02T01:30:00Z"),
            utc("2024-01-02T02:30:00Z"),
            utc("2024-01-02T05:00:00Z"),
            utc("2024-01-02T06:00:00Z"),
        ]
    );
    assert!(rows.iter().all(|row| row.complete));
    assert!(rows.iter().all(|row| row.child_bar_count == 4));
}

#[tokio::test]
async fn cn_a_aggregation_from_15m_to_1d_uses_trading_day_session() {
    let repository = InMemoryCanonicalKlineRepository::default();
    let instrument_id = Uuid::new_v4();
    insert_rows(
        &repository,
        vec![
            row(instrument_id, "2024-01-02T01:30:00Z", "10.0", "10.2", "9.9", "10.1"),
            row(instrument_id, "2024-01-02T01:45:00Z", "10.1", "10.3", "10.0", "10.2"),
            row(instrument_id, "2024-01-02T02:00:00Z", "10.2", "10.4", "10.1", "10.3"),
            row(instrument_id, "2024-01-02T02:15:00Z", "10.3", "10.5", "10.2", "10.4"),
            row(instrument_id, "2024-01-02T02:30:00Z", "10.4", "10.6", "10.3", "10.5"),
            row(instrument_id, "2024-01-02T02:45:00Z", "10.5", "10.7", "10.4", "10.6"),
            row(instrument_id, "2024-01-02T03:00:00Z", "10.6", "10.8", "10.5", "10.7"),
            row(instrument_id, "2024-01-02T03:15:00Z", "10.7", "10.9", "10.6", "10.8"),
            row(instrument_id, "2024-01-02T05:00:00Z", "10.8", "11.0", "10.7", "10.9"),
            row(instrument_id, "2024-01-02T05:15:00Z", "10.9", "11.1", "10.8", "11.0"),
            row(instrument_id, "2024-01-02T05:30:00Z", "11.0", "11.2", "10.9", "11.1"),
            row(instrument_id, "2024-01-02T05:45:00Z", "11.1", "11.3", "11.0", "11.2"),
            row(instrument_id, "2024-01-02T06:00:00Z", "11.2", "11.4", "11.1", "11.3"),
            row(instrument_id, "2024-01-02T06:15:00Z", "11.3", "11.5", "11.2", "11.4"),
            row(instrument_id, "2024-01-02T06:30:00Z", "11.4", "11.6", "11.3", "11.5"),
            row(instrument_id, "2024-01-02T06:45:00Z", "11.5", "11.7", "11.4", "11.6"),
        ],
    )
    .await;

    let rows = aggregate_canonical_klines(
        &repository,
        AggregateCanonicalKlinesRequest {
            instrument_id,
            source_timeframe: Timeframe::M15,
            target_timeframe: Timeframe::D1,
            start_open_time: None,
            end_open_time: None,
            limit: 16,
            market_code: Some("cn-a".to_string()),
            market_timezone: Some("Asia/Shanghai".to_string()),
        },
    )
    .await
    .expect("cn-a day aggregation should succeed");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].open_time, utc("2024-01-02T01:30:00Z"));
    assert_eq!(rows[0].close_time, utc("2024-01-02T07:00:00Z"));
    assert_eq!(rows[0].expected_child_bar_count, 16);
    assert_eq!(rows[0].child_bar_count, 16);
    assert!(rows[0].complete);
}

#[tokio::test]
async fn cn_a_aggregation_ignores_invalid_1500_open_bar_from_provider_tail() {
    let repository = InMemoryCanonicalKlineRepository::default();
    let instrument_id = Uuid::new_v4();
    insert_rows(
        &repository,
        vec![
            row(instrument_id, "2024-01-02T05:15:00Z", "11.12", "11.13", "11.09", "11.09"),
            row(instrument_id, "2024-01-02T05:30:00Z", "11.09", "11.11", "11.08", "11.09"),
            row(instrument_id, "2024-01-02T05:45:00Z", "11.09", "11.10", "11.08", "11.09"),
            row(instrument_id, "2024-01-02T06:00:00Z", "11.10", "11.10", "11.08", "11.08"),
            row(instrument_id, "2024-01-02T06:15:00Z", "11.08", "11.10", "11.08", "11.08"),
            row(instrument_id, "2024-01-02T06:30:00Z", "11.09", "11.10", "11.08", "11.09"),
            row(instrument_id, "2024-01-02T06:45:00Z", "11.08", "11.09", "11.07", "11.08"),
            row(instrument_id, "2024-01-02T07:00:00Z", "11.07", "11.10", "11.07", "11.08"),
        ],
    )
    .await;

    let rows = aggregate_canonical_klines(
        &repository,
        AggregateCanonicalKlinesRequest {
            instrument_id,
            source_timeframe: Timeframe::M15,
            target_timeframe: Timeframe::H1,
            start_open_time: None,
            end_open_time: None,
            limit: 8,
            market_code: Some("cn-a".to_string()),
            market_timezone: Some("Asia/Shanghai".to_string()),
        },
    )
    .await
    .expect("invalid tail bar should be ignored rather than fail aggregation");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].open_time, utc("2024-01-02T05:00:00Z"));
    assert_eq!(rows[0].child_bar_count, 3);
    assert_eq!(rows[1].open_time, utc("2024-01-02T06:00:00Z"));
    assert_eq!(rows[1].child_bar_count, 4);
    assert!(rows.iter().all(|row| row.open_time != utc("2024-01-02T07:00:00Z")));
}

#[tokio::test]
async fn repository_query_fixture_remains_sorted_for_session_tests() {
    let repository = InMemoryCanonicalKlineRepository::default();
    let instrument_id = Uuid::new_v4();
    insert_rows(
        &repository,
        vec![
            row(instrument_id, "2024-01-02T05:15:00Z", "10", "10", "10", "10"),
            row(instrument_id, "2024-01-02T01:30:00Z", "10", "10", "10", "10"),
        ],
    )
    .await;

    let rows = repository
        .list_canonical_klines(CanonicalKlineQuery {
            instrument_id,
            timeframe: Timeframe::M15,
            start_open_time: None,
            end_open_time: None,
            limit: 10,
            descending: false,
        })
        .await
        .expect("fixture query should work");

    assert_eq!(rows[0].open_time, utc("2024-01-02T01:30:00Z"));
    assert_eq!(rows[1].open_time, utc("2024-01-02T05:15:00Z"));
}
