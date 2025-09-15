//! Client manager for coordinating multiple stress testing clients

use crate::client::{BroadcastTracker, HttpClient, StressTestClient, WebSocketClient};
use crate::common::ClientId;
use crate::config::{Config, ConnectionMode, WSMode};
use crate::constants::*;
use crate::errors::Result;
use crate::metrics::AggregateMetrics;

use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Client manager that spawns and coordinates multiple stress testing clients
pub struct ClientManager {
    config: Config,
    metrics: Arc<AggregateMetrics>,
    test_start_time: Option<Instant>,
    test_end_time: Option<Instant>,
    broadcast_tracker: Option<BroadcastTracker>,
}

impl ClientManager {
    pub fn new(config: Config) -> Self {
        // Create broadcast tracker if in WebSocket broadcast mode
        let broadcast_tracker = match (&config.connection_mode(), config.ws_mode()) {
            (ConnectionMode::Ws, Some(WSMode::Broadcast)) => Some(BroadcastTracker::new(
                config.client.count,
                config.timeout_duration(),
            )),
            _ => None,
        };

        Self {
            config,
            metrics: Arc::new(AggregateMetrics::new()),
            test_start_time: None,
            test_end_time: None,
            broadcast_tracker,
        }
    }

    /// Run the stress test with multiple clients
    pub async fn run_stress_test(&mut self) -> Result<()> {
        info!(
            "Starting stress test with {} clients",
            self.config.client.count
        );

        self.test_start_time = Some(Instant::now());
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        self.setup_signal_handler(shutdown_tx.clone());
        let client_handles = self.spawn_all_clients(shutdown_tx.clone()).await;
        self.wait_for_clients_completion(client_handles, shutdown_tx)
            .await?;

        self.test_end_time = Some(Instant::now());
        self.print_final_report().await;

        Ok(())
    }

    /// Set up signal handler for graceful shutdown
    fn setup_signal_handler(&self, shutdown_tx: broadcast::Sender<()>) {
        tokio::spawn(async move {
            if let Err(e) = tokio::signal::ctrl_c().await {
                error!("Failed to listen for ctrl+c: {}", e);
                return;
            }
            warn!("Received Ctrl+C, initiating graceful shutdown...");
            let _ = shutdown_tx.send(());
        });
    }

    /// Spawn all clients and return their handles
    async fn spawn_all_clients(
        &self,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut client_handles = Vec::new();

        for client_id_raw in 0..self.config.client.count {
            let client_id = ClientId::from(client_id_raw);
            let mut client = self.create_client(client_id, shutdown_tx.clone());

            let handle = tokio::spawn(async move {
                if let Err(e) = client.run().await {
                    error!("Client {} failed: {}", client.client_id(), e);
                }
            });

            client_handles.push(handle);

            // Small delay between client starts to avoid thundering herd
            tokio::time::sleep(Duration::from_millis(CLIENT_START_DELAY_MS)).await;
        }

        client_handles
    }

    /// Create a client of the appropriate type
    fn create_client(
        &self,
        client_id: ClientId,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Box<dyn StressTestClient + Send> {
        match self.config.connection_mode() {
            ConnectionMode::Ws => {
                // Clone the broadcast tracker if available
                let broadcast_tracker = self.broadcast_tracker.as_ref().map(|t| t.clone());

                // Determine total clients for metrics normalization
                let total_clients = match self.config.ws_mode() {
                    Some(WSMode::Broadcast) => self.config.client.count,
                    _ => 1, // Echo mode: each client only talks to itself
                };

                Box::new(WebSocketClient::new(
                    client_id,
                    self.config.clone(),
                    Arc::clone(&self.metrics),
                    shutdown_tx.subscribe(),
                    broadcast_tracker,
                    total_clients,
                ))
            }
            ConnectionMode::Http => Box::new(HttpClient::new(
                client_id,
                self.config.clone(),
                Arc::clone(&self.metrics),
                shutdown_tx.subscribe(),
            )),
        }
    }

    /// Wait for all clients to complete with progress indication
    async fn wait_for_clients_completion(
        &mut self,
        client_handles: Vec<tokio::task::JoinHandle<()>>,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Result<()> {
        info!("Waiting for all clients to complete...");

        let global_timeout = self
            .config
            .test_duration()
            .unwrap_or(Duration::from_secs(DEFAULT_GLOBAL_TIMEOUT_MINUTES * 60))
            + Duration::from_secs(EXTRA_CLEANUP_TIME_SECONDS);

        if self.config.output.verbose {
            self.wait_verbose_mode(client_handles, global_timeout, shutdown_tx)
                .await
        } else {
            self.wait_with_progress_dots(client_handles, global_timeout, shutdown_tx)
                .await
        }
    }

    /// Wait for clients in verbose mode (no progress dots)
    async fn wait_verbose_mode(
        &self,
        client_handles: Vec<tokio::task::JoinHandle<()>>,
        global_timeout: Duration,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Result<()> {
        match timeout(
            global_timeout,
            futures_util::future::join_all(client_handles),
        )
        .await
        {
            Ok(_) => {
                info!("All clients completed successfully");
            }
            Err(_) => {
                warn!("Some clients did not complete within timeout");
                let _ = shutdown_tx.send(());
            }
        }
        Ok(())
    }

    /// Wait for clients with progress dots in non-verbose mode
    async fn wait_with_progress_dots(
        &self,
        client_handles: Vec<tokio::task::JoinHandle<()>>,
        global_timeout: Duration,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Result<()> {
        print!("Progress: ");
        let _ = std::io::stdout().flush();

        let mut dot_interval =
            tokio::time::interval(Duration::from_millis(PROGRESS_DOT_INTERVAL_MS));
        let clients_future = futures_util::future::join_all(client_handles);

        tokio::select! {
            result = timeout(global_timeout, clients_future) => {
                println!(); // New line after dots
                match result {
                    Ok(_) => {
                        info!("All clients completed successfully");
                    }
                    Err(_) => {
                        warn!("Some clients did not complete within timeout");
                        let _ = shutdown_tx.send(());
                    }
                }
            }
            _ = async {
                loop {
                    dot_interval.tick().await;
                    print!(".");
                    let _ = std::io::stdout().flush();
                }
            } => {
                // This branch should never be reached as the dots loop is infinite
            }
        }

        Ok(())
    }

    /// Print the final test report
    async fn print_final_report(&self) {
        let test_duration = match (self.test_start_time, self.test_end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        };
        let config_summary = format!(
            "   Target:           {}\n   Clients:          {}\n",
            self.config.target.url, self.config.client.count
        ) + &match (
            self.config.test.duration.map(|d| d.as_secs()),
            self.config.test.total_messages,
        ) {
            (Some(duration), None) => format!("   Duration:         {}s\n", duration),
            (None, Some(total)) => {
                let per_client = self.config.messages_per_client().unwrap_or(0);
                format!(
                    "   Total Messages:   {}\n   Per Client:       {} messages\n",
                    total, per_client
                )
            }
            _ => String::new(),
        } + &format!(
            "   Message Rate:     {} msg/s per client\n   Timeout:          {}s\n",
            self.config.test.message_rate,
            self.config.client.timeout.as_secs()
        ) + &if self.config.target.insecure {
            "   Security:         ⚠️  Insecure mode enabled\n".to_string()
        } else {
            String::new()
        };

        let connection_mode = match self.config.connection_mode() {
            ConnectionMode::Http => "http",
            ConnectionMode::Ws => "ws",
        };
        self.metrics
            .print_report(&config_summary, test_duration, connection_mode)
            .await;
    }
}
