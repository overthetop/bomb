//! WebSocket client implementation for stress testing

use crate::client::messaging::{BroadcastTracker, LoopState};
use crate::common::{ClientCore, ClientId};
use crate::config::{Config, WSMode};
use crate::constants::*;
use crate::errors::{BombError, ErrorContext, Result};
use crate::message::{MessageId, PendingMessage};
use crate::metrics::AggregateMetrics;

use futures_util::{SinkExt, StreamExt};
use http::{HeaderName, HeaderValue};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::time::{MissedTickBehavior, interval};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config, tungstenite::Message,
    tungstenite::client::IntoClientRequest, tungstenite::protocol::WebSocketConfig,
};
use tracing::{debug, error, info, warn};
use url::Url;

/// Individual WebSocket client that connects to the server and sends/receives messages
pub struct WebSocketClient {
    pub(crate) core: ClientCore,
    pub(crate) has_started_sending: Arc<Mutex<bool>>,
    pub(crate) broadcast_tracker: Option<BroadcastTracker>,
    pub(crate) total_clients: u32,
}

impl WebSocketClient {
    pub fn new(
        client_id: ClientId,
        config: Config,
        metrics: Arc<AggregateMetrics>,
        shutdown_rx: broadcast::Receiver<()>,
        broadcast_tracker: Option<BroadcastTracker>,
        total_clients: u32,
    ) -> Self {
        Self {
            core: ClientCore::new(client_id, config, metrics, shutdown_rx),
            has_started_sending: Arc::new(Mutex::new(false)),
            broadcast_tracker,
            total_clients,
        }
    }

    /// Connect to WebSocket server and run the main message loop
    pub(crate) async fn connect_and_run(&mut self) -> Result<()> {
        let resolved_url_str = self.core.config.resolve_target_url();
        let url = Url::parse(&resolved_url_str).with_config_context(&format!(
            "Invalid resolved target URL '{}'",
            resolved_url_str
        ))?;
        info!("Client {} connecting to {}", self.core.client_id, url);

        let ws_stream = self.establish_websocket_connection(&url).await?;
        info!("Client {} connected successfully", self.core.client_id);

        let (tx_send, rx_received) = self.setup_websocket_tasks(ws_stream);
        self.message_loop(tx_send, rx_received).await
    }

    async fn establish_websocket_connection(
        &self,
        url: &Url,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let mut request = url
            .as_str()
            .into_client_request()
            .with_transport_context(&format!("Failed to create WebSocket request for {}", url))?;

        // Add custom headers
        let custom_headers = self.core.config.custom_headers()?;
        for (key, value) in custom_headers {
            let header_name: HeaderName = key
                .parse()
                .with_config_context(&format!("Invalid header name: {}", key))?;
            let header_value: HeaderValue = value
                .parse()
                .with_config_context(&format!("Invalid header value: {}", value))?;
            request.headers_mut().insert(header_name, header_value);
        }

        // Connect to WebSocket server with custom configuration
        let ws_config = WebSocketConfig::default();
        let (ws_stream, _response) =
            connect_async_with_config(request, Some(ws_config), self.core.config.target.insecure)
                .await
                .with_transport_context(&format!(
                    "Failed to connect to WebSocket server at {}",
                    url
                ))?;

        Ok(ws_stream)
    }

    fn setup_websocket_tasks(
        &self,
        ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> (mpsc::Sender<String>, mpsc::Receiver<String>) {
        let (mut ws_sink, mut ws_stream) = ws_stream.split();

        // Create channels for internal communication
        let (tx_send, mut rx_send) = mpsc::channel::<String>(CHANNEL_BUFFER_SIZE);
        let (tx_received, rx_received) = mpsc::channel::<String>(CHANNEL_BUFFER_SIZE);

        // Spawn message receiver task
        let client_id = self.core.client_id;
        let shutdown_rx_clone = self.core.shutdown_rx.resubscribe();
        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_rx_clone;
            loop {
                tokio::select! {
                    msg = ws_stream.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if tx_received.send(text.to_string()).await.is_err() {
                                    debug!("Client {} receiver channel closed", client_id);
                                    break;
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("Client {} received close frame", client_id);
                                break;
                            }
                            Some(Err(e)) => {
                                error!("Client {} WebSocket error: {}", client_id, e);
                                break;
                            }
                            None => {
                                info!("Client {} WebSocket stream ended", client_id);
                                break;
                            }
                            _ => {
                                // Ignore other message types (binary, ping, pong)
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Client {} receiver task shutting down", client_id);
                        break;
                    }
                }
            }
        });

