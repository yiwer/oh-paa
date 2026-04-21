use std::time::Duration;

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
