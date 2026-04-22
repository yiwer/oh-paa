use pa_core::AppError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    models::{Instrument, InstrumentSymbolBinding, Market, PolicyScope, ProviderPolicy},
    resolve_policy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentMarketDataContext {
    pub market: Market,
    pub instrument: Instrument,
    pub policy: ProviderPolicy,
    pub bindings: Vec<InstrumentSymbolBinding>,
}

impl InstrumentMarketDataContext {
    pub fn binding_for_provider(
        &self,
        provider: &str,
    ) -> Result<&InstrumentSymbolBinding, AppError> {
        self.bindings
            .iter()
            .find(|binding| binding.provider == provider)
            .ok_or_else(|| AppError::Validation {
                message: format!(
                    "instrument {} is missing provider binding for `{provider}`",
                    self.instrument.id
                ),
                source: None,
            })
    }
}

#[derive(Debug, Clone)]
pub struct InstrumentRepository {
    pool: PgPool,
}

impl InstrumentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn resolve_market_data_context(
        &self,
        instrument_id: Uuid,
    ) -> Result<InstrumentMarketDataContext, AppError> {
        let instrument =
            self.load_instrument(instrument_id)
                .await?
                .ok_or_else(|| AppError::Validation {
                    message: format!("instrument not found: {instrument_id}"),
                    source: None,
                })?;
        let market = self
            .load_market(instrument.market_id)
            .await?
            .ok_or_else(|| AppError::Storage {
                message: format!(
                    "instrument {} references missing market {}",
                    instrument.id, instrument.market_id
                ),
                source: None,
            })?;
        let bindings = self.load_bindings(instrument.id).await?;
        let instrument_policy = self.load_instrument_policy(instrument.id).await?;
        let market_policy = self.load_market_policy(market.id).await?;
        let policy = resolve_policy(instrument_policy.as_ref(), market_policy.as_ref())?;

        Ok(InstrumentMarketDataContext {
            market,
            instrument,
            policy,
            bindings,
        })
    }

    async fn load_instrument(&self, instrument_id: Uuid) -> Result<Option<Instrument>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT id, market_id, symbol, name, instrument_type, created_at, updated_at
            FROM instruments
            WHERE id = $1
            "#,
        )
        .bind(instrument_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_query_error("failed to load instrument"))?;

        row.map(map_instrument_row).transpose()
    }

    async fn load_market(&self, market_id: Uuid) -> Result<Option<Market>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT id, code, name, timezone, created_at, updated_at
            FROM markets
            WHERE id = $1
            "#,
        )
        .bind(market_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_query_error("failed to load market"))?;

        row.map(map_market_row).transpose()
    }

    async fn load_bindings(
        &self,
        instrument_id: Uuid,
    ) -> Result<Vec<InstrumentSymbolBinding>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT id, instrument_id, provider, provider_symbol, created_at
            FROM instrument_symbol_bindings
            WHERE instrument_id = $1
            ORDER BY provider ASC
            "#,
        )
        .bind(instrument_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage_query_error(
            "failed to load instrument symbol bindings",
        ))?;

        rows.into_iter().map(map_binding_row).collect()
    }

    async fn load_instrument_policy(
        &self,
        instrument_id: Uuid,
    ) -> Result<Option<ProviderPolicy>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT scope_type, market_id, instrument_id, kline_primary, kline_fallback, tick_primary, tick_fallback
            FROM provider_policies
            WHERE instrument_id = $1
            "#,
        )
        .bind(instrument_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_query_error("failed to load instrument provider policy"))?;

        row.map(map_policy_row).transpose()
    }

    async fn load_market_policy(
        &self,
        market_id: Uuid,
    ) -> Result<Option<ProviderPolicy>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT scope_type, market_id, instrument_id, kline_primary, kline_fallback, tick_primary, tick_fallback
            FROM provider_policies
            WHERE market_id = $1
            "#,
        )
        .bind(market_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_query_error("failed to load market provider policy"))?;

        row.map(map_policy_row).transpose()
    }
}

fn map_market_row(row: sqlx::postgres::PgRow) -> Result<Market, AppError> {
    Ok(Market {
        id: row.try_get("id").map_err(storage_decode_error)?,
        code: row.try_get("code").map_err(storage_decode_error)?,
        name: row.try_get("name").map_err(storage_decode_error)?,
        timezone: row.try_get("timezone").map_err(storage_decode_error)?,
        created_at: row.try_get("created_at").map_err(storage_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_decode_error)?,
    })
}

