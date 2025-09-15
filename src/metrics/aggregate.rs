//! Aggregate metrics collection across all clients

use crate::common::ClientId;
use crate::metrics::client::ClientMetrics;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;

/// Thread-safe aggregate metrics for all clients
#[derive(Debug, Default)]
pub struct AggregateMetrics {
    pub total_messages_sent: AtomicU64,
    pub total_messages_received: AtomicU64,
    pub total_messages_failed: AtomicU64,
    pub total_connection_errors: AtomicU64,
    pub total_reconnections: AtomicU64,
    pub total_http_connections_created: AtomicU64,
    pub total_http_connections_reused: AtomicU64,
    pub http_status_codes: Arc<RwLock<HashMap<u16, u64>>>,
    client_metrics: Arc<RwLock<HashMap<ClientId, ClientMetrics>>>,
}

impl AggregateMetrics {
    pub fn new() -> Self {
        Self {
            client_metrics: Arc::new(RwLock::new(HashMap::new())),
            http_status_codes: Arc::new(RwLock::new(HashMap::new())),
            ..Default::default()
        }
    }

    /// Initialize metrics for a client
    pub async fn init_client(&self, client_id: ClientId) {
        let mut metrics = self.client_metrics.write().await;
        metrics.insert(client_id, ClientMetrics::new(client_id));
    }

    /// Initialize metrics for a client with total client count (for broadcast mode)
    pub async fn init_client_with_count(&self, client_id: ClientId, total_clients: u32) {
        let mut metrics = self.client_metrics.write().await;
        let mut client_metrics = ClientMetrics::new(client_id);
        client_metrics.total_clients = total_clients;
        metrics.insert(client_id, client_metrics);
    }

    /// Record a message sent by a client
    pub async fn record_message_sent(&self, client_id: ClientId) {
        self.total_messages_sent.fetch_add(1, Ordering::Relaxed);

        // Use a more efficient pattern - try to minimize lock duration
        let mut metrics = self.client_metrics.write().await;
        if let Some(client_metrics) = metrics.get_mut(&client_id) {
            client_metrics.record_sent();
        }
    }

    /// Record a successful message response
    pub async fn record_message_success(&self, client_id: ClientId, rtt: Duration) {
        self.total_messages_received.fetch_add(1, Ordering::Relaxed);

        let mut metrics = self.client_metrics.write().await;
        if let Some(client_metrics) = metrics.get_mut(&client_id) {
            client_metrics.record_success(rtt);
        }
    }

    /// Record a message failure
    pub async fn record_message_failure(&self, client_id: ClientId) {
        self.total_messages_failed.fetch_add(1, Ordering::Relaxed);

        let mut metrics = self.client_metrics.write().await;
        if let Some(client_metrics) = metrics.get_mut(&client_id) {
            client_metrics.record_failure();
        }
    }

    /// Record a connection error
    pub async fn record_connection_error(&self, client_id: ClientId) {
        self.total_connection_errors.fetch_add(1, Ordering::Relaxed);

        let mut metrics = self.client_metrics.write().await;
        if let Some(client_metrics) = metrics.get_mut(&client_id) {
            client_metrics.record_connection_error();
        }
    }

    /// Record HTTP connection creation
    pub async fn record_http_connection_created(&self, client_id: ClientId) {
        self.total_http_connections_created
            .fetch_add(1, Ordering::Relaxed);

        let mut metrics = self.client_metrics.write().await;
        if let Some(client_metrics) = metrics.get_mut(&client_id) {
            client_metrics.record_http_connection_created();
        }
    }

    /// Record HTTP connection reuse
    pub async fn record_http_connection_reused(&self, client_id: ClientId) {
        self.total_http_connections_reused
            .fetch_add(1, Ordering::Relaxed);

        let mut metrics = self.client_metrics.write().await;
        if let Some(client_metrics) = metrics.get_mut(&client_id) {
            client_metrics.record_http_connection_reused();
        }
    }

    /// Record HTTP status code
    pub async fn record_http_status_code(&self, client_id: ClientId, status_code: u16) {
        // Update global status code tracking
        {
            let mut status_codes = self.http_status_codes.write().await;
            *status_codes.entry(status_code).or_insert(0) += 1;
        }

        // Update client-specific tracking
        let mut metrics = self.client_metrics.write().await;
        if let Some(client_metrics) = metrics.get_mut(&client_id) {
            client_metrics.record_http_status_code(status_code);
        }
    }

    /// Get read-only access to client metrics
    pub async fn get_client_metrics(&self) -> Vec<ClientMetrics> {
        let metrics = self.client_metrics.read().await;
        metrics.values().cloned().collect()
    }

    /// Calculate overall success rate (handles both individual and broadcast aggregated metrics)
    pub async fn overall_success_rate(&self) -> f64 {
        let sent = self.total_messages_sent.load(Ordering::Relaxed);
        let received = self.total_messages_received.load(Ordering::Relaxed);

        if sent == 0 {
            return 100.0;
        }

        // Check if we're in broadcast mode by looking at client metrics
        let metrics = self.client_metrics.read().await;
        let is_broadcast_mode = metrics.values().any(|m| m.total_clients > 1);

        if is_broadcast_mode {
            // In broadcast mode, normalize by dividing received by total clients
            if let Some(total_clients) = metrics.values().next().map(|m| m.total_clients) {
                let normalized_received = received as f64 / total_clients as f64;
                (normalized_received / sent as f64) * 100.0
            } else {
                (received as f64 / sent as f64) * 100.0
            }
        } else {
            // Echo mode: regular calculation
            (received as f64 / sent as f64) * 100.0
        }
    }

    /// Calculate average RTT across all clients
    pub async fn overall_avg_rtt_ms(&self) -> f64 {
        let metrics = self.client_metrics.read().await;
        let client_metrics: Vec<_> = metrics.values().collect();

        let total_rtt: u64 = client_metrics.iter().map(|m| m.total_rtt_ms).sum();
        let total_received: u64 = client_metrics.iter().map(|m| m.messages_received).sum();

        if total_received == 0 {
            return 0.0;
        }

        total_rtt as f64 / total_received as f64
    }

    /// Get minimum RTT across all clients
    pub async fn overall_min_rtt_ms(&self) -> Option<u64> {
        let metrics = self.client_metrics.read().await;
        metrics.values().filter_map(|m| m.min_rtt_ms).min()
    }

    /// Get maximum RTT across all clients
    pub async fn overall_max_rtt_ms(&self) -> Option<u64> {
        let metrics = self.client_metrics.read().await;
        metrics.values().filter_map(|m| m.max_rtt_ms).max()
    }
}
