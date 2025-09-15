//! Command-line argument parsing for Bomb configuration

use clap::{Parser, ValueEnum};
use std::time::Duration;

use super::{
    ClientConfig, Config, HttpMethod, OutputConfig, ProtocolConfig, TargetConfig, TestConfig,
    WSMode,
};
use crate::errors::{BombError, Result};

/// Connection protocol mode for CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConnectionModeArg {
    /// HTTP protocol mode
    Http,
    /// WebSocket protocol mode
    Ws,
}

/// WebSocket server behavior mode for CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WSModeArg {
    /// Server echoes messages back to sender (default)
    Echo,
    /// Server broadcasts messages to all connected clients
    Broadcast,
}

/// HTTP method for CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HttpMethodArg {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

/// Raw configuration from command line arguments
#[derive(Parser, Debug, Clone)]
#[command(
    name = "bomb",
    version = "0.1.0",
    about = "A configurable HTTP and WebSocket stress-testing tool that spawns multiple concurrent clients",
    long_about = None
)]
pub struct RawConfig {
    /// Target URL for stress testing (HTTP or WebSocket)
    #[arg(
        short = 't',
        long = "target",
        value_name = "URL",
        help = "Target URL to stress test (supports HTTP/HTTPS/WS/WSS protocols)"
    )]
    pub target: String,

    /// Number of concurrent clients
    #[arg(
        short = 'c',
        long = "clients",
        value_name = "COUNT",
        default_value = "10",
        help = "Number of concurrent clients to spawn"
    )]
    pub clients: u32,

    /// Duration of the stress test in seconds
    #[arg(
        short = 'd',
        long = "duration",
        value_name = "SECONDS",
        help = "Duration of the test (e.g., '30s', '5m', '2h')",
        conflicts_with = "total_messages"
    )]
    pub duration: Option<String>,

    /// Total number of messages to send
    #[arg(
        short = 'n',
        long = "total-messages",
        value_name = "COUNT",
        help = "Total number of messages to send across all clients",
        conflicts_with = "duration"
    )]
    pub total_messages: Option<u64>,

    /// Message sending rate per client
    #[arg(
        short = 'r',
        long = "message-rate",
        value_name = "RATE",
        default_value = "100",
        help = "Messages per second per client (0 = unlimited)"
    )]
    pub message_rate: u32,

    /// Connection timeout in seconds
    #[arg(
        long = "timeout",
        value_name = "SECONDS",
        default_value = "30",
        help = "Connection timeout in seconds"
    )]
    pub timeout: u64,

    /// Maximum pending messages per client
    #[arg(
        long = "max-pending",
        value_name = "COUNT",
        default_value = "6000",
        help = "Maximum pending messages per client (memory protection)"
    )]
    pub max_pending: usize,

    /// Custom headers
    #[arg(
        short = 'H',
        long = "header",
        value_name = "HEADER",
        action = clap::ArgAction::Append,
        help = "Custom headers in 'Key: Value' format (can be used multiple times)"
    )]
    pub headers: Vec<String>,

    /// Allow insecure connections
    #[arg(
        long = "insecure",
        help = "Allow insecure TLS connections (skip certificate verification)"
    )]
    pub insecure: bool,

    /// Enable verbose logging
    #[arg(short = 'v', long = "verbose", help = "Enable verbose logging")]
    pub verbose: bool,

    /// Custom JSON payload
    #[arg(
        short = 'p',
        long = "payload",
        value_name = "JSON",
        help = "JSON payload to send (WebSocket mode requires 'id' field, HTTP mode does not)"
    )]
    pub payload: Option<String>,

    /// Connection protocol mode
    #[arg(
        short = 'm',
        long = "mode",
        value_enum,
        default_value = "ws",
        help = "Connection protocol mode"
    )]
    pub mode: ConnectionModeArg,

    /// WebSocket mode
    #[arg(
        long = "ws-mode",
        value_enum,
        default_value = "echo",
        help = "WebSocket behavior mode (only used in WebSocket mode)"
    )]
    pub ws_mode: WSModeArg,

    /// HTTP method
    #[arg(
        long = "http-method",
        value_enum,
        default_value = "get",
        help = "HTTP method for stress testing (only used in HTTP mode)"
    )]
    pub http_method: HttpMethodArg,
}

