//! Metrics reporting and output formatting

use crate::metrics::aggregate::AggregateMetrics;
use crate::metrics::client::ClientMetrics;

use std::sync::atomic::Ordering;
use std::time::Duration;

impl AggregateMetrics {
    /// Print a comprehensive test report
    pub async fn print_report(
        &self,
        config_summary: &str,
        test_duration: Option<Duration>,
        connection_mode: &str,
    ) {
        let client_metrics = self.get_client_metrics().await;
        let (avg_rtt, min_rtt, max_rtt) = self.calculate_rtt_stats().await;

        let sent = self.total_messages_sent.load(Ordering::Relaxed);
        let received = self.total_messages_received.load(Ordering::Relaxed);
        let failed = self.total_messages_failed.load(Ordering::Relaxed);
        let connection_errors = self.total_connection_errors.load(Ordering::Relaxed);
        let reconnections = self.total_reconnections.load(Ordering::Relaxed);
        let http_connections_created = self.total_http_connections_created.load(Ordering::Relaxed);
        let http_connections_reused = self.total_http_connections_reused.load(Ordering::Relaxed);

        println!("\n📊 Bomb Stress Test Results");
        println!("═══════════════════════════════════════════════════════════════");

        // Test configuration
        println!("\n🔧 Configuration:");
        print!("{}", config_summary);

        // Overall results
        println!("📈 Overall Results:");
        if let Some(duration) = test_duration {
            println!("   Test Duration:    {:.2}s", duration.as_secs_f64());
        }
        println!("   Messages Sent:    {}", sent);
        println!("   Messages Received: {}", received);
        println!("   Messages Failed:  {}", failed);
        println!(
            "   Success Rate:     {:.2}%",
            self.overall_success_rate().await
        );

        // Performance metrics
        self.print_performance_metrics(&client_metrics, test_duration)
            .await;

        // Round-trip time statistics
        if received > 0 {
            self.print_rtt_stats(avg_rtt, min_rtt, max_rtt);
        }

        // Connection metrics
        if connection_mode == "ws" {
            self.print_connection_metrics(connection_errors, reconnections);
        } else {
            self.print_http_metrics(http_connections_created, http_connections_reused)
                .await;
        }

        // Per-client breakdown
        self.print_client_breakdown(&client_metrics).await;

        println!("\n🎯 Test completed successfully!");
        println!("═══════════════════════════════════════════════════════════════");
    }

    async fn print_performance_metrics(
        &self,
        client_metrics: &[ClientMetrics],
        test_duration: Option<Duration>,
    ) {
        println!("\n⚡ Performance:");
        let messages_per_sec = if let Some(duration) = test_duration {
            let sent = self.total_messages_sent.load(Ordering::Relaxed);
            let seconds = duration.as_secs_f64();
            if seconds > 0.0 {
                sent as f64 / seconds
            } else {
                0.0
            }
        } else {
            0.0
        };
        println!("   Messages/sec:     {:.2}", messages_per_sec);

        if !client_metrics.is_empty() {
            let avg_per_client = messages_per_sec / client_metrics.len() as f64;
            println!("   Per Client:       {:.2} msg/s", avg_per_client);
        }
    }

    fn print_rtt_stats(&self, avg_rtt: f64, min_rtt: Option<u64>, max_rtt: Option<u64>) {
        println!("\n🔄 Round-Trip Time:");
        println!("   Average RTT:      {:.2}ms", avg_rtt);
        if let Some(min) = min_rtt {
            println!("   Min RTT:          {}ms", min);
        }
        if let Some(max) = max_rtt {
            println!("   Max RTT:          {}ms", max);
        }
    }

    fn print_connection_metrics(&self, connection_errors: u64, reconnections: u64) {
        if connection_errors > 0 || reconnections > 0 {
            println!("\n🔌 Connection Metrics:");
            if connection_errors > 0 {
                println!("   Connection Errors: {}", connection_errors);
            }
            if reconnections > 0 {
                println!("   Reconnections:    {}", reconnections);
            }
        }
    }

    async fn print_http_metrics(&self, connections_created: u64, connections_reused: u64) {
        if connections_created > 0 || connections_reused > 0 {
            println!("\n🌐 HTTP Connection Metrics:");
            println!("   Connections Created: {}", connections_created);
            println!("   Connections Reused:  {}", connections_reused);

            // HTTP status code breakdown
            let status_codes = self.http_status_codes.read().await;
            if !status_codes.is_empty() {
                println!("\n📋 HTTP Status Codes:");
                let mut sorted_codes: Vec<_> = status_codes.iter().collect();
                sorted_codes.sort_by_key(|(code, _)| *code);

                for (&status_code, &count) in sorted_codes {
                    let status_text = match status_code {
                        200 => "OK",
                        201 => "Created",
                        400 => "Bad Request",
                        401 => "Unauthorized",
                        403 => "Forbidden",
                        404 => "Not Found",
                        500 => "Internal Server Error",
                        502 => "Bad Gateway",
                        503 => "Service Unavailable",
                        _ => "Unknown",
                    };
                    println!("   {} ({}):        {}", status_code, status_text, count);
                }
            }
        }
    }

    async fn print_client_breakdown(&self, client_metrics: &[ClientMetrics]) {
        let active_clients: Vec<_> = client_metrics.iter().filter(|m| m.has_activity()).collect();

        if active_clients.len() <= 10 {
            println!("\n👥 Per-Client Breakdown:");
            for client in &active_clients {
                println!(
                    "   Client {}: {} sent, {} received, {} failed (Success: {:.1}%)",
                    client.client_id,
                    client.messages_sent,
                    client.messages_received,
                    client.messages_failed,
                    client.success_rate()
                );
            }
        } else {
            println!(
                "\n👥 Client Summary ({} active clients):",
                active_clients.len()
            );

            // Show best and worst performing clients
            let mut sorted_clients = active_clients.clone();
            sorted_clients.sort_by(|a, b| {
                b.success_rate()
                    .partial_cmp(&a.success_rate())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some(best) = sorted_clients.first() {
                println!(
                    "   Best Performance:  Client {} ({:.1}% success rate)",
                    best.client_id,
                    best.success_rate()
                );
            }

            if let Some(worst) = sorted_clients.last()
                && worst.client_id != sorted_clients.first().unwrap().client_id
            {
                println!(
                    "   Worst Performance: Client {} ({:.1}% success rate)",
                    worst.client_id,
                    worst.success_rate()
                );
            }
        }
    }

    async fn calculate_rtt_stats(&self) -> (f64, Option<u64>, Option<u64>) {
        let avg_rtt = self.overall_avg_rtt_ms().await;
        let min_rtt = self.overall_min_rtt_ms().await;
        let max_rtt = self.overall_max_rtt_ms().await;
        (avg_rtt, min_rtt, max_rtt)
    }
}
