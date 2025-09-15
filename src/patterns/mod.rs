//! Random pattern generation for dynamic values in URLs and payloads
//!
//! This module provides runtime replacement of pattern placeholders with random values:
//! - `<rnd:uuid>` - Random UUID v4
//! - `<rnd:int[min, max]>` - Random integer in range
//! - `<rnd:float[min, max]>` - Random float with precision matching
//! - `<rnd:ts[start, end]>` - Random timestamp in Unix seconds
//! - `<rnd:ts>` - Random timestamp in last 30 days
//! - `<rnd:datetime[start, end]>` - Random RFC3339 datetime in range
//! - `<rnd:datetime>` - Random RFC3339 datetime in last 30 days

use chrono::{TimeZone, Utc};
use rand::Rng;
use regex::Regex;
use std::sync::OnceLock;
use uuid::Uuid;

/// Pre-compiled regex patterns for performance
struct Patterns {
    uuid: Regex,
    int_range: Regex,
    float_range: Regex,
    ts_range: Regex,
    ts_simple: Regex,
    datetime_simple: Regex,
    datetime_range: Regex,
}

static PATTERNS: OnceLock<Patterns> = OnceLock::new();

impl Patterns {
    /// Initialize all regex patterns
    fn new() -> Self {
        Self {
            uuid: Regex::new(r"<rnd:uuid>").expect("Invalid UUID regex"),
            int_range: Regex::new(r"<rnd:int\[\s*(-?\d+)\s*,\s*(-?\d+)\s*\]>")
                .expect("Invalid int range regex"),
            float_range: Regex::new(
                r"<rnd:float\[\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*\]>",
            )
            .expect("Invalid float range regex"),
            ts_range: Regex::new(r"<rnd:ts\[\s*(\d+)\s*,\s*(\d+)\s*\]>")
                .expect("Invalid timestamp range regex"),
            ts_simple: Regex::new(r"<rnd:ts>").expect("Invalid timestamp simple regex"),
            datetime_simple: Regex::new(r"<rnd:datetime>").expect("Invalid datetime simple regex"),
            datetime_range: Regex::new(r"<rnd:datetime\[([^,]+),\s*([^\]]+)\]>")
                .expect("Invalid datetime range regex"),
        }
    }

    /// Get the singleton patterns instance
    fn get() -> &'static Patterns {
        PATTERNS.get_or_init(Patterns::new)
    }
}

