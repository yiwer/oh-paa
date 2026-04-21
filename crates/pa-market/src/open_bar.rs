use std::collections::HashMap;

use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use rust_decimal::Decimal;

use crate::models::ProviderTick;

#[derive(Debug, Clone, PartialEq)]
pub struct OpenBar {
    pub open_time: DateTime<Utc>,
    pub latest_tick_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
}

impl OpenBar {
    pub fn new(open_time: DateTime<Utc>, opening_price: Decimal) -> Self {
        Self {
            open_time,
            latest_tick_time: open_time,
            open: opening_price,
            high: opening_price,
            low: opening_price,
            close: opening_price,
        }
    }

    pub fn apply_tick(&mut self, tick: ProviderTick) -> Result<(), AppError> {
        if tick.tick_time < self.open_time {
            return Err(AppError::Validation {
                message: "tick time is before bar open".to_string(),
                source: None,
            });
        }

        if tick.tick_time < self.latest_tick_time {
            return Err(AppError::Validation {
                message: "tick time is older than latest tick".to_string(),
                source: None,
            });
        }

        self.high = self.high.max(tick.price);
        self.low = self.low.min(tick.price);
        self.close = tick.price;
        self.latest_tick_time = tick.tick_time;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct OpenBarBook {
    bars: HashMap<Timeframe, OpenBar>,
}

impl OpenBarBook {
    pub fn start_bar(
        &mut self,
        timeframe: Timeframe,
        open_time: DateTime<Utc>,
        opening_price: Decimal,
    ) -> &OpenBar {
        let bar = self
            .bars
            .entry(timeframe)
            .and_modify(|bar| *bar = OpenBar::new(open_time, opening_price))
            .or_insert_with(|| OpenBar::new(open_time, opening_price));

        &*bar
    }

    pub fn apply_tick(
        &mut self,
        timeframe: Timeframe,
        tick: ProviderTick,
    ) -> Result<&OpenBar, AppError> {
        let bar = self
            .bars
            .get_mut(&timeframe)
            .ok_or_else(|| AppError::Validation {
                message: format!("no open bar started for timeframe {}", timeframe.as_str()),
                source: None,
            })?;

        bar.apply_tick(tick)?;
        Ok(&*bar)
    }

    pub fn current_bar(&self, timeframe: Timeframe) -> Option<&OpenBar> {
        self.bars.get(&timeframe)
    }
}
