//! Shared utilities and common patterns used across the codebase

use crate::config::Config;
use crate::message::{MessageId, PendingMessage};
use crate::metrics::AggregateMetrics;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

/// Type-safe wrapper for client ID to prevent confusion with other numeric types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ClientId(pub u32);

impl ClientId {
    /// Create a new ClientId
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the underlying u32 value
    pub fn get(&self) -> u32 {
        self.0
    }
}

impl From<u32> for ClientId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl From<ClientId> for u32 {
    fn from(client_id: ClientId) -> Self {
        client_id.0
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Common client state shared between HTTP and WebSocket clients
#[derive(Debug)]
pub struct ClientCore {
    pub client_id: ClientId,
    pub config: Config,
    pub metrics: Arc<AggregateMetrics>,
    pub shutdown_rx: broadcast::Receiver<()>,
    pub pending_messages: Arc<Mutex<HashMap<MessageId, PendingMessage>>>,
}

impl ClientCore {
    /// Create a new client core with the given parameters
    pub fn new(
        client_id: ClientId,
        config: Config,
        metrics: Arc<AggregateMetrics>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            client_id,
            config,
            metrics,
            shutdown_rx,
            pending_messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
