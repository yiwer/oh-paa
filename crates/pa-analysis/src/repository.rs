use std::{
    collections::HashMap,
    collections::hash_map::Entry,
    sync::{Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use pa_core::{AppError, Timeframe};
use uuid::Uuid;

use crate::models::{BarAnalysis, DailyMarketContext};

type BarAnalysisKey = (Uuid, Timeframe, DateTime<Utc>, String);
type DailyContextKey = (Uuid, NaiveDate, String);

#[async_trait]
pub trait AnalysisRepository: Send + Sync {
    async fn insert_bar_analysis_if_absent(&self, analysis: BarAnalysis) -> Result<bool, AppError>;

    async fn insert_daily_context_if_absent(
        &self,
        context: DailyMarketContext,
    ) -> Result<bool, AppError>;
}

#[derive(Debug, Default)]
pub struct InMemoryAnalysisRepository {
    bar_analyses: Mutex<HashMap<BarAnalysisKey, BarAnalysis>>,
    daily_contexts: Mutex<HashMap<DailyContextKey, DailyMarketContext>>,
}

impl InMemoryAnalysisRepository {
    pub fn bar_analyses(&self) -> Vec<BarAnalysis> {
        let mut rows = self
            .lock_bar_analyses()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| {
            (
                row.instrument_id,
                row.timeframe.as_str(),
                row.bar_close_time,
                row.analysis_version.clone(),
            )
        });
        rows
    }

    pub fn daily_contexts(&self) -> Vec<DailyMarketContext> {
        let mut rows = self
            .lock_daily_contexts()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| {
            (
                row.instrument_id,
                row.trading_date,
                row.analysis_version.clone(),
            )
        });
        rows
    }

    fn lock_bar_analyses(&self) -> MutexGuard<'_, HashMap<BarAnalysisKey, BarAnalysis>> {
        self.bar_analyses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_daily_contexts(&self) -> MutexGuard<'_, HashMap<DailyContextKey, DailyMarketContext>> {
        self.daily_contexts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl AnalysisRepository for InMemoryAnalysisRepository {
    async fn insert_bar_analysis_if_absent(&self, analysis: BarAnalysis) -> Result<bool, AppError> {
        let key = (
            analysis.instrument_id,
            analysis.timeframe,
            analysis.bar_close_time,
            analysis.analysis_version.clone(),
        );

        match self.lock_bar_analyses().entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(analysis);
                Ok(true)
            }
            Entry::Occupied(_) => Ok(false),
        }
    }

    async fn insert_daily_context_if_absent(
        &self,
        context: DailyMarketContext,
    ) -> Result<bool, AppError> {
        let key = (
            context.instrument_id,
            context.trading_date,
            context.analysis_version.clone(),
        );

        match self.lock_daily_contexts().entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(context);
                Ok(true)
            }
            Entry::Occupied(_) => Ok(false),
        }
    }
}
