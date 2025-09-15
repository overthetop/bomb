//! HTTP client implementation for stress testing

use crate::common::{ClientCore, ClientId};
use crate::config::{Config, HttpMethod};
use crate::constants::*;
use crate::errors::{ErrorContext, Result};
use crate::message::{MessageId, PendingMessage};
use crate::metrics::AggregateMetrics;

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, broadcast};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, info, warn};

/// Individual HTTP client that sends requests to HTTP endpoints
pub struct HttpClient {
    pub(crate) core: ClientCore,
    pub(crate) http_client: reqwest::Client,
    pub(crate) connected_hosts: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl HttpClient {
    pub fn new(
        client_id: ClientId,
        config: Config,
        metrics: Arc<AggregateMetrics>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout_duration())
            .build()
            .expect("Failed to create HTTP client");

        Self {
            core: ClientCore::new(client_id, config, metrics, shutdown_rx),
            http_client,
            connected_hosts: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Main HTTP stress testing loop
    pub(crate) async fn run_http_test(&mut self) -> Result<()> {
        let resolved_url_str = self.core.config.resolve_target_url();
        info!(
            "Client {} starting HTTP stress test against {}",
            self.core.client_id, resolved_url_str
        );

        // Set up rate limiting if configured
        let mut interval_timer = self.core.config.message_interval().map(|duration| {
            let mut timer = interval(duration);
            timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
            timer
        });

        let mut messages_sent = 0u64;
        let test_start = Instant::now();

        // Determine termination condition
        let max_messages = self.core.config.messages_per_client();
        let max_duration = self.core.config.test_duration();

        loop {
            // Check termination conditions
            if let Some(max) = max_messages
                && messages_sent >= max
            {
                info!(
                    "Client {} reached message limit: {}",
                    self.core.client_id, max
                );
                break;
            }

            if let Some(duration) = max_duration
                && test_start.elapsed() >= duration
            {
                info!(
                    "Client {} reached time limit: {:?}",
                    self.core.client_id, duration
                );
                break;
            }

            // Check for shutdown signal
            if self.core.shutdown_rx.try_recv().is_ok() {
                info!("Client {} received shutdown signal", self.core.client_id);
                break;
            }

            // Rate limiting
            if let Some(ref mut timer) = interval_timer {
                timer.tick().await;
            }

            // Send HTTP request
            match self.send_http_request().await {
                Ok(_) => {
                    messages_sent += 1;
                    if messages_sent % DEBUG_LOG_INTERVAL == 0 {
                        debug!(
                            "Client {} sent {} messages",
                            self.core.client_id, messages_sent
                        );
                    }
                }
                Err(e) => {
                    warn!("Client {} HTTP request failed: {}", self.core.client_id, e);
                    self.core
                        .metrics
                        .record_message_failure(self.core.client_id)
                        .await;
                }
            }

            // Clean up timed-out messages periodically
            if messages_sent % BULK_TIMEOUT_CLEANUP_INTERVAL == 0 {
                self.cleanup_timed_out_messages().await;
            }
        }

        info!(
            "Client {} completed HTTP test with {} requests sent",
            self.core.client_id, messages_sent
        );
        Ok(())
    }

    async fn send_http_request(&mut self) -> Result<()> {
        let (url, payload, message_id) = self.prepare_request_data()?;

        self.track_connection_usage(&url).await?;
        self.add_pending_message(&message_id).await;

        let request = self.build_http_request(&url, &payload)?;
        self.send_and_handle_response(request, message_id).await
    }

    fn prepare_request_data(&self) -> Result<(String, String, MessageId)> {
        let url = self.core.config.resolve_target_url();
        let payload = self.core.config.resolve_payload();

        // For HTTP mode, try to extract message ID from payload, but if not present,
        // generate one for internal tracking purposes
        let message_id = match Config::extract_message_id(&payload) {
            Ok(id) => MessageId::from(id),
            Err(_) => {
                // Generate a unique ID for internal tracking when payload doesn't have one
                MessageId::from(uuid::Uuid::new_v4().to_string())
            }
        };

        Ok((url, payload, message_id))
    }

    async fn track_connection_usage(&mut self, url: &str) -> Result<()> {
        let parsed_url = url::Url::parse(url)?;
        let host = format!(
            "{}:{}",
            parsed_url.host_str().unwrap_or("unknown"),
            parsed_url.port_or_known_default().unwrap_or(80)
        );

        let is_new_connection = {
            let mut hosts = self.connected_hosts.lock().await;
            let is_new = !hosts.contains(&host);
            if is_new {
                hosts.insert(host.clone());
                true
            } else {
                false
            }
        };

        if is_new_connection {
            self.core
                .metrics
                .record_http_connection_created(self.core.client_id)
                .await;
        } else {
            self.core
                .metrics
                .record_http_connection_reused(self.core.client_id)
                .await;
        }

        Ok(())
    }

    async fn add_pending_message(&self, message_id: &MessageId) {
        let pending_message = PendingMessage::new(self.core.config.timeout_duration());
        self.core
            .pending_messages
            .lock()
            .await
            .insert(message_id.clone(), pending_message);
    }

    fn build_http_request(&self, url: &str, payload: &str) -> Result<reqwest::RequestBuilder> {
        let method = match self.core.config.http_method().unwrap_or(HttpMethod::Get) {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
            HttpMethod::Put => reqwest::Method::PUT,
            HttpMethod::Delete => reqwest::Method::DELETE,
            HttpMethod::Patch => reqwest::Method::PATCH,
        };

        let mut request = self.http_client.request(method, url);

        // Add custom headers
        let custom_headers = self.core.config.custom_headers()?;
        for (key, value) in custom_headers {
            request = request.header(&key, &value);
        }

        // Add payload for POST/PUT/PATCH requests
        if matches!(
            self.core.config.http_method().unwrap_or(HttpMethod::Get),
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch
        ) {
            request = request
                .header("Content-Type", "application/json")
                .body(payload.to_string());
        }

        Ok(request)
    }

    async fn send_and_handle_response(
        &self,
        request: reqwest::RequestBuilder,
        message_id: MessageId,
    ) -> Result<()> {
        let send_time = Instant::now();

        // Record that we're sending a message
        self.core
            .metrics
            .record_message_sent(self.core.client_id)
            .await;

        // Send request
        let response = request
            .send()
            .await
            .with_transport_context("HTTP request failed")?;

        let response_time = Instant::now();
        let status = response.status();

        // Record HTTP status code
        self.core
            .metrics
            .record_http_status_code(self.core.client_id, status.as_u16())
            .await;

        // Record metrics based on response
        if status.is_success() {
            // Try to extract response body to check for message ID (for echo-like servers)
            let _response_body = response.text().await.unwrap_or_default();

            // For HTTP, we consider the request successful if we get a 2xx status
            let rtt = response_time.duration_since(send_time);
            self.core
                .metrics
                .record_message_success(self.core.client_id, rtt)
                .await;

            // Remove from pending messages
            self.core.pending_messages.lock().await.remove(&message_id);
        } else {
            warn!(
                "Client {} received HTTP status {}",
                self.core.client_id, status
            );
            self.core
                .metrics
                .record_message_failure(self.core.client_id)
                .await;
        }

        Ok(())
    }

    /// Clean up messages that have timed out
    async fn cleanup_timed_out_messages(&mut self) {
        let mut pending = self.core.pending_messages.lock().await;
        let now = Instant::now();
        let mut timed_out = Vec::new();

        for (message_id, pending_message) in pending.iter() {
            if now >= pending_message.timeout_at {
                timed_out.push(message_id.clone());
            }
        }

        for message_id in timed_out {
            pending.remove(&message_id);
            self.core
                .metrics
                .record_message_failure(self.core.client_id)
                .await;
        }
    }
}
