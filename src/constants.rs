//! Application-wide constants and configuration values

use std::time::Duration;

// Client connection constants
pub const MAX_CLIENTS_LIMIT: u32 = 10_000;
pub const MAX_RECONNECT_ATTEMPTS: u32 = 5;
pub const INITIAL_BACKOFF_MS: u64 = 1_000;

// Channel and buffer constants
pub const CHANNEL_BUFFER_SIZE: usize = 100;
pub const CLIENT_START_DELAY_MS: u64 = 10;

// Timeout and interval constants
pub const TIMEOUT_CHECK_INTERVAL: Duration = Duration::from_secs(1);

// Default configuration values
pub const DEFAULT_GLOBAL_TIMEOUT_MINUTES: u64 = 5;
pub const EXTRA_CLEANUP_TIME_SECONDS: u64 = 30;

// Progress indication
pub const PROGRESS_DOT_INTERVAL_MS: u64 = 500;

// Performance constants
pub const BULK_TIMEOUT_CLEANUP_INTERVAL: u64 = 50;
pub const DEBUG_LOG_INTERVAL: u64 = 100;
