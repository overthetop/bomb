//! Client module for stress testing implementations
//!
//! This module provides a clean, modular approach to client implementations:
//! - Common trait abstractions for different client types
//! - HTTP-specific client implementation
//! - WebSocket-specific client implementation
//! - Client manager for coordinating multiple clients
//! - Message tracking and lifecycle management

pub mod http;
pub mod manager;
pub mod messaging;
pub mod websocket;

// Re-export public types for easier access
pub use http::HttpClient;
pub use manager::ClientManager;
pub use messaging::BroadcastTracker;
pub use websocket::WebSocketClient;

use crate::common::ClientId;
use crate::constants::*;
use crate::errors::Result;

use async_trait::async_trait;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

/// Common interface for both HTTP and WebSocket stress testing clients
#[async_trait]
pub trait StressTestClient {
    /// Run the stress test for this client
    async fn run(&mut self) -> Result<()>;

    /// Get the client ID
    fn client_id(&self) -> ClientId;
}

#[async_trait]
impl StressTestClient for HttpClient {
    async fn run(&mut self) -> Result<()> {
        info!("Client {} starting", self.core.client_id);

        // Initialize client metrics
        self.core.metrics.init_client(self.core.client_id).await;

        // For HTTP, we don't need reconnection logic like WebSocket
        // Just run the test and handle any errors
        match self.run_http_test().await {
            Ok(_) => {
                info!("Client {} completed successfully", self.core.client_id);
                Ok(())
            }
            Err(e) => {
                error!("Client {} HTTP test failed: {}", self.core.client_id, e);
                Err(e)
            }
        }
    }

    fn client_id(&self) -> ClientId {
        self.core.client_id
    }
}

#[async_trait]
impl StressTestClient for WebSocketClient {
    async fn run(&mut self) -> Result<()> {
        info!("Client {} starting", self.core.client_id);

        // Initialize client metrics with total client count for broadcast normalization
        self.core
            .metrics
            .init_client_with_count(self.core.client_id, self.total_clients)
            .await;

        let mut reconnect_attempts = 0;
        let initial_backoff = Duration::from_millis(INITIAL_BACKOFF_MS);

        loop {
            match self.connect_and_run().await {
                Ok(_) => {
                    info!("Client {} completed successfully", self.core.client_id);
                    break;
                }
                Err(e) => {
                    error!(
                        "Client {} encountered error: {} (attempt {}/{})",
                        self.core.client_id,
                        e,
                        reconnect_attempts + 1,
                        MAX_RECONNECT_ATTEMPTS
                    );

                    self.core
                        .metrics
                        .record_connection_error(self.core.client_id)
                        .await;
                    reconnect_attempts += 1;

                    if reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
                        error!(
                            "Client {} exceeded maximum reconnection attempts",
                            self.core.client_id
                        );
                        return Err(e);
                    }

                    // Exponential backoff for reconnection
                    let backoff = initial_backoff * 2_u32.pow(reconnect_attempts.saturating_sub(1));
                    tracing::debug!(
                        "Client {} backing off for {:?}",
                        self.core.client_id,
                        backoff
                    );
                    sleep(backoff).await;
                }
            }
        }

        Ok(())
    }

    fn client_id(&self) -> ClientId {
        self.core.client_id
    }
}
