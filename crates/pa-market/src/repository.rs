use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use rust_decimal::Decimal;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalKlineRow {
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
    pub source_provider: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalKlineQuery {
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub start_open_time: Option<DateTime<Utc>>,
    pub end_open_time: Option<DateTime<Utc>>,
    pub limit: usize,
    pub descending: bool,
}

type CanonicalKey = (Uuid, Timeframe, DateTime<Utc>);

#[async_trait]
pub trait CanonicalKlineRepository: Send + Sync {
    async fn upsert_canonical_kline(&self, row: CanonicalKlineRow) -> Result<(), AppError>;
    async fn list_canonical_klines(
        &self,
        query: CanonicalKlineQuery,
    ) -> Result<Vec<CanonicalKlineRow>, AppError>;
}

#[derive(Debug, Default)]
pub struct InMemoryCanonicalKlineRepository {
    rows: Mutex<HashMap<CanonicalKey, CanonicalKlineRow>>,
}

impl InMemoryCanonicalKlineRepository {
    pub fn rows(&self) -> Vec<CanonicalKlineRow> {
        let mut rows = self.lock_rows().values().cloned().collect::<Vec<_>>();
        rows.sort_by_key(|row| (row.instrument_id, row.timeframe.as_str(), row.open_time));
        rows
    }

    fn lock_rows(&self) -> MutexGuard<'_, HashMap<CanonicalKey, CanonicalKlineRow>> {
        self.rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl CanonicalKlineRepository for InMemoryCanonicalKlineRepository {
    async fn upsert_canonical_kline(&self, row: CanonicalKlineRow) -> Result<(), AppError> {
        let key = (row.instrument_id, row.timeframe, row.open_time);
        self.lock_rows().insert(key, row);
        Ok(())
    }

    async fn list_canonical_klines(
        &self,
        query: CanonicalKlineQuery,
    ) -> Result<Vec<CanonicalKlineRow>, AppError> {
        let mut rows = self
            .lock_rows()
            .values()
            .filter(|row| row.instrument_id == query.instrument_id)
            .filter(|row| row.timeframe == query.timeframe)
            .filter(|row| {
                query
                    .start_open_time
                    .is_none_or(|start_open_time| row.open_time >= start_open_time)
            })
            .filter(|row| {
                query
                    .end_open_time
                    .is_none_or(|end_open_time| row.open_time <= end_open_time)
            })
            .cloned()
            .collect::<Vec<_>>();

        if query.descending {
            rows.sort_by(|left, right| right.open_time.cmp(&left.open_time));
        } else {
            rows.sort_by(|left, right| left.open_time.cmp(&right.open_time));
        }

        rows.truncate(query.limit);
        Ok(rows)
    }
}

#[derive(Debug, Clone)]
pub struct PgCanonicalKlineRepository {
    pool: PgPool,
}

impl PgCanonicalKlineRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl CanonicalKlineRepository for PgCanonicalKlineRepository {
    async fn upsert_canonical_kline(&self, row: CanonicalKlineRow) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO canonical_klines (
                id,
                instrument_id,
                provider,
                timeframe,
                open_time,
                close_time,
                open,
                high,
                low,
                close,
                volume
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (instrument_id, timeframe, open_time)
            DO UPDATE SET
                provider = EXCLUDED.provider,
                close_time = EXCLUDED.close_time,
                open = EXCLUDED.open,
                high = EXCLUDED.high,
                low = EXCLUDED.low,
                close = EXCLUDED.close,
                volume = EXCLUDED.volume
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(row.instrument_id)
        .bind(row.source_provider)
        .bind(row.timeframe.as_str())
        .bind(row.open_time)
        .bind(row.close_time)
        .bind(row.open)
        .bind(row.high)
        .bind(row.low)
        .bind(row.close)
        .bind(row.volume)
        .execute(&self.pool)
        .await
        .map_err(|source| AppError::Storage {
            message: "failed to upsert canonical kline".into(),
            source: Some(Box::new(source)),
        })?;

        Ok(())
    }

    async fn list_canonical_klines(
        &self,
        query: CanonicalKlineQuery,
    ) -> Result<Vec<CanonicalKlineRow>, AppError> {
        let mut builder = QueryBuilder::new(
            r#"
            SELECT
                instrument_id,
                timeframe,
                open_time,
                close_time,
                open,
                high,
                low,
                close,
                volume,
                provider
            FROM canonical_klines
            WHERE instrument_id =
            "#,
        );
        builder.push_bind(query.instrument_id);
        builder.push(" AND timeframe = ");
        builder.push_bind(query.timeframe.as_str());

        if let Some(start_open_time) = query.start_open_time {
            builder.push(" AND open_time >= ");
            builder.push_bind(start_open_time);
        }

        if let Some(end_open_time) = query.end_open_time {
            builder.push(" AND open_time <= ");
            builder.push_bind(end_open_time);
        }

        if query.descending {
            builder.push(" ORDER BY open_time DESC");
        } else {
            builder.push(" ORDER BY open_time ASC");
        }

        builder.push(" LIMIT ");
        builder.push_bind(query.limit as i64);

        let rows = builder
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|source| AppError::Storage {
                message: "failed to list canonical klines".into(),
                source: Some(Box::new(source)),
            })?;

        rows.into_iter()
            .map(|row| {
                let timeframe =
                    row.try_get::<String, _>("timeframe")
                        .map_err(|source| AppError::Storage {
                            message: "failed to decode canonical kline timeframe".into(),
                            source: Some(Box::new(source)),
                        })?;

                Ok(CanonicalKlineRow {
                    instrument_id: row.try_get("instrument_id").map_err(storage_decode_error)?,
                    timeframe: timeframe.parse()?,
                    open_time: row.try_get("open_time").map_err(storage_decode_error)?,
                    close_time: row.try_get("close_time").map_err(storage_decode_error)?,
                    open: row.try_get("open").map_err(storage_decode_error)?,
                    high: row.try_get("high").map_err(storage_decode_error)?,
                    low: row.try_get("low").map_err(storage_decode_error)?,
                    close: row.try_get("close").map_err(storage_decode_error)?,
                    volume: row.try_get("volume").map_err(storage_decode_error)?,
                    source_provider: row.try_get("provider").map_err(storage_decode_error)?,
                })
            })
            .collect()
    }
}

fn storage_decode_error(source: sqlx::Error) -> AppError {
    AppError::Storage {
        message: "failed to decode canonical kline row".into(),
        source: Some(Box::new(source)),
    }
}
