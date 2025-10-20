//! Basic monitoring usage example
//!
//! Demonstrates counters, gauges, labels, and Prometheus export.
//!
//! Run with: cargo run --example basic_usage

use rust_monitoring_system::prelude::*;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    println!("=== Rust Monitoring System - Basic Usage Example ===\n");

    // Create and start monitor
    let monitor = Monitor::new();
    monitor.start()?;

    println!("1. Creating and using counters:");

    // Create a counter without labels
    let request_counter = monitor.counter("http_requests_total", HashMap::new());

    // Increment counter
    for i in 1..=10 {
        request_counter.inc();
        println!("   Request {}: counter = {}", i, request_counter.get());
        thread::sleep(Duration::from_millis(50));
    }

    println!("\n2. Creating and using gauges:");

    // Create a gauge for active connections
    let connections_gauge = monitor.gauge("active_connections", HashMap::new());

    // Simulate connection changes
    connections_gauge.set(5);
    println!("   Initial connections: {}", connections_gauge.get());

    connections_gauge.inc_by(3);
    println!("   After adding 3: {}", connections_gauge.get());

    connections_gauge.dec_by(2);
    println!("   After removing 2: {}", connections_gauge.get());

    println!("\n3. Using labels:");

    // Create counters with different labels
    let mut get_labels = HashMap::new();
    get_labels.insert("method".to_string(), "GET".to_string());
    get_labels.insert("endpoint".to_string(), "/api/users".to_string());

    let get_counter = monitor.counter("api_requests_total", get_labels);

    let mut post_labels = HashMap::new();
    post_labels.insert("method".to_string(), "POST".to_string());
    post_labels.insert("endpoint".to_string(), "/api/users".to_string());

    let post_counter = monitor.counter("api_requests_total", post_labels);

    // Simulate API requests
    for _ in 0..15 {
        get_counter.inc();
    }

    for _ in 0..8 {
        post_counter.inc();
    }

    println!("   GET requests: {}", get_counter.get());
    println!("   POST requests: {}", post_counter.get());

    println!("\n4. Collecting all metrics:");

    let metrics = monitor.collect();
    println!("   Total metrics registered: {}", metrics.len());

    for metric in &metrics {
        println!("   - {}: {:?}", metric.name, metric.value);
    }

    println!("\n5. Exporting to Prometheus format:");

    let exporter = PrometheusExporter::new();
    let prometheus_output = exporter.export(&metrics)?;

    println!("{}", prometheus_output);

    // Stop monitor
    monitor.stop()?;

    println!("=== Example completed successfully! ===");

    Ok(())
}
