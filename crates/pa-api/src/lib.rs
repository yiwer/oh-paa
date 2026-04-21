#![forbid(unsafe_code)]

pub mod admin;
pub mod analysis;
mod analysis_runtime;
mod error;
pub mod market;
pub mod router;
pub mod user;

pub use router::{AppState, MarketRuntime, app_router};