        // Spawn message sender task
        let client_id = self.core.client_id;
        tokio::spawn(async move {
            while let Some(json) = rx_send.recv().await {
                if let Err(e) = ws_sink.send(Message::Text(json.into())).await {
                    error!(
                        "Client {} failed to send WebSocket message: {}",
                        client_id, e
                    );
                    break;
                }
            }
        });

        (tx_send, rx_received)
    }

    /// Main message processing loop
    async fn message_loop(
        &mut self,
        tx_send: mpsc::Sender<String>,
        mut rx_received: mpsc::Receiver<String>,
    ) -> Result<()> {
        let (message_timer_opt, timeout_check_timer) = self.setup_message_timers();
        let mut loop_state = LoopState::new(&self.core.config);

        if let Some(message_timer) = message_timer_opt {
            self.run_rate_limited_loop(
                message_timer,
                timeout_check_timer,
                &tx_send,
                &mut rx_received,
                &mut loop_state,
            )
            .await?;
        } else {
            self.run_unlimited_rate_loop(
                timeout_check_timer,
                &tx_send,
                &mut rx_received,
                &mut loop_state,
            )
            .await?;
        }

        // Wait for remaining responses with timeout
        self.wait_for_pending_responses(rx_received).await?;

        info!("Client {} finished processing", self.core.client_id);
        Ok(())
    }

    fn setup_message_timers(&self) -> (Option<tokio::time::Interval>, tokio::time::Interval) {
        let message_timer_opt = self
            .core
            .config
            .message_interval()
            .map(|interval_duration| {
                let mut timer = interval(interval_duration);
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
                timer
            });

        let mut timeout_check_timer = interval(TIMEOUT_CHECK_INTERVAL);
        timeout_check_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        (message_timer_opt, timeout_check_timer)
    }

    async fn run_rate_limited_loop(
        &mut self,
        mut message_timer: tokio::time::Interval,
        mut timeout_check_timer: tokio::time::Interval,
        tx_send: &mpsc::Sender<String>,
        rx_received: &mut mpsc::Receiver<String>,
        loop_state: &mut LoopState,
    ) -> Result<()> {
        loop {
            tokio::select! {
                // Send message based on rate limit
                _ = message_timer.tick() => {
                    if loop_state.check_termination_conditions(self.core.client_id) {
                        break;
                    }

                    if let Err(e) = self.send_message(tx_send, &mut loop_state.messages_sent).await {
                        error!("Client {} send error: {}", self.core.client_id, e);
                        break;
                    }
                }

                // Process received messages
                Some(received_json) = rx_received.recv() => {
                    self.handle_received_json(received_json).await;
                }

                // Check for timeouts periodically
                _ = timeout_check_timer.tick() => {
                    self.handle_timeouts().await;
                }

                // Handle shutdown signal
                _ = self.core.shutdown_rx.recv() => {
                    info!("Client {} received shutdown signal", self.core.client_id);
                    break;
                }
            }
        }
        Ok(())
    }

    async fn run_unlimited_rate_loop(
        &mut self,
        mut timeout_check_timer: tokio::time::Interval,
        tx_send: &mpsc::Sender<String>,
        rx_received: &mut mpsc::Receiver<String>,
        loop_state: &mut LoopState,
    ) -> Result<()> {
        let mut should_break = false;
        loop {
            // Check termination conditions first
            if loop_state.check_termination_conditions(self.core.client_id) {
                break;
            }

            // Send message immediately
            if let Err(e) = self
                .send_message(tx_send, &mut loop_state.messages_sent)
                .await
            {
                error!("Client {} send error: {}", self.core.client_id, e);
                break;
            }

            // Check for other events (non-blocking)
            tokio::select! {
                // Process received messages
                Some(received_json) = rx_received.recv() => {
                    self.handle_received_json(received_json).await;
                }

                // Check for timeouts periodically
                _ = timeout_check_timer.tick() => {
                    self.handle_timeouts().await;
                }

                // Handle shutdown signal
                _ = self.core.shutdown_rx.recv() => {
                    info!("Client {} received shutdown signal", self.core.client_id);
                    should_break = true;
                }

                // Continue immediately if no events are ready
                _ = tokio::time::sleep(Duration::from_nanos(1)) => {
                    // Continue to next iteration immediately
                }
            }

            if should_break {
                break;
            }
        }
        Ok(())
    }

    /// Send a single message with all the required tracking and metrics
    async fn send_message(
        &self,
        tx_send: &mpsc::Sender<String>,
        messages_sent: &mut u64,
    ) -> Result<()> {
        // Create payload with resolved <rnd> values
        let payload_json = self.core.config.resolve_payload();
        let message_id = Config::extract_message_id_typed(&payload_json)
            .with_message_context("Failed to extract message ID from payload")?;

        // Handle mode-specific tracking
        match self.core.config.ws_mode().unwrap_or(WSMode::Echo) {
            WSMode::Echo => {
                self.handle_echo_mode_send(&message_id).await?;
            }
            WSMode::Broadcast => {
                self.handle_broadcast_mode_send(&message_id).await;
            }
        }

        // Send payload JSON
        debug!(
            "Client {} sending message with ID: {}",
            self.core.client_id, message_id
        );
        if tx_send.send(payload_json).await.is_err() {
            return Err(BombError::execution(format!(
                "Client {} failed to send message to internal sender task (channel closed)",
                self.core.client_id
            )));
        }

        // Mark that we've started sending (for the first message only)
        {
            let mut has_started = self.has_started_sending.lock().await;
            if !*has_started {
                *has_started = true;
                debug!("Client {} started sending messages", self.core.client_id);
            }
        }

        *messages_sent += 1;
        self.core
            .metrics
            .record_message_sent(self.core.client_id)
            .await;

        Ok(())
    }

    /// Handle message sending in echo mode with pending message tracking
    async fn handle_echo_mode_send(&self, message_id: &MessageId) -> Result<()> {
        let mut pending = self.core.pending_messages.lock().await;

        // Prevent memory explosion - limit pending messages
        if pending.len() >= self.core.config.client.max_pending_messages {
            warn!(
                "Client {} memory protection: {} pending messages (limit {}), dropping oldest",
                self.core.client_id,
                pending.len(),
                self.core.config.client.max_pending_messages
            );

            // Find and remove oldest message more efficiently
            let oldest_key = pending
                .iter()
                .min_by_key(|(_, p)| p.sent_at)
                .map(|(k, _)| k.clone());

            if let Some(key) = oldest_key {
                debug!(
                    "Client {} dropping oldest message {} - any future server response will be 'unexpected'",
                    self.core.client_id, key
                );
                pending.remove(&key);
                // Record failure for dropped message
                self.core
                    .metrics
                    .record_message_failure(self.core.client_id)
                    .await;
            }
        }

        pending.insert(
            message_id.clone(),
            PendingMessage::new(self.core.config.timeout_duration()),
        );
        Ok(())
    }

    /// Handle message sending in broadcast mode with tracker registration
    async fn handle_broadcast_mode_send(&self, message_id: &MessageId) {
        if let Some(ref tracker) = self.broadcast_tracker {
            tracker
                .register_sent(message_id.clone(), self.core.client_id)
                .await;
        }
    }

    /// Wait for pending responses using dynamic timeout based on latest sent message
    async fn wait_for_pending_responses(
        &mut self,
        mut rx_received: mpsc::Receiver<String>,
    ) -> Result<()> {
        match self.core.config.ws_mode().unwrap_or(WSMode::Echo) {
            WSMode::Echo => self.wait_for_echo_responses(&mut rx_received).await,
            WSMode::Broadcast => {
                info!(
                    "Client {} in broadcast mode - using broadcast finalization",
                    self.core.client_id
                );
                // In broadcast mode, trigger final finalization before exit
                self.handle_broadcast_finalization().await;
                Ok(())
            }
        }
    }

    /// Wait for pending responses in echo mode
    async fn wait_for_echo_responses(
        &mut self,
        rx_received: &mut mpsc::Receiver<String>,
    ) -> Result<()> {
        let initial_pending_count = self.core.pending_messages.lock().await.len();
        info!(
            "Client {} waiting for {} pending responses...",
            self.core.client_id, initial_pending_count
        );

        loop {
            let latest_timeout = self.cleanup_expired_and_get_latest_timeout().await;

            // If no messages remain or all timed out, exit
            if let Some(timeout) = latest_timeout {
                let now = Instant::now();
                if now >= timeout {
                    debug!(
                        "Client {} all pending messages have timed out",
                        self.core.client_id
                    );
                    break;
                }

                if !self.wait_for_next_event(rx_received, timeout).await {
                    break; // Shutdown signal received
                }
            } else {
                break; // No pending messages
            }
        }

        self.cleanup_remaining_messages().await;
        Ok(())
    }

    async fn cleanup_expired_and_get_latest_timeout(&self) -> Option<Instant> {
        let mut pending = self.core.pending_messages.lock().await;
        let now = Instant::now();

        // Remove expired messages first
        let mut expired_ids = Vec::new();
        for (id, message) in pending.iter() {
            if now >= message.timeout_at {
                expired_ids.push(id.clone());
            }
        }

        // Record failures for expired messages
        for id in &expired_ids {
            pending.remove(id);
            self.core
                .metrics
                .record_message_failure(self.core.client_id)
                .await;
        }

        if !expired_ids.is_empty() {
            debug!(
                "Client {} cleaned up {} expired messages",
                self.core.client_id,
                expired_ids.len()
            );
        }

        if pending.is_empty() {
            return None;
        }

        // Find the latest timeout among remaining messages
        pending.values().map(|msg| msg.timeout_at).max()
    }

    async fn wait_for_next_event(
        &mut self,
        rx_received: &mut mpsc::Receiver<String>,
        latest_timeout: Instant,
    ) -> bool {
        tokio::select! {
            Some(received_json) = rx_received.recv() => {
                self.handle_received_json(received_json).await;
                true
            }
            _ = tokio::time::sleep_until(latest_timeout.into()) => {
                debug!("Client {} timeout deadline reached", self.core.client_id);
                true
            }
            _ = self.core.shutdown_rx.recv() => {
                info!("Client {} received shutdown signal during pending wait, exiting early", self.core.client_id);
                false
            }
        }
    }

    async fn cleanup_remaining_messages(&self) {
        let mut pending = self.core.pending_messages.lock().await;
        let remaining = pending.len();
        if remaining > 0 {
            warn!(
                "Client {} has {} remaining unresponded messages",
                self.core.client_id, remaining
            );
            for _ in 0..remaining {
                self.core
                    .metrics
                    .record_message_failure(self.core.client_id)
                    .await;
            }
            pending.clear();
        }
    }

    /// Handle a received JSON by extracting ID and matching it to pending messages
    async fn handle_received_json(&self, received_json: String) {
        // Check if we've started sending messages yet - ignore responses until then
        {
            let has_started = self.has_started_sending.lock().await;
            if !*has_started {
                debug!(
                    "Client {} ignoring response before sending started: {}",
                    self.core.client_id, received_json
                );
                return;
            }
        }

        // Extract message ID from JSON
        let message_id = match Config::extract_message_id_typed(&received_json) {
            Ok(id) => id,
            Err(e) => {
                warn!(
                    "Client {} failed to extract ID from received JSON: {} (JSON: {})",
                    self.core.client_id, e, received_json
                );
                return;
            }
        };

        match self.core.config.ws_mode().unwrap_or(WSMode::Echo) {
            WSMode::Echo => {
                self.handle_echo_mode_receive(&message_id).await;
            }
            WSMode::Broadcast => {
                self.handle_broadcast_mode_receive(&message_id).await;
            }
        }
    }

    /// Handle message reception in echo mode
    async fn handle_echo_mode_receive(&self, message_id: &MessageId) {
        let rtt_result = {
            let mut pending = self.core.pending_messages.lock().await;
            pending
                .remove(message_id)
                .map(|pending_msg| pending_msg.calculate_rtt(Instant::now()))
        };

        if let Some(rtt) = rtt_result {
            self.core
                .metrics
                .record_message_success(self.core.client_id, rtt)
                .await;
            debug!(
                "Client {} received response for message with ID: {} (RTT: {:?})",
                self.core.client_id, message_id, rtt
            );
        } else {
            debug!(
                "Client {} received unexpected message with ID: {} (likely server response delay or memory protection cleanup)",
                self.core.client_id, message_id
            );
        }
    }

    /// Handle message reception in broadcast mode
    async fn handle_broadcast_mode_receive(&self, message_id: &MessageId) {
        if let Some(ref tracker) = self.broadcast_tracker {
            if let Some(sender_id) = tracker
                .record_response(message_id, self.core.client_id)
                .await
            {
                // Record success for ALL messages received in broadcast mode
                // We'll normalize the success rate in the metrics calculation
                let rtt = Duration::from_millis(50); // Placeholder RTT for broadcast mode
                self.core
                    .metrics
                    .record_message_success(self.core.client_id, rtt)
                    .await;

                if sender_id == self.core.client_id {
                    debug!(
                        "Client {} received own broadcast message with ID: {} (recorded success)",
                        self.core.client_id, message_id
                    );
                } else {
                    debug!(
                        "Client {} received broadcast message from client {} with ID: {} (recorded success)",
                        self.core.client_id, sender_id, message_id
                    );
                }
            } else {
                debug!(
                    "Client {} received untracked broadcast message with ID: {} (likely from external source or timeout)",
                    self.core.client_id, message_id
                );
            }
        } else {
            warn!(
                "Client {} in broadcast mode but no broadcast tracker available",
                self.core.client_id
            );
        }
    }

    /// Check for and handle message timeouts
    async fn handle_timeouts(&self) {
        match self.core.config.ws_mode().unwrap_or(WSMode::Echo) {
            WSMode::Echo => {
                self.handle_echo_timeouts().await;
            }
            WSMode::Broadcast => {
                self.handle_broadcast_finalization().await;
            }
        }
    }

    /// Handle timeouts in echo mode
    async fn handle_echo_timeouts(&self) {
        let timed_out_count = {
            let mut pending = self.core.pending_messages.lock().await;
            let now = Instant::now();

            let timed_out_ids: Vec<MessageId> = pending
                .iter()
                .filter_map(|(id, pending_msg)| {
                    if pending_msg.timeout_at <= now {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let count = timed_out_ids.len();
            if count > 0 {
                warn!(
                    "Client {} cleaning up {} timed-out messages (server response delay > {}s)",
                    self.core.client_id,
                    count,
                    self.core.config.client.timeout.as_secs()
                );
            }
            for id in timed_out_ids {
                if pending.remove(&id).is_some() {
                    debug!(
                        "Client {} message {} timed out - any future server response will be 'unexpected'",
                        self.core.client_id, id
                    );
                }
            }

            count
        };

        // Record metrics outside of lock scope
        for _ in 0..timed_out_count {
            self.core
                .metrics
                .record_message_failure(self.core.client_id)
                .await;
        }
    }

    /// Handle broadcast message finalization
    async fn handle_broadcast_finalization(&self) {
        if let Some(ref tracker) = self.broadcast_tracker {
            debug!(
                "Client {} starting broadcast finalization",
                self.core.client_id
            );
            let finalized_metrics = tracker.finalize_ready_messages().await;
            debug!(
                "Client {} finalized {} broadcast entries (cleanup only)",
                self.core.client_id,
                finalized_metrics.len()
            );

            // In the new approach, we don't record metrics during finalization
            // Individual clients already recorded their success when receiving messages
        } else {
            debug!(
                "Client {} no broadcast tracker available for finalization",
                self.core.client_id
            );
        }
    }
}