fn map_instrument_row(row: sqlx::postgres::PgRow) -> Result<Instrument, AppError> {
    Ok(Instrument {
        id: row.try_get("id").map_err(storage_decode_error)?,
        market_id: row.try_get("market_id").map_err(storage_decode_error)?,
        symbol: row.try_get("symbol").map_err(storage_decode_error)?,
        name: row.try_get("name").map_err(storage_decode_error)?,
        instrument_type: row
            .try_get("instrument_type")
            .map_err(storage_decode_error)?,
        created_at: row.try_get("created_at").map_err(storage_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_decode_error)?,
    })
}

fn map_binding_row(row: sqlx::postgres::PgRow) -> Result<InstrumentSymbolBinding, AppError> {
    Ok(InstrumentSymbolBinding {
        id: row.try_get("id").map_err(storage_decode_error)?,
        instrument_id: row.try_get("instrument_id").map_err(storage_decode_error)?,
        provider: row.try_get("provider").map_err(storage_decode_error)?,
        provider_symbol: row
            .try_get("provider_symbol")
            .map_err(storage_decode_error)?,
        created_at: row.try_get("created_at").map_err(storage_decode_error)?,
    })
}

fn map_policy_row(row: sqlx::postgres::PgRow) -> Result<ProviderPolicy, AppError> {
    let scope_type = row
        .try_get::<String, _>("scope_type")
        .map_err(storage_decode_error)?;
    let market_id = row
        .try_get::<Option<Uuid>, _>("market_id")
        .map_err(storage_decode_error)?;
    let instrument_id = row
        .try_get::<Option<Uuid>, _>("instrument_id")
        .map_err(storage_decode_error)?;
    let scope = match scope_type.as_str() {
        "market" => PolicyScope::Market(
            market_id
                .ok_or_else(|| AppError::Storage {
                    message: "market policy row is missing market_id".into(),
                    source: None,
                })?
                .to_string(),
        ),
        "instrument" => PolicyScope::Instrument(
            instrument_id
                .ok_or_else(|| AppError::Storage {
                    message: "instrument policy row is missing instrument_id".into(),
                    source: None,
                })?
                .to_string(),
        ),
        other => {
            return Err(AppError::Storage {
                message: format!("unsupported provider policy scope_type `{other}`"),
                source: None,
            });
        }
    };

    Ok(ProviderPolicy {
        scope,
        kline_primary: row.try_get("kline_primary").map_err(storage_decode_error)?,
        kline_fallback: row
            .try_get("kline_fallback")
            .map_err(storage_decode_error)?,
        tick_primary: row.try_get("tick_primary").map_err(storage_decode_error)?,
        tick_fallback: row.try_get("tick_fallback").map_err(storage_decode_error)?,
    })
}

fn storage_query_error(message: &'static str) -> impl FnOnce(sqlx::Error) -> AppError {
    move |source| AppError::Storage {
        message: message.to_string(),
        source: Some(Box::new(source)),
    }
}

fn storage_decode_error(source: sqlx::Error) -> AppError {
    AppError::Storage {
        message: "failed to decode instrument repository row".into(),
        source: Some(Box::new(source)),
    }
}

#[cfg(test)]
mod tests {
    use super::{InstrumentRepository, PolicyScope};
    use chrono::Utc;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn repository_wraps_a_pg_pool() {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost/oh_paa")
            .expect("lazy pool should be constructible without a live database");
        let repository = InstrumentRepository::new(pool.clone());

        assert_eq!(repository.pool().size(), 0);
    }

    #[test]
    fn binding_lookup_returns_validation_error_for_missing_provider() {
        let context = super::InstrumentMarketDataContext {
            market: crate::models::Market {
                id: uuid::Uuid::nil(),
                code: "cn-a".to_string(),
                name: "CN A".to_string(),
                timezone: "Asia/Shanghai".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            instrument: crate::models::Instrument {
                id: uuid::Uuid::nil(),
                market_id: uuid::Uuid::nil(),
                symbol: "000001".to_string(),
                name: "Ping An".to_string(),
                instrument_type: "equity".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            policy: crate::models::ProviderPolicy::new(
                PolicyScope::Market(uuid::Uuid::nil().to_string()),
                "eastmoney".to_string(),
                Some("twelvedata".to_string()),
                "eastmoney".to_string(),
                Some("twelvedata".to_string()),
            ),
            bindings: Vec::new(),
        };

        let error = context
            .binding_for_provider("eastmoney")
            .expect_err("missing provider binding should fail");

        match error {
            pa_core::AppError::Validation { message, .. } => {
                assert!(message.contains("missing provider binding"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }
}
