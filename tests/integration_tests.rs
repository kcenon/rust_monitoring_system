//! Integration tests for monitoring system
//!
//! These tests verify:
//! - Prometheus export format and injection prevention
//! - Cardinality limits
//! - CPU metric calculation
//! - Concurrent metric access
//! - System collectors

use rust_monitoring_system::collectors::system::SystemCollector;
use rust_monitoring_system::core::metric::MetricValue;
use rust_monitoring_system::core::monitor::{Monitor, MonitorConfig};
use rust_monitoring_system::core::registry::MetricRegistry;
use rust_monitoring_system::exporters::prometheus::PrometheusExporter;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_prometheus_injection_prevention() {
    let exporter = PrometheusExporter::new();

    let mut labels = HashMap::new();
    // Attempt injection attack with newlines and quotes
    labels.insert(
        "user".to_string(),
        "evil\"} malicious_metric 100\n# HELP".to_string(),
    );
    labels.insert("path".to_string(), "/api\\test".to_string());

    let monitor = Monitor::new();
    let counter = monitor.counter("http_requests", labels);
    counter.inc_by(42);

    let metrics = monitor.collect();
    let output = exporter.export(&metrics).expect("Failed to export");

    // Verify injection was prevented
    assert!(!output.contains("malicious_metric 100\n"));
    assert!(output.contains("\\\""));
    assert!(output.contains("\\n"));
    assert!(output.contains("\\\\"));

    // Verify metric value is correct
    assert!(output.contains("42"));
}

#[test]
fn test_cardinality_limit_enforcement() {
    let registry = MetricRegistry::with_max_cardinality(10);

    // Create metrics up to the limit
    for i in 0..10 {
        let mut labels = HashMap::new();
        labels.insert("id".to_string(), i.to_string());
        let result = registry.register_counter(format!("metric_{}", i), "help", labels);
        assert!(result.is_ok(), "Should allow metrics within limit");
    }

    // Try to exceed the limit
    let mut labels = HashMap::new();
    labels.insert("id".to_string(), "999".to_string());
    let result = registry.register_counter("metric_999", "help", labels);
    assert!(result.is_err(), "Should reject metrics exceeding limit");
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Cardinality limit exceeded"));
}

#[test]
fn test_cardinality_limit_get_or_create() {
    let registry = MetricRegistry::with_max_cardinality(5);

    // Create metrics up to limit using get_or_create
    for i in 0..5 {
        let mut labels = HashMap::new();
        labels.insert("id".to_string(), i.to_string());
        let counter = registry.get_or_create_counter(format!("counter_{}", i), labels);
        counter.inc();
    }

    assert_eq!(registry.count(), 5);

    // Try to exceed limit - should return a counter but not store it
    let mut labels = HashMap::new();
    labels.insert("id".to_string(), "999".to_string());
    let _counter = registry.get_or_create_counter("counter_999", labels);

    // The counter is returned but not stored
    assert_eq!(registry.count(), 5, "Should not exceed cardinality limit");
}

