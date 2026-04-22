use pa_instrument::InstrumentRepository;
use sqlx::PgPool;
use std::path::Path;
use uuid::Uuid;

#[tokio::test]
async fn resolves_market_policy_when_instrument_policy_is_missing() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping resolves_market_policy_when_instrument_policy_is_missing: PA_DATABASE_URL not set"
        );
        return;
    };
    let repository = InstrumentRepository::new(pool.clone());
    let fixture = seed_runtime_fixture(&pool, false).await;

    let context = repository
        .resolve_market_data_context(fixture.instrument_id)
        .await
        .expect("market context should resolve");

    assert_eq!(context.policy.kline_primary, "eastmoney");
    assert_eq!(context.policy.kline_fallback.as_deref(), Some("twelvedata"));
    assert_eq!(
        context
            .binding_for_provider("eastmoney")
            .expect("eastmoney binding should exist")
            .provider_symbol,
        "0.000001"
    );

    cleanup_runtime_fixture(&pool, &fixture).await;
}

#[tokio::test]
async fn resolves_instrument_policy_over_market_policy() {
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping resolves_instrument_policy_over_market_policy: PA_DATABASE_URL not set"
        );
        return;
    };
    let repository = InstrumentRepository::new(pool.clone());
    let fixture = seed_runtime_fixture(&pool, true).await;

    let context = repository
        .resolve_market_data_context(fixture.instrument_id)
        .await
        .expect("instrument policy should resolve");

    assert_eq!(context.policy.kline_primary, "twelvedata");
    assert_eq!(context.policy.kline_fallback.as_deref(), Some("eastmoney"));
    assert_eq!(context.bindings.len(), 2);

    cleanup_runtime_fixture(&pool, &fixture).await;
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

async fn seed_runtime_fixture(pool: &PgPool, with_instrument_policy: bool) -> RuntimeFixture {
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();
    let market_policy_id = Uuid::new_v4();
    let eastmoney_binding_id = Uuid::new_v4();
    let twelvedata_binding_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO markets (id, code, name, timezone)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(market_id)
    .bind(format!("MKT-{}", market_id.simple()))
    .bind("CN A")
    .bind("Asia/Shanghai")
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
    .bind("Ping An")
    .bind("equity")
    .execute(pool)
    .await
    .expect("instrument seed should insert");

    for (binding_id, provider, provider_symbol) in [
        (eastmoney_binding_id, "eastmoney", "0.000001"),
        (twelvedata_binding_id, "twelvedata", "000001.SZ"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO instrument_symbol_bindings (id, instrument_id, provider, provider_symbol)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(binding_id)
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
    .bind(market_policy_id)
    .bind(market_id)
    .bind("eastmoney")
    .bind(Some("twelvedata"))
    .bind("eastmoney")
    .bind(Some("twelvedata"))
    .execute(pool)
    .await
    .expect("market policy seed should insert");

    if with_instrument_policy {
        let instrument_policy_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO provider_policies (
                id, scope_type, market_id, instrument_id, kline_primary, kline_fallback, tick_primary, tick_fallback
            ) VALUES ($1, 'instrument', NULL, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(instrument_policy_id)
        .bind(instrument_id)
        .bind("twelvedata")
        .bind(Some("eastmoney"))
        .bind("twelvedata")
        .bind(Some("eastmoney"))
        .execute(pool)
        .await
        .expect("instrument policy seed should insert");
    }

    RuntimeFixture {
        market_id,
        instrument_id,
    }
}

async fn cleanup_runtime_fixture(pool: &PgPool, fixture: &RuntimeFixture) {
    sqlx::query("DELETE FROM provider_policies WHERE instrument_id = $1 OR market_id = $2")
        .bind(fixture.instrument_id)
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
