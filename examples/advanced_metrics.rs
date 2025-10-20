//! Advanced metrics usage example
//!
//! Demonstrates multi-component monitoring with web server and database simulations.
//!
//! Run with: cargo run --example advanced_metrics

use rust_monitoring_system::core::monitor::MonitorConfig;
use rust_monitoring_system::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn simulate_web_server(monitor: Arc<Monitor>) {
    println!("   Starting web server simulation...");

    // Create metrics for different endpoints
    let endpoints = vec!["/api/users", "/api/products", "/api/orders"];
    let methods = vec!["GET", "POST", "PUT", "DELETE"];

    let mut counters = Vec::new();

    // Create counters for each endpoint and method combination
    for endpoint in &endpoints {
        for method in &methods {
            let mut labels = HashMap::new();
            labels.insert("endpoint".to_string(), endpoint.to_string());
            labels.insert("method".to_string(), method.to_string());

            let counter = monitor.counter("http_requests_total", labels);

            counters.push((endpoint.to_string(), method.to_string(), counter));
        }
    }

    // Create response time gauge
    let response_time = monitor.gauge("http_response_time_ms", HashMap::new());

    // Create active requests gauge
    let active_requests = monitor.gauge("http_active_requests", HashMap::new());

    // Simulate requests
    for i in 1..=20 {
        // Randomly pick an endpoint and method
        let counter_idx = i % counters.len();
        let (endpoint, method, counter) = &counters[counter_idx];

        // Simulate request
        active_requests.inc();

        // Simulate processing time
        let processing_time = 50 + (i * 13) % 200;
        thread::sleep(Duration::from_millis(processing_time as u64));

        // Update metrics
        counter.inc();
        response_time.set(processing_time as i64);
        active_requests.dec();

        if i % 5 == 0 {
            println!(
                "   Request {}: {} {} - {}ms",
                i, method, endpoint, processing_time
            );
        }
    }

    println!("   Web server simulation completed");
}

fn simulate_database(monitor: Arc<Monitor>) {
    println!("   Starting database simulation...");

    // Create database metrics
    let query_counter = monitor.counter("db_queries_total", HashMap::new());

    let connection_pool_gauge = monitor.gauge("db_connection_pool_size", HashMap::new());

    let query_time_gauge = monitor.gauge("db_query_time_ms", HashMap::new());

    // Simulate database operations
    connection_pool_gauge.set(10);

    for i in 1..=15 {
        // Simulate query
        let query_time = 10 + (i * 7) % 100;
        thread::sleep(Duration::from_millis(query_time as u64));

        query_counter.inc();
        query_time_gauge.set(query_time as i64);

        // Simulate connection pool changes
        if i % 5 == 0 {
            connection_pool_gauge.inc();
        }

        if i % 7 == 0 {
            connection_pool_gauge.dec();
        }

        if i % 5 == 0 {
            println!("   Query {}: {}ms", i, query_time);
        }
    }

    println!("   Database simulation completed");
}

fn main() -> Result<()> {
    println!("=== Rust Monitoring System - Advanced Metrics Example ===\n");

    // Create monitor with custom configuration
    let config = MonitorConfig::new("advanced_app")
        .with_interval(Duration::from_secs(10))
        .with_auto_collect(false);

    let monitor = Arc::new(Monitor::with_config(config));
    monitor.start()?;

    println!("1. Running multi-component simulation:\n");

    // Spawn web server simulation
    let monitor_clone = monitor.clone();
    let web_thread = thread::spawn(move || {
        simulate_web_server(monitor_clone);
    });

    // Spawn database simulation
    let monitor_clone = monitor.clone();
    let db_thread = thread::spawn(move || {
        simulate_database(monitor_clone);
    });

    // Wait for simulations to complete
    web_thread.join().unwrap();
    db_thread.join().unwrap();

    println!("\n2. Metric summary:");

    let metrics = monitor.collect();
    println!("   Total metrics: {}", metrics.len());

    // Group metrics by name
    let mut metric_counts: HashMap<String, usize> = HashMap::new();
    for metric in &metrics {
        *metric_counts.entry(metric.name.clone()).or_insert(0) += 1;
    }

    println!("\n   Metrics by name:");
    for (name, count) in metric_counts {
        println!("     {}: {} variants", name, count);
    }

    println!("\n3. Sample metrics:");

    for metric in metrics.iter().take(5) {
        println!("   - {} ({:?})", metric.name, metric.metric_type);
        if !metric.labels.is_empty() {
            println!("     Labels: {:?}", metric.labels);
        }
        println!("     Value: {:?}", metric.value);
    }

    println!("\n4. Prometheus export (first 50 lines):");

    let exporter = PrometheusExporter::new();
    let output = exporter.export(&metrics)?;

    for (i, line) in output.lines().enumerate() {
        if i >= 50 {
            println!("   ... (truncated)");
            break;
        }
        println!("   {}", line);
    }

    // Stop monitor
    monitor.stop()?;

    println!("\n=== Example completed successfully! ===");

    Ok(())
}
