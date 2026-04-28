use sqlx::{PgPool, Row};
use std::path::Path;
use uuid::Uuid;

#[tokio::test]
async fn bootstrap_seeds_fixed_local_instruments_when_enabled_and_database_is_empty() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping bootstrap_seeds_fixed_local_instruments_when_enabled_and_database_is_empty: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    clear_bootstrap_tables(&pool).await;

    pa_app::bootstrap::seed_local_test_instruments_if_empty(&pool, true)
        .await
        .expect("bootstrap should succeed");

    assert_eq!(table_count(&pool, "markets").await, 3);
    assert_eq!(table_count(&pool, "instruments").await, 9);
    assert_eq!(table_count(&pool, "instrument_symbol_bindings").await, 14);
    assert_eq!(table_count(&pool, "provider_policies").await, 3);

    assert_binding(&pool, "002025", "eastmoney", "0.002025").await;
    assert_binding(&pool, "601336", "eastmoney", "1.601336").await;
    assert_binding(&pool, "601888", "twelvedata", "601888.SS").await;
    assert_binding(&pool, "BTC", "twelvedata", "BTC/USD").await;
    assert_binding(&pool, "USDJPY", "twelvedata", "USD/JPY").await;

    clear_bootstrap_tables(&pool).await;
}

#[tokio::test]
async fn bootstrap_skips_when_disabled() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping bootstrap_skips_when_disabled: PA_TEST_DATABASE_URL not set");
        return;
    };
    clear_bootstrap_tables(&pool).await;

    pa_app::bootstrap::seed_local_test_instruments_if_empty(&pool, false)
        .await
        .expect("disabled bootstrap should be a no-op");

    assert_eq!(table_count(&pool, "markets").await, 0);
    assert_eq!(table_count(&pool, "instruments").await, 0);
}

#[tokio::test]
async fn bootstrap_skips_when_database_is_not_empty() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping bootstrap_skips_when_database_is_not_empty: PA_TEST_DATABASE_URL not set"
        );
        return;
    };
    clear_bootstrap_tables(&pool).await;
    insert_single_market(&pool).await;

    pa_app::bootstrap::seed_local_test_instruments_if_empty(&pool, true)
        .await
        .expect("non-empty bootstrap should skip");

    assert_eq!(table_count(&pool, "markets").await, 1);
    assert_eq!(table_count(&pool, "instruments").await, 0);

    clear_bootstrap_tables(&pool).await;
}

async fn test_pool() -> Option<PgPool> {
    let database_url = std::env::var("PA_TEST_DATABASE_URL").ok()?;

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

async fn clear_bootstrap_tables(pool: &PgPool) {
    for stmt in [
        "DELETE FROM instrument_symbol_bindings",
        "DELETE FROM provider_policies",
        "DELETE FROM instruments",
        "DELETE FROM markets",
    ] {
        sqlx::query(stmt)
            .execute(pool)
            .await
            .unwrap_or_else(|err| panic!("clear `{stmt}` should succeed: {err}"));
    }
}

async fn table_count(pool: &PgPool, table: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) AS count FROM {table}");
    let row = sqlx::query(&sql)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("count from {table} should succeed: {err}"));
    row.try_get::<i64, _>("count")
        .expect("count column should be i64")
}

async fn assert_binding(pool: &PgPool, symbol: &str, provider: &str, expected_provider_symbol: &str) {
    let row = sqlx::query(
        r#"
        SELECT b.provider_symbol AS provider_symbol
        FROM instrument_symbol_bindings b
        INNER JOIN instruments i ON i.id = b.instrument_id
        WHERE i.symbol = $1 AND b.provider = $2
        "#,
    )
    .bind(symbol)
    .bind(provider)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|err| panic!("binding lookup for {symbol}/{provider} should succeed: {err}"));

    let provider_symbol: String = row
        .try_get("provider_symbol")
        .expect("provider_symbol column should be string");
    assert_eq!(
        provider_symbol, expected_provider_symbol,
        "binding {symbol}/{provider} should map to {expected_provider_symbol}"
    );
}

async fn insert_single_market(pool: &PgPool) {
    sqlx::query(
        r#"
        INSERT INTO markets (id, code, name, timezone)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind("placeholder")
    .bind("Placeholder Market")
    .bind("UTC")
    .execute(pool)
    .await
    .expect("placeholder market should insert");
}
