use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Strongly typed message ID to prevent confusion with other string types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub String);

impl From<String> for MessageId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for MessageId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::borrow::Borrow<str> for MessageId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// Message tracking for pending responses
#[derive(Debug)]
pub struct PendingMessage {
    pub sent_at: Instant,
    pub timeout_at: Instant,
}

impl PendingMessage {
    /// Create a new pending message with timeout
    #[inline]
    pub fn new(timeout: Duration) -> Self {
        let now = Instant::now();
        Self {
            sent_at: now,
            timeout_at: now + timeout,
        }
    }

    /// Calculate the round-trip time if a response is received
    #[inline]
    pub fn calculate_rtt(&self, response_received_at: Instant) -> Duration {
        response_received_at.duration_since(self.sent_at)
    }
}
