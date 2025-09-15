mod client;
mod common;
mod config;
mod constants;
mod errors;
mod message;
mod metrics;
mod patterns;

use client::ClientManager;
use config::Config;
use errors::Result;
use std::process;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() {
    // Initialize the application and run
    if let Err(e) = run().await {
        error!("Application failed: {}", e);
        process::exit(1);
    }
}

/// Main application logic
async fn run() -> Result<()> {
    // Parse and validate configuration
    let config = Config::from_args()?;

    // Initialize logging based on verbosity
    init_logging(&config);

    info!("💣 Bomb - HTTP & WebSocket Stress Testing Tool");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Print configuration summary
    config.print_summary();

    // Create and run the client manager for both HTTP and WebSocket modes
    let mut client_manager = ClientManager::new(config);

    // Run the stress test
    client_manager
        .run_stress_test()
        .await
        .map(|_| {
            info!("Stress test completed successfully");
        })
        .map_err(|e| {
            error!("Stress test failed: {}", e);
            e
        })
}

/// Initialize logging based on configuration
fn init_logging(config: &Config) {
    let bomb_level = if config.output.verbose {
        "debug"
    } else {
        "info"
    };

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(
                    format!("bomb={}", bomb_level)
                        .parse()
                        .expect("Invalid filter directive"),
                )
                .add_directive(
                    "tokio_tungstenite=warn"
                        .parse()
                        .expect("Invalid filter directive"),
                )
                .add_directive(
                    "tungstenite=warn"
                        .parse()
                        .expect("Invalid filter directive"),
                ),
        )
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");

    if config.output.verbose {
        info!("Verbose logging enabled");
    }
}
