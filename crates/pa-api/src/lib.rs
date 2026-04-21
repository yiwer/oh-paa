#![forbid(unsafe_code)]

pub mod admin;
pub mod analysis;
mod error;
pub mod market;
pub mod router;
pub mod user;

pub use router::{AppState, app_router};
