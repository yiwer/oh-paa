use pa_core::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    RetryNow,
    FailTerminal,
    MoveToDeadLetter,
}

pub fn classify_retry(err: &AppError, attempt_count: u32, max_attempts: u32) -> RetryDecision {
    if err.is_retryable() && attempt_count < max_attempts {
        RetryDecision::RetryNow
    } else if err.is_retryable() {
        RetryDecision::MoveToDeadLetter
    } else {
        RetryDecision::FailTerminal
    }
}
