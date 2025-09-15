//! Metrics collection and reporting for Bomb stress testing tool
//!
//! This module provides a clean, modular approach to metrics:
//! - Individual client metrics tracking
//! - Aggregate metrics collection across all clients
//! - Comprehensive test reporting and analysis

pub mod aggregate;
pub mod client;
pub mod reporting;

// Re-export public types for easier access
pub use aggregate::AggregateMetrics;
