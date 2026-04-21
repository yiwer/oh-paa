use std::{fmt, str::FromStr, time::Duration};

use crate::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timeframe {
    M15,
    H1,
    D1,
}

impl Timeframe {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::D1 => "1d",
        }
    }

    pub const fn duration(self) -> Duration {
        match self {
            Self::M15 => Duration::from_secs(15 * 60),
            Self::H1 => Duration::from_secs(60 * 60),
            Self::D1 => Duration::from_secs(24 * 60 * 60),
        }
    }
}

impl FromStr for Timeframe {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "15m" => Ok(Self::M15),
            "1h" => Ok(Self::H1),
            "1d" => Ok(Self::D1),
            other => Err(AppError::Validation {
                message: format!("invalid timeframe: {other}"),
                source: None,
            }),
        }
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
