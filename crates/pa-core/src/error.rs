use std::{error::Error as StdError, io::ErrorKind};

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

impl AppError {
    pub fn is_retryable(&self) -> bool {
        match self {
            AppError::Provider { message, source } | AppError::Storage { message, source } => {
                is_transient_message(message)
                    || source
                        .as_deref()
                        .is_some_and(|source| is_transient_error_source(source))
            }
            _ => false,
        }
    }
}

fn is_transient_error_source(source: &(dyn StdError + 'static)) -> bool {
    let mut current = Some(source);
    while let Some(err) = current {
        if let Some(io_error) = err.downcast_ref::<std::io::Error>() {
            match io_error.kind() {
                ErrorKind::Interrupted
                | ErrorKind::TimedOut
                | ErrorKind::WouldBlock
                | ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::NotConnected
                | ErrorKind::BrokenPipe
                | ErrorKind::AddrInUse
                | ErrorKind::AddrNotAvailable
                | ErrorKind::NetworkDown
                | ErrorKind::NetworkUnreachable
                | ErrorKind::HostUnreachable => return true,
                _ => {}
            }
        }
        current = err.source();
    }

    false
}

fn is_transient_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    [
        "timeout",
        "timed out",
        "temporar",
        "rate limit",
        "too many requests",
        "connection refused",
        "connection reset",
        "network unreachable",
        "service unavailable",
        "try again",
        "deadlock",
    ]
    .iter()
    .any(|token| normalized.contains(token))
}
