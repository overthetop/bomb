//! Message tracking and lifecycle management for stress testing clients

use crate::common::ClientId;
use crate::config::Config;
use crate::message::MessageId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Loop state for message processing
#[derive(Debug)]
pub struct LoopState {
    pub messages_sent: u64,
    pub target_messages: Option<u64>,
    pub start_time: Instant,
    pub test_duration: Option<Duration>,
}

impl LoopState {
    /// Create new loop state from configuration
    pub fn new(config: &Config) -> Self {
        Self {
            messages_sent: 0,
            target_messages: config.messages_per_client(),
            start_time: Instant::now(),
            test_duration: config.test_duration(),
        }
    }

    /// Check if termination conditions have been met
    pub fn check_termination_conditions(&self, client_id: crate::common::ClientId) -> bool {
        if let Some(target) = self.target_messages
            && self.messages_sent >= target
        {
            tracing::info!("Client {} reached message target: {}", client_id, target);
            return true;
        }

        if let Some(duration) = self.test_duration
            && self.start_time.elapsed() >= duration
        {
            tracing::info!("Client {} reached time limit: {:?}", client_id, duration);
            return true;
        }

        false
    }
}

/// Individual broadcast message tracking with response aggregation
#[derive(Debug)]
struct BroadcastMessage {
    sender_id: ClientId,
    send_time: Instant,
    responses: Vec<(ClientId, Duration)>,
    timeout_at: Instant,
    finalized: bool,
}

/// Tracks broadcast messages across multiple clients with comprehensive response aggregation
#[derive(Debug)]
pub struct BroadcastTracker {
    /// Map of message ID to comprehensive tracking data
    messages: Arc<RwLock<HashMap<MessageId, BroadcastMessage>>>,
    /// Total number of clients participating
    total_clients: u32,
    /// Timeout for message tracking
    timeout: Duration,
}

impl BroadcastTracker {
    pub fn new(total_clients: u32, timeout: Duration) -> Self {
        Self {
            messages: Arc::new(RwLock::new(HashMap::new())),
            total_clients,
            timeout,
        }
    }

    /// Register a sent message for comprehensive tracking
    pub async fn register_sent(&self, message_id: MessageId, sender_id: ClientId) {
        let now = Instant::now();
        let mut messages = self.messages.write().await;
        messages.insert(
            message_id,
            BroadcastMessage {
                sender_id,
                send_time: now,
                responses: Vec::new(),
                timeout_at: now + self.timeout,
                finalized: false,
            },
        );
    }

    /// Record a response from a client for a broadcast message
    pub async fn record_response(
        &self,
        message_id: &MessageId,
        receiver_id: ClientId,
    ) -> Option<ClientId> {
        let mut messages = self.messages.write().await;
        if let Some(broadcast_msg) = messages.get_mut(message_id)
            && !broadcast_msg.finalized
        {
            let rtt = broadcast_msg.send_time.elapsed();
            broadcast_msg.responses.push((receiver_id, rtt));
            return Some(broadcast_msg.sender_id);
        }
        None
    }

    /// Finalize completed or timed-out messages and return aggregated metrics for all clients
    pub async fn finalize_ready_messages(&self) -> Vec<(ClientId, f64, f64)> {
        let mut messages = self.messages.write().await;
        let now = Instant::now();
        let mut finalized_metrics = Vec::new();
        let mut to_remove = Vec::new();

        for (msg_id, broadcast_msg) in messages.iter_mut() {
            if broadcast_msg.finalized {
                continue;
            }

            // Finalize if we have responses from all expected clients OR timeout reached
            let should_finalize = broadcast_msg.responses.len() as u32 >= self.total_clients
                || now >= broadcast_msg.timeout_at;

            if should_finalize {
                broadcast_msg.finalized = true;

                // Calculate true broadcast success rate: how many clients received the message
                let response_count = broadcast_msg.responses.len();
                let success_rate = if self.total_clients > 0 {
                    (response_count as f64) / (self.total_clients as f64)
                } else {
                    0.0
                };

                let avg_rtt_ms = if response_count > 0 {
                    let total_rtt_ms: u64 = broadcast_msg
                        .responses
                        .iter()
                        .map(|(_, rtt)| rtt.as_millis() as u64)
                        .sum();
                    (total_rtt_ms as f64) / (response_count as f64)
                } else {
                    0.0
                };

                // Credit the success to ALL clients that received the message, not just the sender
                for (client_id, _) in &broadcast_msg.responses {
                    finalized_metrics.push((*client_id, success_rate, avg_rtt_ms));
                }

                to_remove.push(msg_id.clone());
            }
        }

        // Remove finalized messages
        for msg_id in to_remove {
            messages.remove(&msg_id);
        }

        finalized_metrics
    }

    /// Get a clone of the tracker for sharing across clients
    pub fn clone(&self) -> Self {
        Self {
            messages: self.messages.clone(),
            total_clients: self.total_clients,
            timeout: self.timeout,
        }
    }
}