/// Resolve all random generation patterns in a string
///
/// # Examples
///
/// ```
/// use bomb::patterns::resolve;
///
/// let input = r#"{"id": "<rnd:uuid>", "value": <rnd:int[1, 100]>}"#;
/// let resolved = resolve(input);
/// // Returns something like: {"id": "550e8400-e29b-41d4-a716-446655440000", "value": 42}
/// ```
pub fn resolve(input: &str) -> String {
    let patterns = Patterns::get();
    let mut result = input.to_string();

    // UUID generation
    result = patterns
        .uuid
        .replace_all(&result, |_: &regex::Captures| Uuid::new_v4().to_string())
        .to_string();

    // Integer range generation
    result = patterns
        .int_range
        .replace_all(&result, |caps: &regex::Captures| {
            let min = caps[1].trim().parse::<i64>().unwrap_or(0);
            let max = caps[2].trim().parse::<i64>().unwrap_or(100);
            let (actual_min, actual_max) = if min <= max { (min, max) } else { (max, min) };

            rand::rng()
                .random_range(actual_min..=actual_max)
                .to_string()
        })
        .to_string();

    // Float range generation with precision matching
    result = patterns
        .float_range
        .replace_all(&result, |caps: &regex::Captures| {
            let min_str = caps[1].trim();
            let max_str = caps[2].trim();
            let min = min_str.parse::<f64>().unwrap_or(0.0);
            let max = max_str.parse::<f64>().unwrap_or(1.0);

            // Determine precision from the minimum value
            let precision = if min_str.contains('.') {
                min_str.split('.').nth(1).map(|s| s.len()).unwrap_or(1)
            } else {
                0
            };

            let (actual_min, actual_max) = if min <= max { (min, max) } else { (max, min) };
            let random_value = rand::rng().random_range(actual_min..=actual_max);

            format!("{:.precision$}", random_value, precision = precision)
        })
        .to_string();

    // Timestamp range generation
    result = patterns
        .ts_range
        .replace_all(&result, |caps: &regex::Captures| {
            let start_ts = caps[1].trim().parse::<i64>().unwrap_or(0);
            let end_ts = caps[2]
                .trim()
                .parse::<i64>()
                .unwrap_or_else(|_| Utc::now().timestamp());

            let (actual_start, actual_end) = if start_ts <= end_ts {
                (start_ts, end_ts)
            } else {
                (end_ts, start_ts)
            };

            let random_ts = rand::rng().random_range(actual_start..=actual_end);
            random_ts.to_string()
        })
        .to_string();

    // Simple timestamp generation (last 30 days)
    result = patterns
        .ts_simple
        .replace_all(&result, |_: &regex::Captures| {
            let now = Utc::now().timestamp();
            let thirty_days_ago = now - (30 * 24 * 60 * 60); // 30 days in seconds
            let random_ts = rand::rng().random_range(thirty_days_ago..=now);
            random_ts.to_string()
        })
        .to_string();

    // Simple datetime generation (last 30 days, RFC3339 format)
    result = patterns
        .datetime_simple
        .replace_all(&result, |_: &regex::Captures| {
            let now = Utc::now().timestamp();
            let thirty_days_ago = now - (30 * 24 * 60 * 60); // 30 days in seconds
            let random_ts = rand::rng().random_range(thirty_days_ago..=now);

            match Utc.timestamp_opt(random_ts, 0) {
                chrono::LocalResult::Single(dt) => dt.to_rfc3339(),
                _ => format!("{{datetime_error: invalid_timestamp_{}}}", random_ts),
            }
        })
        .to_string();

    // Datetime range generation (RFC3339 format)
    result = patterns
        .datetime_range
        .replace_all(&result, |caps: &regex::Captures| {
            let start_str = caps[1].trim();
            let end_str = caps[2].trim();

            // Try to parse as RFC3339 datetime strings first
            let start_ts = chrono::DateTime::parse_from_rfc3339(start_str)
                .map(|dt| dt.timestamp())
                .unwrap_or_else(|_| {
                    // If that fails, try as Unix timestamp
                    start_str.parse::<i64>().unwrap_or(0)
                });

            let end_ts = chrono::DateTime::parse_from_rfc3339(end_str)
                .map(|dt| dt.timestamp())
                .unwrap_or_else(|_| {
                    // If that fails, try as Unix timestamp, default to now
                    end_str
                        .parse::<i64>()
                        .unwrap_or_else(|_| Utc::now().timestamp())
                });

            let (actual_start, actual_end) = if start_ts <= end_ts {
                (start_ts, end_ts)
            } else {
                (end_ts, start_ts)
            };

            let random_ts = rand::rng().random_range(actual_start..=actual_end);

            match Utc.timestamp_opt(random_ts, 0) {
                chrono::LocalResult::Single(dt) => dt.to_rfc3339(),
                _ => format!("{{datetime_error: invalid_timestamp_{}}}", random_ts),
            }
        })
        .to_string();

    result
}

/// Generate a random UUID string
#[allow(dead_code)]
pub fn uuid() -> String {
    Uuid::new_v4().to_string()
}

/// Generate a random integer in range [min, max]
#[allow(dead_code)]
pub fn int_range(min: i64, max: i64) -> i64 {
    let (actual_min, actual_max) = if min <= max { (min, max) } else { (max, min) };
    rand::rng().random_range(actual_min..=actual_max)
}

/// Generate a random float in range [min, max] with specified precision
#[allow(dead_code)]
pub fn float_range(min: f64, max: f64, precision: usize) -> String {
    let (actual_min, actual_max) = if min <= max { (min, max) } else { (max, min) };
    let value = rand::rng().random_range(actual_min..=actual_max);
    format!("{:.precision$}", value, precision = precision)
}

