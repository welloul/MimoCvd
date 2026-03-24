//! Specific error types for execution operations
//!
//! These error types enable precise error handling and recovery strategies
//! for different failure modes in order execution.

use std::fmt;

/// Errors that can occur during order execution
#[derive(Debug, Clone)]
pub enum ExecutionError {
    /// Network connectivity issues (retryable)
    Network { message: String, retryable: bool },
    /// Order validation failed (not retryable)
    Validation { message: String },
    /// Exchange API error (may be retryable depending on error code)
    Exchange {
        code: i64,
        message: String,
        retryable: bool,
    },
    /// Request timeout (retryable)
    Timeout { message: String },
    /// Rate limited by exchange (retryable after backoff)
    RateLimited { retry_after_secs: u64 },
    /// Insufficient balance (not retryable)
    InsufficientBalance { required: f64, available: f64 },
    /// Order rejected by exchange (not retryable)
    Rejected { reason: String },
    /// Position not found (not retryable)
    PositionNotFound { symbol: String },
    /// Order not found (not retryable)
    OrderNotFound { order_id: String },
}

impl ExecutionError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            ExecutionError::Network { retryable, .. } => *retryable,
            ExecutionError::Validation { .. } => false,
            ExecutionError::Exchange { retryable, .. } => *retryable,
            ExecutionError::Timeout { .. } => true,
            ExecutionError::RateLimited { .. } => true,
            ExecutionError::InsufficientBalance { .. } => false,
            ExecutionError::Rejected { .. } => false,
            ExecutionError::PositionNotFound { .. } => false,
            ExecutionError::OrderNotFound { .. } => false,
        }
    }

    /// Get retry delay in seconds (if retryable)
    pub fn retry_delay_secs(&self) -> Option<u64> {
        match self {
            ExecutionError::RateLimited { retry_after_secs } => Some(*retry_after_secs),
            ExecutionError::Timeout { .. } => Some(1),
            ExecutionError::Network { .. } => Some(2),
            ExecutionError::Exchange {
                retryable: true, ..
            } => Some(5),
            _ => None,
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionError::Network { message, retryable } => {
                write!(f, "Network error: {} (retryable: {})", message, retryable)
            }
            ExecutionError::Validation { message } => {
                write!(f, "Validation error: {}", message)
            }
            ExecutionError::Exchange {
                code,
                message,
                retryable,
            } => {
                write!(
                    f,
                    "Exchange error {}: {} (retryable: {})",
                    code, message, retryable
                )
            }
            ExecutionError::Timeout { message } => {
                write!(f, "Timeout: {}", message)
            }
            ExecutionError::RateLimited { retry_after_secs } => {
                write!(f, "Rate limited, retry after {} seconds", retry_after_secs)
            }
            ExecutionError::InsufficientBalance {
                required,
                available,
            } => {
                write!(
                    f,
                    "Insufficient balance: required {}, available {}",
                    required, available
                )
            }
            ExecutionError::Rejected { reason } => {
                write!(f, "Order rejected: {}", reason)
            }
            ExecutionError::PositionNotFound { symbol } => {
                write!(f, "Position not found for symbol: {}", symbol)
            }
            ExecutionError::OrderNotFound { order_id } => {
                write!(f, "Order not found: {}", order_id)
            }
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Convert reqwest errors to ExecutionError
impl From<reqwest::Error> for ExecutionError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ExecutionError::Timeout {
                message: err.to_string(),
            }
        } else if err.is_connect() {
            ExecutionError::Network {
                message: err.to_string(),
                retryable: true,
            }
        } else {
            ExecutionError::Network {
                message: err.to_string(),
                retryable: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_errors() {
        let network_err = ExecutionError::Network {
            message: "connection refused".to_string(),
            retryable: true,
        };
        assert!(network_err.is_retryable());
        assert_eq!(network_err.retry_delay_secs(), Some(2));

        let validation_err = ExecutionError::Validation {
            message: "invalid size".to_string(),
        };
        assert!(!validation_err.is_retryable());
        assert_eq!(validation_err.retry_delay_secs(), None);

        let timeout_err = ExecutionError::Timeout {
            message: "request timed out".to_string(),
        };
        assert!(timeout_err.is_retryable());
        assert_eq!(timeout_err.retry_delay_secs(), Some(1));

        let rate_limited = ExecutionError::RateLimited {
            retry_after_secs: 30,
        };
        assert!(rate_limited.is_retryable());
        assert_eq!(rate_limited.retry_delay_secs(), Some(30));
    }

    #[test]
    fn test_error_display() {
        let err = ExecutionError::InsufficientBalance {
            required: 1000.0,
            available: 500.0,
        };
        let display = format!("{}", err);
        assert!(display.contains("1000"));
        assert!(display.contains("500"));
    }
}
