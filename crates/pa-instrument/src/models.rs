use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Market {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub timezone: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instrument {
    pub id: Uuid,
    pub market_id: Uuid,
    pub symbol: String,
    pub name: String,
    pub instrument_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSymbolBinding {
    pub id: Uuid,
    pub instrument_id: Uuid,
    pub provider: String,
    pub provider_symbol: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyScope {
    /// Immutable market ID stored as a UUID string.
    Market(String),
    /// Immutable instrument ID stored as a UUID string.
    Instrument(String),
}

impl PolicyScope {
    pub fn scope_id(&self) -> &str {
        match self {
            Self::Market(scope_id) | Self::Instrument(scope_id) => scope_id,
        }
    }

    pub const fn scope_type(&self) -> &'static str {
        match self {
            Self::Market(_) => "market",
            Self::Instrument(_) => "instrument",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPolicy {
    pub scope: PolicyScope,
    pub kline_primary: String,
    pub kline_fallback: Option<String>,
    pub tick_primary: String,
    pub tick_fallback: Option<String>,
}

impl ProviderPolicy {
    pub fn new(
        scope: PolicyScope,
        kline_primary: String,
        kline_fallback: Option<String>,
        tick_primary: String,
        tick_fallback: Option<String>,
    ) -> Self {
        Self {
            scope,
            kline_primary,
            kline_fallback,
            tick_primary,
            tick_fallback,
        }
    }
}
