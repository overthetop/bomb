//! Domain-specific error types for Bomb stress testing tool
//!
//! This module provides structured error types using `thiserror` for
//! precise and ergonomic error handling throughout the application.

use thiserror::Error;

/// Main error type for the Bomb application
#[derive(Error, Debug)]
pub enum BombError {
    /// Configuration-related errors (CLI parsing, validation, etc.)
    #[error("Configuration error: {0}")]
    Config(String),

    /// Network transport errors (connection, protocol, etc.)
    #[error("Transport error: {0}")]
    Transport(String),

    /// Test execution errors (client failures, timeout issues, etc.)
    #[error("Test execution error: {0}")]
    TestExecution(String),

    /// Message processing errors (serialization, ID extraction, etc.)
    #[error("Message error: {0}")]
    Message(String),

    /// URL parsing errors
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// HTTP request errors
    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] reqwest::Error),

    /// JSON serialization errors
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    /// WebSocket errors
    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
}

/// Result type using BombError
pub type Result<T> = std::result::Result<T, BombError>;

/// Helper trait for adding context to errors
pub trait ErrorContext<T> {
    fn with_config_context(self, msg: &str) -> Result<T>;
    fn with_transport_context(self, msg: &str) -> Result<T>;
    fn with_message_context(self, msg: &str) -> Result<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::fmt::Display,
{
    fn with_config_context(self, msg: &str) -> Result<T> {
        self.map_err(|e| BombError::Config(format!("{}: {}", msg, e)))
    }

    fn with_transport_context(self, msg: &str) -> Result<T> {
        self.map_err(|e| BombError::Transport(format!("{}: {}", msg, e)))
    }

    fn with_message_context(self, msg: &str) -> Result<T> {
        self.map_err(|e| BombError::Message(format!("{}: {}", msg, e)))
    }
}

impl<T> ErrorContext<T> for Option<T> {
    fn with_config_context(self, msg: &str) -> Result<T> {
        self.ok_or_else(|| BombError::Config(msg.to_string()))
    }

    fn with_transport_context(self, msg: &str) -> Result<T> {
        self.ok_or_else(|| BombError::Transport(msg.to_string()))
    }

    fn with_message_context(self, msg: &str) -> Result<T> {
        self.ok_or_else(|| BombError::Message(msg.to_string()))
    }
}

// Convenience constructors
impl BombError {
    pub fn config<S: Into<String>>(msg: S) -> Self {
        BombError::Config(msg.into())
    }

    pub fn execution<S: Into<String>>(msg: S) -> Self {
        BombError::TestExecution(msg.into())
    }
}

// Custom From implementation for boxed WebSocket errors
impl From<tokio_tungstenite::tungstenite::Error> for BombError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        BombError::WebSocket(Box::new(err))
    }
}
