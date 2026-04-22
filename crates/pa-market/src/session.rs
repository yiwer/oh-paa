use chrono::{
    DateTime, Datelike, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
    Timelike, Utc, Weekday,
};
use pa_core::{AppError, Timeframe};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketSessionKind {
    ContinuousUtc,
    CnA,
    Fx24x5Utc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSessionProfile {
    pub market_code: String,
    pub market_timezone: String,
    pub kind: MarketSessionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBucket {
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub expected_open_times: Vec<DateTime<Utc>>,
}

impl MarketSessionProfile {
    pub fn from_market(market_code: Option<&str>, market_timezone: Option<&str>) -> Self {
        let market_code = market_code.unwrap_or("continuous-utc").to_ascii_lowercase();
        let normalized_code = market_code.replace('_', "-");
        let kind = match normalized_code.as_str() {
            "cn-a" | "ashare" | "a-share" | "a-shares" => MarketSessionKind::CnA,
            "fx" | "forex" => MarketSessionKind::Fx24x5Utc,
            _ => MarketSessionKind::ContinuousUtc,
        };

        Self {
            market_code: normalized_code,
            market_timezone: market_timezone.unwrap_or("UTC").to_string(),
            kind,
        }
    }

    pub fn expected_child_bar_count(
        &self,
        source_timeframe: Timeframe,
        target_timeframe: Timeframe,
    ) -> Result<usize, AppError> {
        Ok(match self.kind {
            MarketSessionKind::CnA => match (source_timeframe, target_timeframe) {
                (Timeframe::M15, Timeframe::H1) => 4,
                (Timeframe::H1, Timeframe::D1) => 4,
                (Timeframe::M15, Timeframe::D1) => 16,
                (same_source, same_target) if same_source == same_target => 1,
                _ => unsupported_aggregation(source_timeframe, target_timeframe)?,
            },
            MarketSessionKind::ContinuousUtc | MarketSessionKind::Fx24x5Utc => {
                match (source_timeframe, target_timeframe) {
                    (Timeframe::M15, Timeframe::H1) => 4,
                    (Timeframe::H1, Timeframe::D1) => 24,
                    (Timeframe::M15, Timeframe::D1) => 96,
                    (same_source, same_target) if same_source == same_target => 1,
                    _ => unsupported_aggregation(source_timeframe, target_timeframe)?,
                }
            }
        })
    }

    pub fn bucket_for_open_time(
        &self,
        source_timeframe: Timeframe,
        target_timeframe: Timeframe,
        open_time: DateTime<Utc>,
    ) -> Result<SessionBucket, AppError> {
        let bucket_open_time = match self.kind {
            MarketSessionKind::CnA => self.cn_a_bucket_open_time(target_timeframe, open_time)?,
            MarketSessionKind::Fx24x5Utc => {
                self.fx_bucket_open_time(target_timeframe, open_time)?
            }
            MarketSessionKind::ContinuousUtc => {
                continuous_bucket_open_time(open_time, target_timeframe)?
            }
        };
        let close_time = self.bucket_close_time(bucket_open_time, target_timeframe)?;
        let expected_open_times =
            self.expected_open_times(source_timeframe, target_timeframe, bucket_open_time)?;

        Ok(SessionBucket {
            open_time: bucket_open_time,
            close_time,
            expected_open_times,
        })
    }

    pub fn accepts_bar_open(&self, timeframe: Timeframe, open_time: DateTime<Utc>) -> bool {
        match self.kind {
            MarketSessionKind::ContinuousUtc => true,
            MarketSessionKind::Fx24x5Utc => match timeframe {
                Timeframe::D1 => {
                    is_fx_market_open(open_time)
                        && open_time.hour() == 22
                        && open_time.minute() == 0
                }
                Timeframe::H1 => is_fx_market_open(open_time) && open_time.minute() == 0,
                Timeframe::M15 => {
                    is_fx_market_open(open_time) && open_time.minute().is_multiple_of(15)
                }
            },
            MarketSessionKind::CnA => accepts_cn_a_bar_open(timeframe, open_time),
        }
    }

    pub fn current_bucket_for_tick(
        &self,
        timeframe: Timeframe,
        tick_time: DateTime<Utc>,
    ) -> Result<Option<SessionBucket>, AppError> {
        if !self.is_market_open(tick_time, timeframe) {
            return Ok(None);
        }

        self.bucket_for_open_time(timeframe, timeframe, tick_time)
            .map(Some)
    }

    pub fn is_market_open(&self, tick_time: DateTime<Utc>, timeframe: Timeframe) -> bool {
        match self.kind {
            MarketSessionKind::ContinuousUtc => true,
            MarketSessionKind::Fx24x5Utc => is_fx_market_open(tick_time),
            MarketSessionKind::CnA => is_cn_a_market_open(tick_time, timeframe),
        }
    }

    fn expected_open_times(
        &self,
        source_timeframe: Timeframe,
        target_timeframe: Timeframe,
        bucket_open_time: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, AppError> {
        match self.kind {
            MarketSessionKind::CnA => {
                cn_a_expected_open_times(source_timeframe, target_timeframe, bucket_open_time)
            }
            MarketSessionKind::ContinuousUtc | MarketSessionKind::Fx24x5Utc => {
                let expected_count =
                    self.expected_child_bar_count(source_timeframe, target_timeframe)?;
                let duration = duration_from_timeframe(source_timeframe)?;
                Ok((0..expected_count)
                    .map(|offset| {
                        bucket_open_time + (duration * i32::try_from(offset).unwrap_or_default())
                    })
                    .collect())
            }
        }
    }

    fn bucket_close_time(
        &self,
        bucket_open_time: DateTime<Utc>,
        timeframe: Timeframe,
    ) -> Result<DateTime<Utc>, AppError> {
        match self.kind {
            MarketSessionKind::CnA if timeframe == Timeframe::D1 => {
                let local = bucket_open_time.with_timezone(&cn_a_offset()?);
                Ok(local_datetime(local.date_naive(), hms(15, 0, 0)?)?.with_timezone(&Utc))
            }
            _ => bucket_open_time
                .checked_add_signed(duration_from_timeframe(timeframe)?)
                .ok_or_else(|| AppError::Validation {
                    message: format!(
                        "failed to compute close time for {} bucket at {}",
                        timeframe,
                        bucket_open_time.to_rfc3339()
                    ),
                    source: None,
                }),
        }
    }

    fn cn_a_bucket_open_time(
        &self,
        target_timeframe: Timeframe,
        open_time: DateTime<Utc>,
    ) -> Result<DateTime<Utc>, AppError> {
        let local = open_time.with_timezone(&cn_a_offset()?);
        let date = local.date_naive();
        let clock = local.time();

        let bucket_time = match target_timeframe {
            Timeframe::M15 => {
                if !is_cn_a_market_open(open_time, Timeframe::M15) {
                    return Err(AppError::Validation {
                        message: format!(
                            "timestamp {} is outside cn-a trading sessions",
                            open_time.to_rfc3339()
                        ),
                        source: None,
                    });
                }

                hms(clock.hour(), (clock.minute() / 15) * 15, 0)?
            }
            Timeframe::H1 => {
                if within_range(clock, hms(9, 30, 0)?, hms(10, 30, 0)?) {
                    hms(9, 30, 0)?
                } else if within_range(clock, hms(10, 30, 0)?, hms(11, 30, 0)?) {
                    hms(10, 30, 0)?
                } else if within_range(clock, hms(13, 0, 0)?, hms(14, 0, 0)?) {
                    hms(13, 0, 0)?
                } else if within_range(clock, hms(14, 0, 0)?, hms(15, 0, 0)?) {
                    hms(14, 0, 0)?
                } else {
                    return Err(AppError::Validation {
                        message: format!(
                            "timestamp {} is outside cn-a trading sessions",
                            open_time.to_rfc3339()
                        ),
                        source: None,
                    });
                }
            }
            Timeframe::D1 => hms(9, 30, 0)?,
        };

        Ok(local_datetime(date, bucket_time)?.with_timezone(&Utc))
    }

    fn fx_bucket_open_time(
        &self,
        target_timeframe: Timeframe,
        open_time: DateTime<Utc>,
    ) -> Result<DateTime<Utc>, AppError> {
        if target_timeframe != Timeframe::D1 {
            return continuous_bucket_open_time(open_time, target_timeframe);
        }

        let shifted = open_time - Duration::hours(22);
        day_start_utc(shifted)?
            .checked_add_signed(Duration::hours(22))
            .ok_or_else(|| AppError::Validation {
                message: format!(
                    "failed to build fx daily bucket for {}",
                    open_time.to_rfc3339()
                ),
                source: None,
            })
    }
}

fn cn_a_expected_open_times(
    source_timeframe: Timeframe,
    target_timeframe: Timeframe,
    bucket_open_time: DateTime<Utc>,
) -> Result<Vec<DateTime<Utc>>, AppError> {
    let local_bucket = bucket_open_time.with_timezone(&cn_a_offset()?);
    let date = local_bucket.date_naive();

    match (source_timeframe, target_timeframe) {
        (Timeframe::M15, Timeframe::H1) => Ok((0..4)
            .map(|offset| bucket_open_time + Duration::minutes(15 * i64::from(offset)))
            .collect()),
        (Timeframe::M15, Timeframe::D1) => [
            (9, 30),
            (9, 45),
            (10, 0),
            (10, 15),
            (10, 30),
            (10, 45),
            (11, 0),
            (11, 15),
            (13, 0),
            (13, 15),
            (13, 30),
            (13, 45),
            (14, 0),
            (14, 15),
            (14, 30),
            (14, 45),
        ]
        .into_iter()
        .map(|(hour, minute)| {
            local_datetime(date, hms(hour, minute, 0)?).map(|value| value.with_timezone(&Utc))
        })
        .collect(),
        (Timeframe::H1, Timeframe::D1) => [(9, 30), (10, 30), (13, 0), (14, 0)]
            .into_iter()
            .map(|(hour, minute)| {
                local_datetime(date, hms(hour, minute, 0)?).map(|value| value.with_timezone(&Utc))
            })
            .collect(),
        (same_source, same_target) if same_source == same_target => Ok(vec![bucket_open_time]),
        _ => unsupported_aggregation(source_timeframe, target_timeframe),
    }
}

fn continuous_bucket_open_time(
    open_time: DateTime<Utc>,
    timeframe: Timeframe,
) -> Result<DateTime<Utc>, AppError> {
    if timeframe == Timeframe::D1 {
        return day_start_utc(open_time);
    }

    let duration = duration_from_timeframe(timeframe)?;
    let timestamp = open_time.timestamp();
    let bucket_timestamp = timestamp - timestamp.rem_euclid(duration.num_seconds());
    DateTime::<Utc>::from_timestamp(bucket_timestamp, 0).ok_or_else(|| AppError::Validation {
        message: format!(
            "failed to build continuous bucket for {}",
            open_time.to_rfc3339()
        ),
        source: None,
    })
}

fn day_start_utc(open_time: DateTime<Utc>) -> Result<DateTime<Utc>, AppError> {
    let date = open_time.date_naive();
    let datetime = NaiveDateTime::new(date, hms(0, 0, 0)?);
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc))
}

fn duration_from_timeframe(timeframe: Timeframe) -> Result<Duration, AppError> {
    Duration::from_std(timeframe.duration()).map_err(|source| AppError::Validation {
        message: format!("failed to convert timeframe duration for {}", timeframe),
        source: Some(Box::new(source)),
    })
}

fn unsupported_aggregation<T>(
    source_timeframe: Timeframe,
    target_timeframe: Timeframe,
) -> Result<T, AppError> {
    Err(AppError::Validation {
        message: format!(
            "unsupported aggregation from {} to {}",
            source_timeframe, target_timeframe
        ),
        source: None,
    })
}

fn is_cn_a_market_open(tick_time: DateTime<Utc>, timeframe: Timeframe) -> bool {
    let offset = match cn_a_offset() {
        Ok(offset) => offset,
        Err(_) => return false,
    };
    let morning_open = match hms(9, 30, 0) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let morning_close = match hms(11, 30, 0) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let afternoon_open = match hms(13, 0, 0) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let day_close = match hms(15, 0, 0) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let local = tick_time.with_timezone(&offset);
    let clock = local.time();

    match timeframe {
        Timeframe::D1 => within_range(clock, morning_open, day_close),
        Timeframe::M15 | Timeframe::H1 => {
            within_range(clock, morning_open, morning_close)
                || within_range(clock, afternoon_open, day_close)
        }
    }
}

fn accepts_cn_a_bar_open(timeframe: Timeframe, open_time: DateTime<Utc>) -> bool {
    let offset = match cn_a_offset() {
        Ok(offset) => offset,
        Err(_) => return false,
    };
    let local = open_time.with_timezone(&offset);
    let clock = local.time();

    match timeframe {
        Timeframe::M15 => matches!(
            (clock.hour(), clock.minute()),
            (9, 30)
                | (9, 45)
                | (10, 0)
                | (10, 15)
                | (10, 30)
                | (10, 45)
                | (11, 0)
                | (11, 15)
                | (13, 0)
                | (13, 15)
                | (13, 30)
                | (13, 45)
                | (14, 0)
                | (14, 15)
                | (14, 30)
                | (14, 45)
        ),
        Timeframe::H1 => matches!(
            (clock.hour(), clock.minute()),
            (9, 30) | (10, 30) | (13, 0) | (14, 0)
        ),
        Timeframe::D1 => matches!((clock.hour(), clock.minute()), (9, 30)),
    }
}

fn is_fx_market_open(tick_time: DateTime<Utc>) -> bool {
    match tick_time.weekday() {
        Weekday::Sat => false,
        Weekday::Sun => tick_time.hour() >= 22,
        Weekday::Fri => tick_time.hour() < 22,
        _ => true,
    }
}

fn within_range(value: NaiveTime, start: NaiveTime, end: NaiveTime) -> bool {
    value >= start && value < end
}

fn cn_a_offset() -> Result<FixedOffset, AppError> {
    FixedOffset::east_opt(8 * 60 * 60).ok_or_else(|| AppError::Validation {
        message: "failed to construct cn-a timezone offset".into(),
        source: None,
    })
}

fn local_datetime(date: NaiveDate, time: NaiveTime) -> Result<DateTime<FixedOffset>, AppError> {
    cn_a_offset()?
        .from_local_datetime(&NaiveDateTime::new(date, time))
        .single()
        .ok_or_else(|| AppError::Validation {
            message: format!("failed to construct local datetime for {date} {time}"),
            source: None,
        })
}

fn hms(hour: u32, minute: u32, second: u32) -> Result<NaiveTime, AppError> {
    NaiveTime::from_hms_opt(hour, minute, second).ok_or_else(|| AppError::Validation {
        message: format!("invalid time components: {hour}:{minute}:{second}"),
        source: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{MarketSessionKind, MarketSessionProfile};
    use chrono::{DateTime, Utc};
    use pa_core::Timeframe;

    fn utc(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("fixture timestamp should be valid")
            .with_timezone(&Utc)
    }

    #[test]
    fn resolves_known_market_profiles() {
        assert_eq!(
            MarketSessionProfile::from_market(Some("cn-a"), Some("Asia/Shanghai")).kind,
            MarketSessionKind::CnA
        );
        assert_eq!(
            MarketSessionProfile::from_market(Some("forex"), Some("UTC")).kind,
            MarketSessionKind::Fx24x5Utc
        );
        assert_eq!(
            MarketSessionProfile::from_market(Some("custom-market"), Some("UTC")).kind,
            MarketSessionKind::ContinuousUtc
        );
    }

    #[test]
    fn cn_a_daily_bucket_uses_local_trading_day_anchor() {
        let profile = MarketSessionProfile::from_market(Some("cn-a"), Some("Asia/Shanghai"));
        let bucket = profile
            .bucket_for_open_time(Timeframe::M15, Timeframe::D1, utc("2024-01-02T05:15:00Z"))
            .expect("bucket should resolve");

        assert_eq!(bucket.open_time, utc("2024-01-02T01:30:00Z"));
        assert_eq!(bucket.close_time, utc("2024-01-02T07:00:00Z"));
        assert_eq!(bucket.expected_open_times.len(), 16);
    }

    #[test]
    fn fx_market_profile_marks_weekend_as_closed() {
        let profile = MarketSessionProfile::from_market(Some("fx"), Some("UTC"));

        assert!(profile.is_market_open(utc("2024-01-04T12:00:00Z"), Timeframe::H1));
        assert!(!profile.is_market_open(utc("2024-01-06T12:00:00Z"), Timeframe::H1));
        assert!(profile.is_market_open(utc("2024-01-07T22:15:00Z"), Timeframe::H1));
    }
}
