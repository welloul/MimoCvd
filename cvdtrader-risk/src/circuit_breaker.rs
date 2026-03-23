use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    /// Normal operation
    Normal,
    /// Tripped (halted)
    Tripped,
    /// Resetting
    Resetting,
}

/// Circuit breaker for detecting high latency and consecutive failures
pub struct CircuitBreaker {
    /// Current state
    state: Arc<RwLock<CircuitBreakerState>>,
    /// Maximum latency in milliseconds
    max_latency_ms: u64,
    /// Maximum consecutive failures
    max_failures: u32,
    /// Current consecutive failure count
    failure_count: Arc<RwLock<u32>>,
    /// Last latency measurement
    last_latency: Arc<RwLock<Option<u64>>>,
    /// Last failure timestamp
    last_failure: Arc<RwLock<Option<Instant>>>,
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(
        max_latency_ms: u64,
        max_failures: u32,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState::Normal)),
            max_latency_ms,
            max_failures,
            failure_count: Arc::new(RwLock::new(0)),
            last_latency: Arc::new(RwLock::new(None)),
            last_failure: Arc::new(RwLock::new(None)),
            shutdown_tx,
        }
    }

    /// Check if circuit breaker is tripped
    pub async fn is_tripped(&self) -> bool {
        *self.state.read().await == CircuitBreakerState::Tripped
    }

    /// Get current state
    pub async fn state(&self) -> CircuitBreakerState {
        self.state.read().await.clone()
    }

    /// Record latency measurement
    pub async fn record_latency(&self, latency_ms: u64) {
        *self.last_latency.write().await = Some(latency_ms);

        if latency_ms > self.max_latency_ms {
            warn!(
                "High latency detected: {}ms (max: {}ms)",
                latency_ms, self.max_latency_ms
            );
            self.trip().await;
        }
    }

    /// Record a failure
    pub async fn record_failure(&self) {
        let mut failure_count = self.failure_count.write().await;
        *failure_count += 1;
        *self.last_failure.write().await = Some(Instant::now());

        if *failure_count >= self.max_failures {
            error!(
                "Consecutive failures detected: {} (max: {})",
                *failure_count, self.max_failures
            );
            self.trip().await;
        }
    }

    /// Record a success (reset failure count)
    pub async fn record_success(&self) {
        *self.failure_count.write().await = 0;
    }

    /// Trip the circuit breaker
    async fn trip(&self) {
        let mut state = self.state.write().await;
        if *state != CircuitBreakerState::Tripped {
            *state = CircuitBreakerState::Tripped;
            error!("Circuit breaker tripped!");

            // Send shutdown signal
            if let Err(e) = self.shutdown_tx.send(()) {
                error!("Failed to send shutdown signal: {}", e);
            }
        }
    }

    /// Reset the circuit breaker
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Normal;
        *self.failure_count.write().await = 0;
        *self.last_failure.write().await = None;
        info!("Circuit breaker reset");
    }

    /// Get last latency measurement
    pub async fn last_latency(&self) -> Option<u64> {
        *self.last_latency.read().await
    }

    /// Get failure count
    pub async fn failure_count(&self) -> u32 {
        *self.failure_count.read().await
    }

    /// Get maximum latency
    pub fn max_latency_ms(&self) -> u64 {
        self.max_latency_ms
    }

    /// Get maximum failures
    pub fn max_failures(&self) -> u32 {
        self.max_failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_normal() {
        let (shutdown_tx, _) = broadcast::channel(1);
        let breaker = CircuitBreaker::new(500, 3, shutdown_tx);

        assert!(!breaker.is_tripped().await);
        assert_eq!(breaker.state().await, CircuitBreakerState::Normal);
    }

    #[tokio::test]
    async fn test_circuit_breaker_high_latency() {
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        let breaker = CircuitBreaker::new(500, 3, shutdown_tx);

        breaker.record_latency(600).await;
        assert!(breaker.is_tripped().await);

        // Check shutdown signal was sent
        assert!(shutdown_rx.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_consecutive_failures() {
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        let breaker = CircuitBreaker::new(500, 3, shutdown_tx);

        breaker.record_failure().await;
        assert!(!breaker.is_tripped().await);

        breaker.record_failure().await;
        assert!(!breaker.is_tripped().await);

        breaker.record_failure().await;
        assert!(breaker.is_tripped().await);

        // Check shutdown signal was sent
        assert!(shutdown_rx.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let (shutdown_tx, _) = broadcast::channel(1);
        let breaker = CircuitBreaker::new(500, 3, shutdown_tx);

        breaker.record_failure().await;
        breaker.record_failure().await;
        breaker.record_failure().await;
        assert!(breaker.is_tripped().await);

        breaker.reset().await;
        assert!(!breaker.is_tripped().await);
        assert_eq!(breaker.failure_count().await, 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_success_resets_failures() {
        let (shutdown_tx, _) = broadcast::channel(1);
        let breaker = CircuitBreaker::new(500, 3, shutdown_tx);

        breaker.record_failure().await;
        breaker.record_failure().await;
        assert_eq!(breaker.failure_count().await, 2);

        breaker.record_success().await;
        assert_eq!(breaker.failure_count().await, 0);
    }
}
