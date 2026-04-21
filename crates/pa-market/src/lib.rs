#![forbid(unsafe_code)]

pub mod models;
pub mod normalize;
pub mod open_bar;
pub mod provider;
pub mod repository;
pub mod service;

pub use models::{CanonicalKline, ProviderKline, ProviderTick};
pub use normalize::normalize_kline;
pub use open_bar::{OpenBar, OpenBarBook};
pub use provider::{MarketDataProvider, ProviderRouter};
pub use repository::{
    CanonicalKlineRepository, CanonicalKlineRow, InMemoryCanonicalKlineRepository,
};
pub use service::{BackfillCanonicalKlinesRequest, backfill_canonical_klines};
