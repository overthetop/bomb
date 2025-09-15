//! Configuration validation logic

use super::{Config, ConnectionMode, ProtocolConfig};
use crate::constants::MAX_CLIENTS_LIMIT;
use crate::errors::{BombError, Result};
use url::Url;

/// Validate the configuration
pub fn validate(config: &Config) -> Result<()> {
    validate_target(config)?;
    validate_client_config(config)?;
    validate_test_config(config)?;
    validate_payload(config)?;
    validate_headers(config)?;
    Ok(())
}

/// Validate target configuration
fn validate_target(config: &Config) -> Result<()> {
    let url = Url::parse(&config.target.url).map_err(|e| {
        BombError::config(format!("Invalid target URL '{}': {}", config.target.url, e))
    })?;

    // Validate scheme based on connection mode
    match config.connection_mode() {
        ConnectionMode::Ws => match url.scheme() {
            "ws" | "wss" => Ok(()),
            scheme => Err(BombError::config(format!(
                "Invalid URL scheme '{}' for WebSocket mode. Only 'ws' and 'wss' are supported",
                scheme
            ))),
        },
        ConnectionMode::Http => match url.scheme() {
            "http" | "https" => Ok(()),
            scheme => Err(BombError::config(format!(
                "Invalid URL scheme '{}' for HTTP mode. Only 'http' and 'https' are supported",
                scheme
            ))),
        },
    }
}

/// Validate client configuration
fn validate_client_config(config: &Config) -> Result<()> {
    if config.client.count == 0 {
        return Err(BombError::config(
            "Number of clients must be greater than 0",
        ));
    }

    if config.client.count > MAX_CLIENTS_LIMIT {
        return Err(BombError::config(format!(
            "Number of clients cannot exceed {}",
            MAX_CLIENTS_LIMIT
        )));
    }

    if config.client.timeout.is_zero() {
        return Err(BombError::config("Timeout must be greater than 0"));
    }

    Ok(())
}

/// Validate test configuration
fn validate_test_config(config: &Config) -> Result<()> {
    // Ensure at least one termination condition is specified
    if config.test.duration.is_none() && config.test.total_messages.is_none() {
        return Err(BombError::config(
            "Must specify either --duration or --total-messages to define test termination",
        ));
    }

    if let Some(duration) = config.test.duration
        && duration.is_zero()
    {
        return Err(BombError::config("Duration must be greater than 0"));
    }

    if let Some(total_messages) = config.test.total_messages
        && total_messages == 0
    {
        return Err(BombError::config("Total messages must be greater than 0"));
    }

    Ok(())
}

/// Validate payload configuration
fn validate_payload(config: &Config) -> Result<()> {
    // Validate JSON payload format
    let json: serde_json::Value = serde_json::from_str(&config.test.payload).map_err(|_| {
        BombError::config(format!(
            "Invalid JSON payload format: '{}'",
            config.test.payload
        ))
    })?;

    if !json.is_object() {
        return Err(BombError::config("Payload must be a JSON object"));
    }

    // Only require 'id' field for WebSocket mode (needed for echo tracking)
    // HTTP mode doesn't require id field
    match &config.protocol {
        ProtocolConfig::WebSocket { .. } => {
            if let Some(obj) = json.as_object() {
                if !obj.contains_key("id") {
                    return Err(BombError::config(
                        "Payload JSON must contain an 'id' field for WebSocket mode",
                    ));
                }
                // Validate that id field is a string
                if !json["id"].is_string() {
                    return Err(BombError::config("Payload 'id' field must be a string"));
                }
            }
        }
        ProtocolConfig::Http { .. } => {
            // HTTP mode doesn't require any specific fields
        }
    }

    Ok(())
}

/// Validate custom headers format
fn validate_headers(config: &Config) -> Result<()> {
    for header in &config.target.headers {
        if !header.contains(':') {
            return Err(BombError::config(format!(
                "Invalid header format '{}'. Use 'Key: Value' format",
                header
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_config() -> Config {
        Config {
            target: super::super::TargetConfig {
                url: "ws://localhost:8080".to_string(),
                insecure: false,
                headers: vec![],
            },
            client: super::super::ClientConfig {
                count: 1,
                timeout: Duration::from_secs(30),
                max_pending_messages: 1000,
            },
            test: super::super::TestConfig {
                duration: Some(Duration::from_secs(10)),
                total_messages: None,
                message_rate: 10,
                payload: r#"{"id": "test"}"#.to_string(),
            },
            output: super::super::OutputConfig { verbose: false },
            protocol: ProtocolConfig::WebSocket {
                mode: super::super::WSMode::Echo,
            },
        }
    }

    #[test]
    fn test_validate_valid_config() {
        let config = create_test_config();
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_url() {
        let mut config = create_test_config();
        config.target.url = "invalid-url".to_string();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_zero_clients() {
        let mut config = create_test_config();
        config.client.count = 0;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_no_termination_condition() {
        let mut config = create_test_config();
        config.test.duration = None;
        config.test.total_messages = None;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_json_payload() {
        let mut config = create_test_config();
        config.test.payload = "invalid json".to_string();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_websocket_missing_id() {
        let mut config = create_test_config();
        config.test.payload = r#"{"data": "test"}"#.to_string();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_http_no_id_required() {
        let mut config = create_test_config();
        config.protocol = ProtocolConfig::Http {
            method: super::super::HttpMethod::Post,
        };
        config.target.url = "http://localhost:8080".to_string();
        config.test.payload = r#"{"data": "test"}"#.to_string();
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_header_format() {
        let mut config = create_test_config();
        config.target.headers = vec!["InvalidHeader".to_string()];
        assert!(validate(&config).is_err());
    }
}
