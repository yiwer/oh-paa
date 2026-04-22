use chrono::{DateTime, Utc};
use pa_core::Timeframe;
use pa_market::{
    CanonicalKlineQuery, CanonicalKlineRepository, CanonicalKlineRow, PgCanonicalKlineRepository,
};
use sqlx::PgPool;
use std::path::Path;
use uuid::Uuid;

#[tokio::test]
async fn pg_repository_upserts_and_reads_canonical_rows() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_upserts_and_reads_canonical_rows: PA_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgCanonicalKlineRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    let mut row = fixture_row(instrument_id, "2024-01-02T09:30:00Z");
    repository
        .upsert_canonical_kline(row.clone())
        .await
        .expect("initial upsert should succeed");

    row.close = rust_decimal::Decimal::new(109, 1);
    row.source_provider = "fallback".to_string();
    repository
        .upsert_canonical_kline(row.clone())
        .await
        .expect("conflicting upsert should update the stored row");

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
        .expect("stored rows should be readable");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].close, rust_decimal::Decimal::new(109, 1));
    assert_eq!(rows[0].source_provider, "fallback");

    cleanup_market_and_instrument(&pool, market_id, instrument_id).await;
}

#[tokio::test]
async fn pg_repository_filters_by_window_and_descending_order() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping pg_repository_filters_by_window_and_descending_order: PA_DATABASE_URL not set"
        );
        return;
    };
    let repository = PgCanonicalKlineRepository::new(pool.clone());
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();

    ensure_market_and_instrument(&pool, market_id, instrument_id).await;

    for timestamp in [
        "2024-01-02T09:30:00Z",
        "2024-01-02T09:45:00Z",
        "2024-01-02T10:00:00Z",
    ] {
        repository
            .upsert_canonical_kline(fixture_row(instrument_id, timestamp))
            .await
            .expect("seed row should upsert");
    }

    let rows = repository
        .list_canonical_klines(CanonicalKlineQuery {
            instrument_id,
            timeframe: Timeframe::M15,
            start_open_time: Some(utc("2024-01-02T09:45:00Z")),
            end_open_time: Some(utc("2024-01-02T10:00:00Z")),
            limit: 1,
            descending: true,
        })
        .await
        .expect("windowed rows should be readable");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].open_time, utc("2024-01-02T10:00:00Z"));

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
    .bind(format!("MKT-{}", &market_id.simple()))
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
    .bind(format!("SYM-{}", &instrument_id.simple()))
    .bind("Test Instrument")
    .bind("crypto")
    .execute(pool)
    .await
    .expect("instrument seed should insert");
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

fn fixture_row(instrument_id: Uuid, open_time: &str) -> CanonicalKlineRow {
    let open_time = utc(open_time);

    CanonicalKlineRow {
        instrument_id,
        timeframe: Timeframe::M15,
        open_time,
        close_time: open_time + chrono::Duration::minutes(15),
        open: rust_decimal::Decimal::new(101, 1),
        high: rust_decimal::Decimal::new(110, 1),
        low: rust_decimal::Decimal::new(100, 1),
        close: rust_decimal::Decimal::new(108, 1),
        volume: Some(rust_decimal::Decimal::new(12_345, 0)),
        source_provider: "primary".to_string(),
    }
}

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp should be valid")
        .with_timezone(&Utc)
}
