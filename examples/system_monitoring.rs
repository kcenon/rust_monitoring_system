//! System metrics monitoring example
//!
//! Demonstrates automated system metrics collection (CPU, memory, uptime).
//!
//! Run with: cargo run --example system_monitoring

use rust_monitoring_system::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    println!("=== Rust Monitoring System - System Monitoring Example ===\n");

    // Create monitor
    let monitor = Arc::new(Monitor::new());
    monitor.start()?;

    println!("1. Starting system metrics collection:");

    // Create system collector
    let collector = SystemCollector::new(monitor.clone())?;

    println!("   System collector created\n");

    println!("2. Collecting system metrics over time:");

    // Collect metrics periodically
    for i in 1..=5 {
        println!("   Collection #{}", i);

        // Collect system metrics
        collector.collect()?;

        // Get current metrics
        let metrics = monitor.collect();

        for metric in &metrics {
            match metric.name.as_str() {
                "system_cpu_usage_percent" => {
                    if let MetricValue::Float(value) = metric.value {
                        println!("     CPU Usage: {:.2}%", value);
                    }
                }
                "system_memory_usage_bytes" => {
                    if let MetricValue::Float(value) = metric.value {
                        let mb = value / 1024.0 / 1024.0;
                        println!("     Memory Usage: {:.2} MB", mb);
                    }
                }
                "system_memory_total_bytes" => {
                    if let MetricValue::Float(value) = metric.value {
                        let mb = value / 1024.0 / 1024.0;
                        println!("     Memory Total: {:.2} MB", mb);
                    }
                }
                "system_uptime_seconds" => {
                    if let MetricValue::Float(value) = metric.value {
                        println!("     Monitor Uptime: {} seconds", value);
                    }
                }
                _ => {}
            }
        }

        println!();

        if i < 5 {
            thread::sleep(Duration::from_secs(2));
        }
    }

    println!("3. Exporting system metrics:");

    let metrics = monitor.collect();
    let exporter = PrometheusExporter::new();
    let output = exporter.export(&metrics)?;

    println!("{}", output);

    // Stop monitor
    monitor.stop()?;

    println!("=== Example completed successfully! ===");

    Ok(())
}
