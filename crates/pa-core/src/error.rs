use std::error::Error as StdError;

use thiserror::Error;

type BoxError = Box<dyn StdError + Send + Sync + 'static>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("validation error: {message}")]
    Validation {
        message: String,
        #[source]
        source: Option<BoxError>,
    },
    #[error("provider error: {message}")]
    Provider {
        message: String,
        #[source]
        source: Option<BoxError>,
    },
    #[error("storage error: {message}")]
    Storage {
        message: String,
        #[source]
        source: Option<BoxError>,
    },
    #[error("analysis error: {message}")]
    Analysis {
        message: String,
        #[source]
        source: Option<BoxError>,
    },
}
