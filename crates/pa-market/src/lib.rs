#![forbid(unsafe_code)]

pub mod models;
pub mod normalize;
pub mod open_bar;
pub mod provider;
pub mod repository;
pub mod session;
pub mod service;

pub use models::{CanonicalKline, ProviderKline, ProviderTick};
pub use normalize::normalize_kline;
pub use open_bar::{OpenBar, OpenBarBook};
pub use provider::{MarketDataProvider, ProviderRouter};
pub use repository::{
    CanonicalKlineQuery, CanonicalKlineRepository, CanonicalKlineRow,
    InMemoryCanonicalKlineRepository, PgCanonicalKlineRepository,
};
pub use session::{MarketSessionKind, MarketSessionProfile, SessionBucket};
pub use service::{
    AggregateCanonicalKlinesRequest, AggregatedKline, BackfillCanonicalKlinesRequest,
    DeriveOpenBarRequest, DerivedOpenBar, aggregate_canonical_klines, backfill_canonical_klines,
    derive_open_bar, list_canonical_klines,
};
