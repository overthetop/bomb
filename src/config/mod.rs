//! Configuration management for Bomb stress testing tool
//!
//! This module provides a clean, layered approach to configuration:
//! - Core structures and enums
//! - CLI argument parsing
//! - Configuration validation
//! - Default value management

pub mod defaults;
pub mod parser;
pub mod validation;

use crate::errors::{BombError, Result};
use std::time::Duration;

/// Connection protocol mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// HTTP protocol mode
    Http,
    /// WebSocket protocol mode
    Ws,
}

/// WebSocket server behavior mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WSMode {
    /// Server echoes messages back to sender (default)
    Echo,
    /// Server broadcasts messages to all connected clients
    Broadcast,
}

/// HTTP method for stress testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

/// Target configuration
#[derive(Debug, Clone)]
pub struct TargetConfig {
    pub url: String,
    pub insecure: bool,
    pub headers: Vec<String>,
}

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub count: u32,
    pub timeout: Duration,
    pub max_pending_messages: usize,
}

/// Test execution configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub duration: Option<Duration>,
    pub total_messages: Option<u64>,
    pub message_rate: u32,
    pub payload: String,
}

/// Output configuration
#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub verbose: bool,
}

/// Protocol-specific configuration
#[derive(Debug, Clone)]
pub enum ProtocolConfig {
    Http { method: HttpMethod },
    WebSocket { mode: WSMode },
}

/// Main configuration structure
#[derive(Debug, Clone)]
pub struct Config {
    pub target: TargetConfig,
    pub client: ClientConfig,
    pub test: TestConfig,
    pub output: OutputConfig,
    pub protocol: ProtocolConfig,
}

impl Config {
    /// Parse and validate configuration from command line arguments
    pub fn from_args() -> Result<Self> {
        let raw_config = parser::RawConfig::parse_from_args()?;
        let config = raw_config.try_into()?;
        validation::validate(&config)?;
        Ok(config)
    }

    /// Get the target URL with random patterns resolved
    pub fn resolve_target_url(&self) -> String {
        crate::patterns::resolve(&self.target.url)
    }

    /// Get the payload with random patterns resolved
    pub fn resolve_payload(&self) -> String {
        crate::patterns::resolve(&self.test.payload)
    }

    /// Extract message ID from a JSON payload string
    pub fn extract_message_id(payload_json: &str) -> Result<String> {
        let json: serde_json::Value = serde_json::from_str(payload_json)?;
        json["id"]
            .as_str()
            .ok_or_else(|| BombError::config("Missing or invalid 'id' field in payload"))
            .map(|s| s.to_string())
    }

    /// Extract message ID as a strongly-typed MessageId
    pub fn extract_message_id_typed(payload_json: &str) -> Result<crate::message::MessageId> {
        Self::extract_message_id(payload_json).map(crate::message::MessageId::from)
    }

    /// Get the connection mode
    pub fn connection_mode(&self) -> ConnectionMode {
        match &self.protocol {
            ProtocolConfig::Http { .. } => ConnectionMode::Http,
            ProtocolConfig::WebSocket { .. } => ConnectionMode::Ws,
        }
    }

    /// Get custom headers as key-value pairs
    pub fn custom_headers(&self) -> Result<Vec<(String, String)>> {
        let mut headers = Vec::new();
        for header in &self.target.headers {
            if let Some((key, value)) = header.split_once(':') {
                headers.push((key.trim().to_string(), value.trim().to_string()));
            } else {
                return Err(BombError::config(format!(
                    "Invalid header format '{}'. Use 'Key: Value' format",
                    header
                )));
            }
        }
        Ok(headers)
    }

    /// Get the test duration as Duration
    pub fn test_duration(&self) -> Option<Duration> {
        self.test.duration
    }

    /// Get total messages per client
    pub fn messages_per_client(&self) -> Option<u64> {
        self.test
            .total_messages
            .map(|total| total / self.client.count as u64)
    }

    /// Get HTTP method from protocol config
    pub fn http_method(&self) -> Option<HttpMethod> {
        match &self.protocol {
            ProtocolConfig::Http { method } => Some(*method),
            _ => None,
        }
    }

    /// Get WebSocket mode from protocol config
    pub fn ws_mode(&self) -> Option<WSMode> {
        match &self.protocol {
            ProtocolConfig::WebSocket { mode } => Some(*mode),
            _ => None,
        }
    }

    /// Get message sending interval for rate limiting
    pub fn message_interval(&self) -> Option<Duration> {
        if self.test.message_rate > 0 {
            Some(Duration::from_millis(1000 / self.test.message_rate as u64))
        } else {
            None
        }
    }

    /// Get timeout as Duration
    pub fn timeout_duration(&self) -> Duration {
        self.client.timeout
    }

    /// Print configuration summary
    pub fn print_summary(&self) {
        println!("💣 Bomb Stress Test Configuration:");
        println!("   Target:           {}", self.target.url);
        println!("   Clients:          {}", self.client.count);

        match (self.test.duration, self.test.total_messages) {
            (Some(duration), None) => {
                println!("   Duration:         {}s", duration.as_secs());
            }
            (None, Some(total)) => {
                let per_client = self.messages_per_client().unwrap_or(0);
                println!("   Total Messages:   {}", total);
                println!("   Per Client:       {} messages", per_client);
            }
            _ => {}
        }

        println!(
            "   Message Rate:     {} msg/s per client",
            self.test.message_rate
        );
        println!("   Timeout:          {}s", self.client.timeout.as_secs());

        match &self.protocol {
            ProtocolConfig::Http { method } => {
                println!("   HTTP Method:      {:?}", method);
                println!("   Connection Mode:  Http");
            }
            ProtocolConfig::WebSocket { mode } => {
                println!("   Mode:             {:?}", mode);
                println!("   Connection Mode:  Ws");
            }
        }

        let payload_label = match self.connection_mode() {
            ConnectionMode::Ws => "Payload:",
            ConnectionMode::Http => "Request Body:",
        };

        if self.test.payload.len() > 50 {
            println!(
                "   {}          {}... ({} chars)",
                payload_label,
                &self.test.payload[..47],
                self.test.payload.len()
            );
        } else {
            println!("   {}          {}", payload_label, self.test.payload);
        }

        if !self.target.headers.is_empty() {
            println!("   Custom Headers:   {}", self.target.headers.len());
            for header in &self.target.headers {
                println!("                     {}", header);
            }
        }

        if self.target.insecure {
            println!("   Security:         ⚠️  Insecure mode enabled");
        }

        println!();
    }
}
