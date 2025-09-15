//! Individual client metrics collection

use crate::common::ClientId;
use std::time::Duration;

/// Individual client metrics
#[derive(Debug, Clone, Default)]
pub struct ClientMetrics {
    pub client_id: ClientId,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_failed: u64,
    pub connection_errors: u64,
    pub total_rtt_ms: u64,
    pub min_rtt_ms: Option<u64>,
    pub max_rtt_ms: Option<u64>,
    pub http_connections_created: u64,
    pub http_connections_reused: u64,
    pub http_status_codes: std::collections::HashMap<u16, u64>,
    pub total_clients: u32,
}

impl ClientMetrics {
    pub fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            total_clients: 1, // Default to 1 for echo mode
            ..Default::default()
        }
    }

    /// Record a successful message round trip
    pub fn record_success(&mut self, rtt: Duration) {
        self.messages_received += 1;
        let rtt_ms = u64::try_from(rtt.as_millis()).unwrap_or(u64::MAX);
        self.total_rtt_ms += rtt_ms;

        self.min_rtt_ms = Some(match self.min_rtt_ms {
            Some(min) => min.min(rtt_ms),
            None => rtt_ms,
        });

        self.max_rtt_ms = Some(match self.max_rtt_ms {
            Some(max) => max.max(rtt_ms),
            None => rtt_ms,
        });
    }

    /// Record a message send
    pub fn record_sent(&mut self) {
        self.messages_sent += 1;
    }

    /// Record a message failure
    pub fn record_failure(&mut self) {
        self.messages_failed += 1;
    }

    /// Record a connection error
    pub fn record_connection_error(&mut self) {
        self.connection_errors += 1;
    }

    /// Record HTTP connection creation
    pub fn record_http_connection_created(&mut self) {
        self.http_connections_created += 1;
    }

    /// Record HTTP connection reuse
    pub fn record_http_connection_reused(&mut self) {
        self.http_connections_reused += 1;
    }

    /// Record HTTP status code
    pub fn record_http_status_code(&mut self, status_code: u16) {
        *self.http_status_codes.entry(status_code).or_insert(0) += 1;
    }

    /// Calculate success rate (0.0 to 100.0)
    pub fn success_rate(&self) -> f64 {
        let total = self.messages_sent;
        if total == 0 {
            return 100.0;
        }

        // For broadcast mode, normalize by dividing by total clients
        if self.total_clients > 1 {
            // Broadcast mode: normalize the received count by total clients
            let normalized_received = (self.messages_received as f64) / (self.total_clients as f64);
            (normalized_received / total as f64) * 100.0
        } else {
            // Echo mode: regular calculation
            (self.messages_received as f64 / total as f64) * 100.0
        }
    }

    /// Calculate average RTT in milliseconds
    #[allow(dead_code)]
    pub fn avg_rtt_ms(&self) -> f64 {
        if self.messages_received == 0 {
            0.0
        } else {
            self.total_rtt_ms as f64 / self.messages_received as f64
        }
    }

    /// Get total messages attempted (sent + failed)
    pub fn total_attempted(&self) -> u64 {
        self.messages_sent + self.messages_failed
    }

    /// Check if client has any activity
    pub fn has_activity(&self) -> bool {
        self.total_attempted() > 0 || self.connection_errors > 0
    }
}
