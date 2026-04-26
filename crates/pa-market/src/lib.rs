#![forbid(unsafe_code)]

pub mod models;
pub mod normalize;
pub mod open_bar;
pub mod provider;
pub mod gateway;
pub mod repository;
pub mod service;
pub mod session;

pub use models::{CanonicalKline, ProviderKline, ProviderTick};
pub use normalize::normalize_kline;
pub use open_bar::{OpenBar, OpenBarBook};
pub use provider::{HistoricalKlineQuery, MarketDataProvider, ProviderRouter};
pub use gateway::MarketGateway;
pub use repository::{
    CanonicalKlineQuery, CanonicalKlineRepository, CanonicalKlineRow,
    InMemoryCanonicalKlineRepository, PgCanonicalKlineRepository,
};
pub use service::{
    AggregateCanonicalKlinesRequest, AggregatedKline,
    DerivedOpenBar, aggregate_canonical_klines, aggregate_replay_window_rows,
    backfill_canonical_klines, derive_open_bar, list_canonical_klines,
};
pub use session::{MarketSessionKind, MarketSessionProfile, SessionBucket};