impl RawConfig {
    /// Parse from command line arguments
    pub fn parse_from_args() -> Result<Self> {
        Ok(Self::parse())
    }

    /// Parse duration string with time suffixes (s/m/h)
    fn parse_duration(duration_str: &str) -> Result<Duration> {
        let duration_str = duration_str.trim();

        if duration_str.is_empty() {
            return Err(BombError::config("Duration cannot be empty"));
        }

        // Check if it ends with a suffix
        if let Some(last_char) = duration_str.chars().last() {
            match last_char {
                's' | 'S' => {
                    let number_part = &duration_str[..duration_str.len() - 1];
                    let seconds = number_part.parse::<u64>().map_err(|_| {
                        BombError::config(format!(
                            "Invalid duration format: '{}' - expected number before 's'",
                            duration_str
                        ))
                    })?;
                    Ok(Duration::from_secs(seconds))
                }
                'm' | 'M' => {
                    let number_part = &duration_str[..duration_str.len() - 1];
                    let minutes = number_part.parse::<u64>().map_err(|_| {
                        BombError::config(format!(
                            "Invalid duration format: '{}' - expected number before 'm'",
                            duration_str
                        ))
                    })?;
                    Ok(Duration::from_secs(minutes * 60))
                }
                'h' | 'H' => {
                    let number_part = &duration_str[..duration_str.len() - 1];
                    let hours = number_part.parse::<u64>().map_err(|_| {
                        BombError::config(format!(
                            "Invalid duration format: '{}' - expected number before 'h'",
                            duration_str
                        ))
                    })?;
                    Ok(Duration::from_secs(hours * 3600))
                }
                _ => {
                    // No suffix, assume seconds
                    let seconds = duration_str.parse::<u64>().map_err(|_| {
                        BombError::config(format!(
                            "Invalid duration format: '{}' - expected a number",
                            duration_str
                        ))
                    })?;
                    Ok(Duration::from_secs(seconds))
                }
            }
        } else {
            Err(BombError::config("Duration cannot be empty"))
        }
    }
}

impl TryFrom<RawConfig> for Config {
    type Error = BombError;

    fn try_from(raw: RawConfig) -> Result<Self> {
        // Parse duration if provided
        let duration = if let Some(duration_str) = &raw.duration {
            Some(RawConfig::parse_duration(duration_str)?)
        } else {
            None
        };

        // Determine protocol configuration
        let protocol = match raw.mode {
            ConnectionModeArg::Http => ProtocolConfig::Http {
                method: match raw.http_method {
                    HttpMethodArg::Get => HttpMethod::Get,
                    HttpMethodArg::Post => HttpMethod::Post,
                    HttpMethodArg::Put => HttpMethod::Put,
                    HttpMethodArg::Delete => HttpMethod::Delete,
                    HttpMethodArg::Patch => HttpMethod::Patch,
                },
            },
            ConnectionModeArg::Ws => ProtocolConfig::WebSocket {
                mode: match raw.ws_mode {
                    WSModeArg::Echo => WSMode::Echo,
                    WSModeArg::Broadcast => WSMode::Broadcast,
                },
            },
        };

        // Set default payload based on protocol if not provided
        let payload = if let Some(p) = raw.payload {
            p
        } else {
            match &protocol {
                ProtocolConfig::Http { .. } => {
                    r#"{"message": "Hello from Bomb!", "timestamp": "<rnd:ts>"}"#.to_string()
                }
                ProtocolConfig::WebSocket { .. } => r#"{"id": "<rnd:uuid>"}"#.to_string(),
            }
        };

        Ok(Config {
            target: TargetConfig {
                url: raw.target,
                insecure: raw.insecure,
                headers: raw.headers,
            },
            client: ClientConfig {
                count: raw.clients,
                timeout: Duration::from_secs(raw.timeout),
                max_pending_messages: raw.max_pending,
            },
            test: TestConfig {
                duration,
                total_messages: raw.total_messages,
                message_rate: raw.message_rate,
                payload,
            },
            output: OutputConfig {
                verbose: raw.verbose,
            },
            protocol,
        })
    }
}
