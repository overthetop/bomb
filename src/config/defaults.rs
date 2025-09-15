//! Default values and configuration presets

use super::{
    ClientConfig, Config, HttpMethod, OutputConfig, ProtocolConfig, TargetConfig, TestConfig,
    WSMode,
};
use std::time::Duration;

/// Default configuration values
#[allow(dead_code)]
pub struct Defaults;

#[allow(dead_code)]
impl Defaults {
    pub const CLIENT_COUNT: u32 = 10;
    pub const MESSAGE_RATE: u32 = 100;
    pub const TIMEOUT_SECONDS: u64 = 30;
    pub const MAX_PENDING_MESSAGES: usize = 6000;
    pub const DEFAULT_HTTP_PAYLOAD: &'static str =
        r#"{"message": "Hello from Bomb!", "timestamp": "<rnd:ts>"}"#;
    pub const DEFAULT_WS_PAYLOAD: &'static str = r#"{"id": "<rnd:uuid>"}"#;
}

#[allow(dead_code)]
impl Config {
    /// Create a default WebSocket configuration
    pub fn default_websocket(target_url: String) -> Self {
        Self {
            target: TargetConfig {
                url: target_url,
                insecure: false,
                headers: vec![],
            },
            client: ClientConfig {
                count: Defaults::CLIENT_COUNT,
                timeout: Duration::from_secs(Defaults::TIMEOUT_SECONDS),
                max_pending_messages: Defaults::MAX_PENDING_MESSAGES,
            },
            test: TestConfig {
                duration: None,
                total_messages: None,
                message_rate: Defaults::MESSAGE_RATE,
                payload: Defaults::DEFAULT_WS_PAYLOAD.to_string(),
            },
            output: OutputConfig { verbose: false },
            protocol: ProtocolConfig::WebSocket { mode: WSMode::Echo },
        }
    }

    /// Create a default HTTP configuration
    pub fn default_http(target_url: String) -> Self {
        Self {
            target: TargetConfig {
                url: target_url,
                insecure: false,
                headers: vec![],
            },
            client: ClientConfig {
                count: Defaults::CLIENT_COUNT,
                timeout: Duration::from_secs(Defaults::TIMEOUT_SECONDS),
                max_pending_messages: Defaults::MAX_PENDING_MESSAGES,
            },
            test: TestConfig {
                duration: None,
                total_messages: None,
                message_rate: Defaults::MESSAGE_RATE,
                payload: Defaults::DEFAULT_HTTP_PAYLOAD.to_string(),
            },
            output: OutputConfig { verbose: false },
            protocol: ProtocolConfig::Http {
                method: HttpMethod::Get,
            },
        }
    }

    /// Create a quick test configuration (short duration, few clients)
    pub fn quick_test(target_url: String, is_websocket: bool) -> Self {
        let mut config = if is_websocket {
            Self::default_websocket(target_url)
        } else {
            Self::default_http(target_url)
        };

        config.client.count = 2;
        config.test.duration = Some(Duration::from_secs(10));
        config.test.message_rate = 5;

        config
    }

    /// Create a load test configuration (many clients, longer duration)
    pub fn load_test(target_url: String, is_websocket: bool) -> Self {
        let mut config = if is_websocket {
            Self::default_websocket(target_url)
        } else {
            Self::default_http(target_url)
        };

        config.client.count = 100;
        config.test.duration = Some(Duration::from_secs(300)); // 5 minutes
        config.test.message_rate = 50;

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_websocket_config() {
        let config = Config::default_websocket("ws://localhost:8080".to_string());
        assert_eq!(config.client.count, Defaults::CLIENT_COUNT);
        assert_eq!(config.test.message_rate, Defaults::MESSAGE_RATE);
        assert_eq!(config.client.timeout.as_secs(), Defaults::TIMEOUT_SECONDS);
        assert!(matches!(config.protocol, ProtocolConfig::WebSocket { .. }));
    }

    #[test]
    fn test_default_http_config() {
        let config = Config::default_http("http://localhost:8080".to_string());
        assert_eq!(config.client.count, Defaults::CLIENT_COUNT);
        assert_eq!(config.test.message_rate, Defaults::MESSAGE_RATE);
        assert!(matches!(config.protocol, ProtocolConfig::Http { .. }));
    }

    #[test]
    fn test_quick_test_config() {
        let config = Config::quick_test("ws://localhost:8080".to_string(), true);
        assert_eq!(config.client.count, 2);
        assert_eq!(config.test.duration, Some(Duration::from_secs(10)));
        assert_eq!(config.test.message_rate, 5);
    }

    #[test]
    fn test_load_test_config() {
        let config = Config::load_test("ws://localhost:8080".to_string(), true);
        assert_eq!(config.client.count, 100);
        assert_eq!(config.test.duration, Some(Duration::from_secs(300)));
        assert_eq!(config.test.message_rate, 50);
    }
}