#[test]
fn test_concurrent_metric_updates() {
    let monitor = Arc::new(Monitor::new());
    let counter = monitor.counter("concurrent_counter", HashMap::new());

    let mut handles = vec![];

    // Spawn 10 threads each incrementing the counter 100 times
    for _ in 0..10 {
        let counter_clone = counter.clone();
        let handle = std::thread::spawn(move || {
            for _ in 0..100 {
                counter_clone.inc();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify counter value
    assert_eq!(counter.get(), 1000);
}

#[test]
fn test_gauge_operations() {
    let monitor = Monitor::new();
    let gauge = monitor.gauge("test_gauge", HashMap::new());

    // Test set
    gauge.set(42);
    assert_eq!(gauge.get(), 42);

    // Test inc
    gauge.inc();
    assert_eq!(gauge.get(), 43);

    // Test dec
    gauge.dec();
    assert_eq!(gauge.get(), 42);

    // Test inc_by
    gauge.inc_by(10);
    assert_eq!(gauge.get(), 52);

    // Test dec_by
    gauge.dec_by(2);
    assert_eq!(gauge.get(), 50);
}

#[test]
fn test_metric_labels() {
    let monitor = Monitor::new();

    let mut labels1 = HashMap::new();
    labels1.insert("method".to_string(), "GET".to_string());
    labels1.insert("status".to_string(), "200".to_string());

    let mut labels2 = HashMap::new();
    labels2.insert("method".to_string(), "POST".to_string());
    labels2.insert("status".to_string(), "201".to_string());

    let counter1 = monitor.counter("http_requests", labels1);
    let counter2 = monitor.counter("http_requests", labels2);

    counter1.inc_by(10);
    counter2.inc_by(20);

    assert_eq!(counter1.get(), 10);
    assert_eq!(counter2.get(), 20);

    // Verify they're separate metrics
    let metrics = monitor.collect();
    assert_eq!(metrics.len(), 2);
}

#[test]
fn test_default_labels() {
    let mut default_labels = HashMap::new();
    default_labels.insert("service".to_string(), "test_service".to_string());
    default_labels.insert("env".to_string(), "test".to_string());

    let config = MonitorConfig::new("test").with_labels(default_labels);
    let monitor = Monitor::with_config(config);

    let counter = monitor
        .register_counter("requests", "Request count", HashMap::new())
        .expect("Failed to register");

    counter.inc();

    let metrics = monitor.collect();
    assert_eq!(metrics.len(), 1);

    // Verify default labels are applied
    assert_eq!(
        metrics[0].labels.get("service"),
        Some(&"test_service".to_string())
    );
    assert_eq!(metrics[0].labels.get("env"), Some(&"test".to_string()));
}

#[test]
fn test_prometheus_export_format() {
    let monitor = Monitor::new();

    let counter = monitor
        .register_counter("http_requests_total", "Total HTTP requests", HashMap::new())
        .expect("Failed to register");
    counter.inc_by(42);

    let mut labels = HashMap::new();
    labels.insert("status".to_string(), "200".to_string());
    let gauge = monitor
        .register_gauge("http_response_time", "HTTP response time", labels)
        .expect("Failed to register");
    gauge.set(123);

    let metrics = monitor.collect();
    let exporter = PrometheusExporter::new();
    let output = exporter.export(&metrics).expect("Failed to export");

    // Verify Prometheus format
    assert!(output.contains("# HELP http_requests_total Total HTTP requests"));
    assert!(output.contains("# TYPE http_requests_total counter"));
    assert!(output.contains("http_requests_total 42"));

    assert!(output.contains("# HELP http_response_time HTTP response time"));
    assert!(output.contains("# TYPE http_response_time gauge"));
    assert!(output.contains("http_response_time{status=\"200\"} 123"));
}

#[test]
fn test_metric_name_sanitization() {
    let exporter = PrometheusExporter::new();
    let monitor = Monitor::new();

    // Create metric with invalid characters
    let counter = monitor.counter("test-metric.with!invalid@chars", HashMap::new());
    counter.inc();

    let metrics = monitor.collect();
    let output = exporter.export(&metrics).expect("Failed to export");

    // Verify metric name is sanitized
    assert!(output.contains("test_metric_with_invalid_chars"));
    assert!(!output.contains("test-metric.with!invalid@chars"));
}

#[test]
fn test_label_name_sanitization() {
    let exporter = PrometheusExporter::new();
    let monitor = Monitor::new();

    let mut labels = HashMap::new();
    labels.insert("invalid-label.name!".to_string(), "value".to_string());

    let counter = monitor.counter("test_metric", labels);
    counter.inc();

    let metrics = monitor.collect();
    let output = exporter.export(&metrics).expect("Failed to export");

    // Verify label name is sanitized
    assert!(output.contains("invalid_label_name_"));
}

#[test]
fn test_system_collector_creation() {
    let monitor = Arc::new(Monitor::new());
    let result = SystemCollector::new(monitor);
    assert!(result.is_ok());
}

#[test]
fn test_system_collector_metrics() {
    let monitor = Arc::new(Monitor::new());
    let collector = SystemCollector::new(monitor.clone()).expect("Failed to create collector");

    // Collect metrics
    let result = collector.collect();
    assert!(result.is_ok());

    // Verify metrics were created
    let metrics = monitor.collect();

    // Should have at least: cpu_usage, memory_usage, memory_total, uptime
    assert!(metrics.len() >= 4, "Should have at least 4 system metrics");

    // Find specific metrics
    let has_cpu = metrics.iter().any(|m| m.name.contains("cpu_usage"));
    let has_memory = metrics.iter().any(|m| m.name.contains("memory"));
    let has_uptime = metrics.iter().any(|m| m.name.contains("uptime"));

    assert!(has_cpu, "Should have CPU metric");
    assert!(has_memory, "Should have memory metric");
    assert!(has_uptime, "Should have uptime metric");
}

#[test]
fn test_monitor_lifecycle() {
    let monitor = Monitor::new();

    assert!(!monitor.is_running());

    monitor.start().expect("Failed to start");
    assert!(monitor.is_running());

    monitor.stop().expect("Failed to stop");
    assert!(!monitor.is_running());
}

#[test]
fn test_monitor_config() {
    let config = MonitorConfig::new("test_service")
        .with_interval(Duration::from_secs(30))
        .with_auto_collect(true)
        .with_max_cardinality(5000);

    assert_eq!(config.service_name, "test_service");
    assert_eq!(config.collection_interval, Duration::from_secs(30));
    assert!(config.auto_collect);
    assert_eq!(config.max_cardinality, 5000);
}

#[test]
fn test_metric_collection_snapshot() {
    let monitor = Monitor::new();

    let counter = monitor.counter("test_counter", HashMap::new());
    counter.inc_by(10);

    // Collect metrics
    let metrics1 = monitor.collect();
    assert_eq!(metrics1.len(), 1);
    assert!(matches!(metrics1[0].value, MetricValue::Uint(10)));

    // Increment again
    counter.inc_by(5);

    // Collect again - should see new value
    let metrics2 = monitor.collect();
    assert_eq!(metrics2.len(), 1);
    assert!(matches!(metrics2[0].value, MetricValue::Uint(15)));
}

#[test]
fn test_clear_metrics() {
    let monitor = Monitor::new();

    // Create some metrics
    let counter = monitor.counter("counter1", HashMap::new());
    counter.inc();

    let gauge = monitor.gauge("gauge1", HashMap::new());
    gauge.set(42);

    assert_eq!(monitor.metric_count(), 2);

    // Clear all metrics
    monitor.clear();

    assert_eq!(monitor.metric_count(), 0);
}

#[test]
fn test_concurrent_metric_collection() {
    let monitor = Arc::new(Monitor::new());

    // Create a counter
    let counter = monitor.counter("shared_counter", HashMap::new());

    // Thread 1: increment counter
    let counter1 = counter.clone();
    let handle1 = std::thread::spawn(move || {
        for _ in 0..100 {
            counter1.inc();
            std::thread::sleep(Duration::from_micros(10));
        }
    });

    // Thread 2: collect metrics periodically
    let monitor2 = Arc::clone(&monitor);
    let handle2 = std::thread::spawn(move || {
        for _ in 0..20 {
            let _metrics = monitor2.collect();
            std::thread::sleep(Duration::from_micros(50));
        }
    });

    handle1.join().expect("Thread 1 panicked");
    handle2.join().expect("Thread 2 panicked");

    // Verify final counter value
    assert_eq!(counter.get(), 100);
}

#[test]
fn test_zero_cardinality_limit() {
    // Test that cardinality limit of 0 means unlimited
    let registry = MetricRegistry::with_max_cardinality(0);

    // Should be able to create many metrics
    for i in 0..100 {
        let mut labels = HashMap::new();
        labels.insert("id".to_string(), i.to_string());
        let result = registry.register_counter(format!("metric_{}", i), "help", labels);
        assert!(
            result.is_ok(),
            "Should allow unlimited metrics when limit is 0"
        );
    }

    assert_eq!(registry.count(), 100);
}
