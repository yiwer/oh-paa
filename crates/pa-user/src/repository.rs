use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use pa_analysis::{BarAnalysis, DailyMarketContext};
use pa_core::{AppError, Timeframe};
use uuid::Uuid;

use crate::models::{PositionSnapshot, UserSubscription};

type BarAnalysisKey = (Uuid, Timeframe, DateTime<Utc>, String);
type DailyContextKey = (Uuid, NaiveDate, String);

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn list_user_subscriptions(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<UserSubscription>, AppError>;

    async fn list_user_positions(
        &self,
        user_id: Uuid,
        instrument_id: Uuid,
    ) -> Result<Vec<PositionSnapshot>, AppError>;
}

#[async_trait]
pub trait SharedAnalysisLookup: Send + Sync {
    async fn get_bar_analysis(
        &self,
        instrument_id: Uuid,
        timeframe: Timeframe,
        bar_close_time: DateTime<Utc>,
        analysis_version: &str,
    ) -> Result<Option<BarAnalysis>, AppError>;

    async fn get_daily_market_context(
        &self,
        instrument_id: Uuid,
        trading_date: NaiveDate,
        analysis_version: &str,
    ) -> Result<Option<DailyMarketContext>, AppError>;
}

#[derive(Debug, Default)]
pub struct InMemoryUserRepository {
    subscriptions: Mutex<Vec<UserSubscription>>,
    positions: Mutex<Vec<PositionSnapshot>>,
}

impl InMemoryUserRepository {
    pub fn new(subscriptions: Vec<UserSubscription>, positions: Vec<PositionSnapshot>) -> Self {
        Self {
            subscriptions: Mutex::new(subscriptions),
            positions: Mutex::new(positions),
        }
    }

    fn lock_subscriptions(&self) -> MutexGuard<'_, Vec<UserSubscription>> {
        self.subscriptions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_positions(&self) -> MutexGuard<'_, Vec<PositionSnapshot>> {
        self.positions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn list_user_subscriptions(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<UserSubscription>, AppError> {
        Ok(self
            .lock_subscriptions()
            .iter()
            .filter(|subscription| subscription.user_id == user_id)
            .cloned()
            .collect())
    }

    async fn list_user_positions(
        &self,
        user_id: Uuid,
        instrument_id: Uuid,
    ) -> Result<Vec<PositionSnapshot>, AppError> {
        Ok(self
            .lock_positions()
            .iter()
            .filter(|position| {
                position.user_id == user_id && position.instrument_id == instrument_id
            })
            .cloned()
            .collect())
    }
}

#[derive(Debug, Default)]
pub struct InMemorySharedAnalysisLookup {
    bar_analyses: Mutex<HashMap<BarAnalysisKey, BarAnalysis>>,
    daily_contexts: Mutex<HashMap<DailyContextKey, DailyMarketContext>>,
}

impl InMemorySharedAnalysisLookup {
    pub fn new(bar_analyses: Vec<BarAnalysis>, daily_contexts: Vec<DailyMarketContext>) -> Self {
        let bar_analyses = bar_analyses
            .into_iter()
            .map(|analysis| {
                (
                    (
                        analysis.instrument_id,
                        analysis.timeframe,
                        analysis.bar_close_time,
                        analysis.analysis_version.clone(),
                    ),
                    analysis,
                )
            })
            .collect();
        let daily_contexts = daily_contexts
            .into_iter()
            .map(|context| {
                (
                    (
                        context.instrument_id,
                        context.trading_date,
                        context.analysis_version.clone(),
                    ),
                    context,
                )
            })
            .collect();

        Self {
            bar_analyses: Mutex::new(bar_analyses),
            daily_contexts: Mutex::new(daily_contexts),
        }
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
impl SharedAnalysisLookup for InMemorySharedAnalysisLookup {
    async fn get_bar_analysis(
        &self,
        instrument_id: Uuid,
        timeframe: Timeframe,
        bar_close_time: DateTime<Utc>,
        analysis_version: &str,
    ) -> Result<Option<BarAnalysis>, AppError> {
        Ok(self
            .lock_bar_analyses()
            .get(&(
                instrument_id,
                timeframe,
                bar_close_time,
                analysis_version.to_string(),
            ))
            .cloned())
    }

    async fn get_daily_market_context(
        &self,
        instrument_id: Uuid,
        trading_date: NaiveDate,
        analysis_version: &str,
    ) -> Result<Option<DailyMarketContext>, AppError> {
        Ok(self
            .lock_daily_contexts()
            .get(&(instrument_id, trading_date, analysis_version.to_string()))
            .cloned())
    }
}
