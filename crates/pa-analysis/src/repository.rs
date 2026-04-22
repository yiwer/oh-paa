use std::{
    collections::HashMap,
    collections::hash_map::Entry,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use pa_core::{AppError, Timeframe};
use pa_orchestrator::AnalysisBarState;
use uuid::Uuid;

use crate::models::{BarAnalysis, DailyMarketContext, PaStateBar};

type BarAnalysisKey = (Uuid, Timeframe, DateTime<Utc>, String);
type DailyContextKey = (Uuid, NaiveDate, String);
type PaStateBarKey = (
    Uuid,
    Timeframe,
    String,
    DateTime<Utc>,
    DateTime<Utc>,
    String,
    u64,
);

#[async_trait]
pub trait AnalysisRepository: Send + Sync {
    async fn insert_bar_analysis_if_absent(&self, analysis: BarAnalysis) -> Result<bool, AppError>;

    async fn insert_pa_state_bar_if_absent(
        &self,
        pa_state_bar: PaStateBar,
    ) -> Result<bool, AppError>;

    async fn insert_daily_context_if_absent(
        &self,
        context: DailyMarketContext,
    ) -> Result<bool, AppError>;
}

#[derive(Debug, Default)]
pub struct InMemoryAnalysisRepository {
    bar_analyses: Mutex<HashMap<BarAnalysisKey, BarAnalysis>>,
    pa_state_bars: Mutex<HashMap<PaStateBarKey, PaStateBar>>,
    open_pa_state_bar_insert_seq: AtomicU64,
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

    pub fn pa_state_bars(&self) -> Vec<PaStateBar> {
        let mut rows = self
            .lock_pa_state_bars()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| {
            (
                row.instrument_id,
                row.timeframe.as_str(),
                row.bar_state.as_str(),
                row.bar_open_time,
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

    fn lock_pa_state_bars(&self) -> MutexGuard<'_, HashMap<PaStateBarKey, PaStateBar>> {
        self.pa_state_bars
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

    async fn insert_pa_state_bar_if_absent(
        &self,
        pa_state_bar: PaStateBar,
    ) -> Result<bool, AppError> {
        let identity = (
            pa_state_bar.instrument_id,
            pa_state_bar.timeframe,
            pa_state_bar.bar_state.as_str().to_string(),
            pa_state_bar.bar_open_time,
            pa_state_bar.bar_close_time,
            pa_state_bar.analysis_version.clone(),
        );

        if pa_state_bar.bar_state == AnalysisBarState::Open {
            let key = (
                identity.0,
                identity.1,
                identity.2,
                identity.3,
                identity.4,
                identity.5,
                self.open_pa_state_bar_insert_seq
                    .fetch_add(1, Ordering::Relaxed)
                    + 1,
            );
            self.lock_pa_state_bars().insert(key, pa_state_bar);
            return Ok(true);
        }

        let key = (
            identity.0, identity.1, identity.2, identity.3, identity.4, identity.5, 0,
        );
        match self.lock_pa_state_bars().entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(pa_state_bar);
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