/// Generate a random timestamp in range [start, end] (Unix seconds)
#[allow(dead_code)]
pub fn timestamp_range(start: i64, end: i64) -> i64 {
    let (actual_start, actual_end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    rand::rng().random_range(actual_start..=actual_end)
}

/// Generate a random timestamp in the last 30 days (Unix seconds)
#[allow(dead_code)]
pub fn timestamp() -> i64 {
    let now = Utc::now().timestamp();
    let thirty_days_ago = now - (30 * 24 * 60 * 60);
    rand::rng().random_range(thirty_days_ago..=now)
}

/// Generate a random RFC3339 datetime string in the last 30 days
#[allow(dead_code)]
pub fn datetime() -> String {
    let ts = timestamp();
    match Utc.timestamp_opt(ts, 0) {
        chrono::LocalResult::Single(dt) => dt.to_rfc3339(),
        _ => format!("{{datetime_error: invalid_timestamp_{}}}", ts),
    }
}

/// Generate a random RFC3339 datetime string in range [start, end]
#[allow(dead_code)]
pub fn datetime_range(start: i64, end: i64) -> String {
    let ts = timestamp_range(start, end);
    match Utc.timestamp_opt(ts, 0) {
        chrono::LocalResult::Single(dt) => dt.to_rfc3339(),
        _ => format!("{{datetime_error: invalid_timestamp_{}}}", ts),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_uuid() {
        let input = "test-<rnd:uuid>-end";
        let result = resolve(input);

        assert!(result.starts_with("test-"));
        assert!(result.ends_with("-end"));
        assert_ne!(result, input); // Should be different due to UUID

        // Check UUID format (36 chars with hyphens)
        let uuid_part = &result[5..41]; // Extract UUID part
        assert_eq!(uuid_part.len(), 36);
        assert_eq!(uuid_part.matches('-').count(), 4);
    }

    #[test]
    fn test_resolve_int_range() {
        let input = "value: <rnd:int[10, 20]>";
        let result = resolve(input);

        assert!(result.starts_with("value: "));
        let value_str = result.strip_prefix("value: ").unwrap();
        let value: i32 = value_str.parse().expect("Should be a valid integer");
        assert!((10..=20).contains(&value));
    }

    #[test]
    fn test_resolve_int_range_reversed() {
        let input = "value: <rnd:int[20, 10]>";
        let result = resolve(input);

        let value_str = result.strip_prefix("value: ").unwrap();
        let value: i32 = value_str.parse().expect("Should be a valid integer");
        assert!(
            (10..=20).contains(&value),
            "Value {} should be between 10 and 20",
            value
        );
    }

    #[test]
    fn test_resolve_float_range() {
        let input = "score: <rnd:float[0.0, 1.0]>";
        let result = resolve(input);

        let value_str = result.strip_prefix("score: ").unwrap();
        let value: f64 = value_str.parse().expect("Should be a valid float");
        assert!((0.0..=1.0).contains(&value));
    }

    #[test]
    fn test_resolve_timestamp_simple() {
        let input = "ts: <rnd:ts>";
        let result = resolve(input);

        let ts_str = result.strip_prefix("ts: ").unwrap();
        let ts: i64 = ts_str.parse().expect("Should be a valid timestamp");

        let now = Utc::now().timestamp();
        let thirty_days_ago = now - (30 * 24 * 60 * 60);
        assert!(ts >= thirty_days_ago && ts <= now);
    }

    #[test]
    fn test_resolve_datetime_simple() {
        let input = "dt: <rnd:datetime>";
        let result = resolve(input);

        assert!(result.starts_with("dt: "));
        let dt_str = result.strip_prefix("dt: ").unwrap();

        // Should be valid RFC3339 format
        assert!(chrono::DateTime::parse_from_rfc3339(dt_str).is_ok());
    }

    #[test]
    fn test_resolve_multiple_patterns() {
        let input =
            r#"{"id": "<rnd:uuid>", "value": <rnd:int[1, 10]>, "score": <rnd:float[0.0, 1.0]>}"#;
        let result = resolve(input);

        // Should be valid JSON
        let json: serde_json::Value = serde_json::from_str(&result).expect("Should be valid JSON");

        // Check that all patterns were resolved
        assert!(json["id"].is_string());
        assert!(json["value"].is_number());
        assert!(json["score"].is_number());

        let value = json["value"].as_i64().unwrap();
        assert!((1..=10).contains(&value));

        let score = json["score"].as_f64().unwrap();
        assert!((0.0..=1.0).contains(&score));
    }

    #[test]
    fn test_no_patterns_unchanged() {
        let input = "plain text with no patterns";
        let result = resolve(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_malformed_patterns_ignored() {
        let input = "test <rnd:invalid> <rnd> <rnd:>";
        let result = resolve(input);
        assert_eq!(result, input); // Should remain unchanged
    }

    #[test]
    fn test_helper_functions() {
        // Test individual helper functions
        let uuid = uuid();
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.matches('-').count(), 4);

        let int_val = int_range(5, 15);
        assert!((5..=15).contains(&int_val));

        let float_val = float_range(0.0, 1.0, 2);
        assert!(float_val.contains('.'));
        let parsed: f64 = float_val.parse().unwrap();
        assert!((0.0..=1.0).contains(&parsed));

        let ts = timestamp();
        let now = Utc::now().timestamp();
        let thirty_days_ago = now - (30 * 24 * 60 * 60);
        assert!(ts >= thirty_days_ago && ts <= now);

        let dt = datetime();
        assert!(chrono::DateTime::parse_from_rfc3339(&dt).is_ok());
    }
}
