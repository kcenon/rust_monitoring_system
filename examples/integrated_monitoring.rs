//! Comprehensive example of integrated CPU and memory monitoring
//!
//! This example demonstrates:
//! - Setting up the integrated system monitor
//! - Configuring collection intervals and history size
//! - Auto-collection in background
//! - Accessing real-time and historical metrics
//! - Integration with metric registry

use rust_monitoring_system::collectors::{IntegratedSystemConfig, IntegratedSystemMonitor};
use rust_monitoring_system::MetricRegistry;
use std::sync::Arc;
use std::time::Duration;

fn main() {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("=== Integrated CPU/Memory Monitoring Example ===\n");

    // Create metric registry
    let registry = Arc::new(MetricRegistry::new());

    // Configure the integrated monitor
    let config = IntegratedSystemConfig::default()
        .with_history_size(30) // Keep 30 samples in history
        .with_collection_interval(Duration::from_secs(1)) // Collect every second
        .with_process_monitoring(true) // Enable process-specific metrics
        .with_per_core_monitoring(true); // Enable per-CPU core metrics

    // Create the monitor
    let monitor = Arc::new(IntegratedSystemMonitor::new(Arc::clone(&registry), config));

    // Start automatic collection in background
    let handle = monitor.clone().start_auto_collect();

    println!("✓ Started integrated system monitoring");
    println!("  - Collection interval: 1 second");
    println!("  - History size: 30 samples");
    println!("  - Process monitoring: enabled");
    println!("  - Per-core monitoring: enabled\n");

    // Collect metrics for 10 seconds
    println!("Collecting metrics for 10 seconds...\n");
    for i in 1..=10 {
        std::thread::sleep(Duration::from_secs(1));

        // Print real-time metrics
        println!("--- Sample {} ---", i);
        println!(
            "System CPU:     {:.2}% (avg: {:.2}%, peak: {:.2}%)",
            monitor.current_cpu_usage(),
            monitor.average_cpu_usage(),
            monitor.peak_cpu_usage()
        );

        let mem_mb = monitor.current_memory_usage() / 1024 / 1024;
        let avg_mem_mb = monitor.average_memory_usage() / 1024 / 1024;
        let peak_mem_mb = monitor.peak_memory_usage() / 1024 / 1024;

        println!(
            "System Memory:  {} MB (avg: {} MB, peak: {} MB)",
            mem_mb, avg_mem_mb, peak_mem_mb
        );

        println!(
            "Process CPU:    {:.2}% (avg: {:.2}%)",
            monitor.current_process_cpu_usage(),
            monitor.average_process_cpu_usage()
        );

        let proc_mem_mb = monitor.current_process_memory_usage() / 1024 / 1024;
        let proc_avg_mb = monitor.average_process_memory_usage() / 1024 / 1024;

        println!(
            "Process Memory: {} MB (avg: {} MB)\n",
            proc_mem_mb, proc_avg_mb
        );
    }

    // Show all collected metrics
    println!("=== All Metrics ===");
    let metrics = registry.collect();
    println!("Total metrics collected: {}\n", metrics.len());

    // Group metrics by category
    let mut cpu_metrics = Vec::new();
    let mut memory_metrics = Vec::new();
    let mut process_metrics = Vec::new();

    for metric in metrics {
        if metric.name.contains("cpu") {
            cpu_metrics.push(metric);
        } else if metric.name.contains("memory") || metric.name.contains("swap") {
            memory_metrics.push(metric);
        } else if metric.name.starts_with("process") {
            process_metrics.push(metric);
        }
    }

    println!("CPU Metrics ({}):", cpu_metrics.len());
    for metric in cpu_metrics {
        println!("  - {}: {:?}", metric.name, metric.value);
    }

    println!("\nMemory Metrics ({}):", memory_metrics.len());
    for metric in memory_metrics {
        println!("  - {}: {:?}", metric.name, metric.value);
    }

    println!("\nProcess Metrics ({}):", process_metrics.len());
    for metric in process_metrics {
        println!("  - {}: {:?}", metric.name, metric.value);
    }

    // Demonstrate reset functionality
    println!("\n=== Testing Reset ===");
    println!("Peak CPU before reset: {:.2}%", monitor.peak_cpu_usage());
    println!(
        "Peak memory before reset: {} MB",
        monitor.peak_memory_usage() / 1024 / 1024
    );

    monitor.reset_history();

    println!("Peak CPU after reset: {:.2}%", monitor.peak_cpu_usage());
    println!(
        "Peak memory after reset: {} MB",
        monitor.peak_memory_usage() / 1024 / 1024
    );

    // Demonstrate enable/disable
    println!("\n=== Testing Enable/Disable ===");
    monitor.set_enabled(false);
    println!("Monitoring disabled: {}", !monitor.is_enabled());

    std::thread::sleep(Duration::from_secs(2));

    monitor.set_enabled(true);
    println!("Monitoring re-enabled: {}", monitor.is_enabled());

    // Clean shutdown
    println!("\n=== Shutting Down ===");
    handle.stop();
    println!("✓ Monitoring stopped gracefully");
}
