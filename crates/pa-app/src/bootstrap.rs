use pa_core::AppError;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

const SCOPE_TYPE_MARKET: &str = "market";
const PROVIDER_EASTMONEY: &str = "eastmoney";
const PROVIDER_TWELVEDATA: &str = "twelvedata";

pub async fn seed_local_test_instruments_if_empty(
    pool: &PgPool,
    enabled: bool,
) -> Result<(), AppError> {
    if !enabled {
        tracing::info!("local instrument bootstrap disabled");
        return Ok(());
    }

    let markets = table_count(pool, "markets").await?;
    let instruments = table_count(pool, "instruments").await?;
    let bindings = table_count(pool, "instrument_symbol_bindings").await?;
    let policies = table_count(pool, "provider_policies").await?;

    if [markets, instruments, bindings, policies]
        .into_iter()
        .any(|count| count > 0)
    {
        tracing::info!(
            markets,
            instruments,
            bindings,
            policies,
            "local instrument bootstrap skipped because database is not empty"
        );
        return Ok(());
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(storage_error("failed to begin bootstrap transaction"))?;

    let cn_a_market_id = insert_market(&mut tx, "cn-a", "China A Share", "Asia/Shanghai").await?;
    insert_market_policy(
        &mut tx,
        cn_a_market_id,
        PROVIDER_EASTMONEY,
        Some(PROVIDER_TWELVEDATA),
        PROVIDER_EASTMONEY,
        Some(PROVIDER_TWELVEDATA),
    )
    .await?;

    let crypto_market_id = insert_market(&mut tx, "crypto", "Crypto", "UTC").await?;
    insert_market_policy(
        &mut tx,
        crypto_market_id,
        PROVIDER_TWELVEDATA,
        None,
        PROVIDER_TWELVEDATA,
        None,
    )
    .await?;

    let fx_market_id = insert_market(&mut tx, "fx", "Foreign Exchange", "UTC").await?;
    insert_market_policy(
        &mut tx,
        fx_market_id,
        PROVIDER_TWELVEDATA,
        None,
        PROVIDER_TWELVEDATA,
        None,
    )
    .await?;

    for (symbol, eastmoney_symbol, twelvedata_symbol) in [
        ("002025", "0.002025", "002025.SZ"),
        ("002230", "0.002230", "002230.SZ"),
        ("601336", "1.601336", "601336.SS"),
        ("601888", "1.601888", "601888.SS"),
        ("159273", "0.159273", "159273.SZ"),
    ] {
        let instrument_id =
            insert_instrument(&mut tx, cn_a_market_id, symbol, symbol, "equity").await?;
        insert_binding(&mut tx, instrument_id, PROVIDER_EASTMONEY, eastmoney_symbol).await?;
        insert_binding(&mut tx, instrument_id, PROVIDER_TWELVEDATA, twelvedata_symbol).await?;
    }

    for (symbol, provider_symbol) in [("BTC", "BTC/USD"), ("ETH", "ETH/USD")] {
        let instrument_id =
            insert_instrument(&mut tx, crypto_market_id, symbol, symbol, "crypto").await?;
        insert_binding(&mut tx, instrument_id, PROVIDER_TWELVEDATA, provider_symbol).await?;
    }

    for (symbol, provider_symbol) in [("USDJPY", "USD/JPY"), ("EURUSD", "EUR/USD")] {
        let instrument_id = insert_instrument(&mut tx, fx_market_id, symbol, symbol, "forex").await?;
        insert_binding(&mut tx, instrument_id, PROVIDER_TWELVEDATA, provider_symbol).await?;
    }

    tx.commit()
        .await
        .map_err(storage_error("failed to commit bootstrap transaction"))?;

    tracing::info!(
        markets = 3,
        instruments = 9,
        bindings = 14,
        policies = 3,
        "seeded local test instruments"
    );

    Ok(())
}

async fn table_count(pool: &PgPool, table: &str) -> Result<i64, AppError> {
    let sql = format!("SELECT COUNT(*) AS count FROM {table}");
    let row = sqlx::query(&sql)
        .fetch_one(pool)
        .await
        .map_err(storage_error(&format!("failed to count rows in {table}")))?;
    row.try_get::<i64, _>("count")
        .map_err(storage_error(&format!("failed to read count column from {table}")))
}

async fn insert_market(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
    name: &str,
    timezone: &str,
) -> Result<Uuid, AppError> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO markets (id, code, name, timezone)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(id)
    .bind(code)
    .bind(name)
    .bind(timezone)
    .execute(&mut **tx)
    .await
    .map_err(storage_error(&format!("failed to insert market {code}")))?;
    Ok(id)
}

async fn insert_market_policy(
    tx: &mut Transaction<'_, Postgres>,
    market_id: Uuid,
    kline_primary: &str,
    kline_fallback: Option<&str>,
    tick_primary: &str,
    tick_fallback: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO provider_policies (
            id,
            scope_type,
            market_id,
            instrument_id,
            kline_primary,
            kline_fallback,
            tick_primary,
            tick_fallback
        )
        VALUES ($1, $2, $3, NULL, $4, $5, $6, $7)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(SCOPE_TYPE_MARKET)
    .bind(market_id)
    .bind(kline_primary)
    .bind(kline_fallback)
    .bind(tick_primary)
    .bind(tick_fallback)
    .execute(&mut **tx)
    .await
    .map_err(storage_error("failed to insert market provider policy"))?;
    Ok(())
}

async fn insert_instrument(
    tx: &mut Transaction<'_, Postgres>,
    market_id: Uuid,
    symbol: &str,
    name: &str,
    instrument_type: &str,
) -> Result<Uuid, AppError> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO instruments (id, market_id, symbol, name, instrument_type)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(id)
    .bind(market_id)
    .bind(symbol)
    .bind(name)
    .bind(instrument_type)
    .execute(&mut **tx)
    .await
    .map_err(storage_error(&format!("failed to insert instrument {symbol}")))?;
    Ok(id)
}

async fn insert_binding(
    tx: &mut Transaction<'_, Postgres>,
    instrument_id: Uuid,
    provider: &str,
    provider_symbol: &str,
) -> Result<(), AppError> {
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
    .execute(&mut **tx)
    .await
    .map_err(storage_error(&format!(
        "failed to insert binding {provider}/{provider_symbol}"
    )))?;
    Ok(())
}

fn storage_error(message: &str) -> impl FnOnce(sqlx::Error) -> AppError + '_ {
    move |source| AppError::Storage {
        message: message.to_string(),
        source: Some(Box::new(source)),
    }
}
