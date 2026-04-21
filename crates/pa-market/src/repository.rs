use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use rust_decimal::Decimal;
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

type CanonicalKey = (Uuid, Timeframe, DateTime<Utc>);

#[async_trait]
pub trait CanonicalKlineRepository: Send + Sync {
    async fn upsert_canonical_kline(&self, row: CanonicalKlineRow) -> Result<(), AppError>;
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
}
