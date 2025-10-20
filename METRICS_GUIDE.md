# Rust Monitoring System - Metrics Guide

> **Languages**: English | [한국어](./METRICS_GUIDE.ko.md)

## Overview

This comprehensive guide covers all aspects of the Rust Monitoring System, including metric types, usage patterns, best practices, and integration with monitoring platforms.

## Table of Contents

1. [Metric Types](#metric-types)
2. [Creating Metrics](#creating-metrics)
3. [Labels and Dimensions](#labels-and-dimensions)
4. [Collection Strategies](#collection-strategies)
5. [Export Formats](#export-formats)
6. [System Metrics](#system-metrics)
7. [Best Practices](#best-practices)
8. [Integration Examples](#integration-examples)

## Metric Types

### Counter

A **Counter** is a cumulative metric that only increases (or resets to zero).

**Use Cases:**
- Request counts
- Error counts
- Bytes processed
- Events triggered

**Example:**
```rust
use rust_monitoring_system::prelude::*;
use std::collections::HashMap;

let monitor = Monitor::new();
let counter = monitor.counter(
    "http_requests_total",
    "Total HTTP requests received",
    HashMap::new()
);

counter.inc();           // Increment by 1
counter.inc_by(5);       // Increment by 5
let value = counter.get(); // Read current value
```

**Characteristics:**
- Monotonically increasing
- Never decreases (except on reset)
- Rate of change is meaningful
- Thread-safe (atomic operations)

**Prometheus Query Examples:**
```promql
# Rate of requests per second
rate(http_requests_total[5m])

# Total requests in last hour
increase(http_requests_total[1h])
```

### Gauge

A **Gauge** is a metric that can go up or down.

**Use Cases:**
- Current temperature
- Memory usage
- Active connections
- Queue size
- CPU utilization

**Example:**
```rust
let gauge = monitor.gauge(
    "active_connections",
    "Current number of active connections",
    HashMap::new()
);

gauge.set(42);          // Set to specific value
gauge.inc();            // Increment by 1
gauge.dec();            // Decrement by 1
gauge.inc_by(10);       // Increment by 10
gauge.dec_by(5);        // Decrement by 5
let value = gauge.get(); // Read current value
```

**Characteristics:**
- Can increase or decrease
- Represents current state
- Point-in-time measurement
- Thread-safe (atomic operations)

**Prometheus Query Examples:**
```promql
# Current value
active_connections

# Average over time
avg_over_time(active_connections[5m])

# Max connections in last hour
max_over_time(active_connections[1h])
```

### Histogram

A **Histogram** samples observations and counts them in configurable buckets.

**Use Cases:**
- Request durations
- Response sizes
- Query execution times
- Processing latencies

**Example:**
```rust
let histogram = monitor.histogram(
    "http_request_duration_seconds",
    "HTTP request duration in seconds",
    vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0], // Buckets
    HashMap::new()
);

histogram.observe(0.234);  // Record observation
histogram.observe(1.456);
histogram.observe(0.089);
```

**Characteristics:**
- Records distribution of values
- Pre-defined buckets
- Calculates sum and count
- Supports percentile calculation

**Bucket Configuration:**

```rust
// Latency buckets (seconds)
let latency_buckets = vec![
    0.001, 0.005, 0.01, 0.025, 0.05,
    0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0
];

// Size buckets (bytes)
let size_buckets = vec![
    100.0, 1000.0, 10_000.0, 100_000.0,
    1_000_000.0, 10_000_000.0
];

// Percentage buckets
let percent_buckets = vec![
    0.0, 10.0, 20.0, 30.0, 40.0, 50.0,
    60.0, 70.0, 80.0, 90.0, 95.0, 99.0, 100.0
];
```

**Prometheus Query Examples:**
```promql
# 95th percentile latency
histogram_quantile(0.95, http_request_duration_seconds_bucket)

# Requests slower than 1 second
sum(rate(http_request_duration_seconds_bucket{le="1"}[5m]))

# Average request duration
rate(http_request_duration_seconds_sum[5m])
  / rate(http_request_duration_seconds_count[5m])
```

### Summary

A **Summary** samples observations and calculates configurable quantiles over a sliding time window.

**Use Cases:**
- Request latencies (when buckets unknown)
- Message sizes (variable distribution)
- Custom percentiles

**Example:**
```rust
let summary = monitor.summary(
    "api_response_time_seconds",
    "API response time in seconds",
    HashMap::new()
);

summary.observe(0.123);
summary.observe(0.456);
summary.observe(0.789);
```

**Histogram vs Summary:**

| Feature | Histogram | Summary |
|---------|-----------|---------|
| **Quantiles** | Approximated | Exact |
| **Aggregation** | Server-side | Client-side |
| **Buckets** | Pre-defined | Not needed |
| **Overhead** | Lower | Higher |
| **Accuracy** | Approximate | Exact |
| **Best For** | Known ranges | Unknown ranges |

### Timer

A **Timer** is a specialized histogram for measuring durations.

**Example:**
```rust
let timer = monitor.timer(
    "operation_duration",
    "Operation duration in seconds",
    HashMap::new()
);

let start = std::time::Instant::now();
// ... do work ...
timer.observe_duration(start.elapsed());

// Or use automatic timing
{
    let _guard = timer.start_timer();
    // ... work is timed automatically ...
} // Timer stopped and recorded on drop
```

## Creating Metrics

### Basic Metric Creation

```rust
use rust_monitoring_system::prelude::*;
use std::collections::HashMap;

let monitor = Monitor::new();
monitor.start()?;

// Counter without labels
let requests = monitor.counter(
    "http_requests_total",
    "Total HTTP requests",
    HashMap::new()
);

// Gauge without labels
let memory = monitor.gauge(
    "memory_usage_bytes",
    "Current memory usage",
    HashMap::new()
);

// Histogram with buckets
let latency = monitor.histogram(
    "request_latency_seconds",
    "Request latency",
    vec![0.01, 0.1, 0.5, 1.0, 5.0],
    HashMap::new()
);
```

### Metrics with Labels

```rust
use std::collections::HashMap;

// Create metric with labels
let mut labels = HashMap::new();
labels.insert("method".to_string(), "GET".to_string());
labels.insert("endpoint".to_string(), "/api/users".to_string());
labels.insert("status".to_string(), "200".to_string());

let requests = monitor.counter(
    "http_requests_total",
    "Total HTTP requests",
    labels
);
```

### Dynamic Label Creation

```rust
fn record_request(monitor: &Monitor, method: &str, endpoint: &str, status: u16) {
    let mut labels = HashMap::new();
    labels.insert("method".to_string(), method.to_string());
    labels.insert("endpoint".to_string(), endpoint.to_string());
    labels.insert("status".to_string(), status.to_string());

    let counter = monitor.counter(
        "http_requests_total",
        "Total HTTP requests",
        labels
    );

    counter.inc();
}

// Usage
record_request(&monitor, "GET", "/api/users", 200);
record_request(&monitor, "POST", "/api/users", 201);
record_request(&monitor, "GET", "/api/products", 200);
```

## Labels and Dimensions

### Label Best Practices

#### Good Label Names

```rust
// ✓ Good: Descriptive, lowercase, underscore-separated
"method"
"endpoint"
"status_code"
"error_type"
"instance_id"
```

#### Bad Label Names

```rust
// ✗ Bad: Unclear, inconsistent casing
"m"
"Method"
"status-code"
"errorType"
```

### Label Cardinality

**Low Cardinality (Good):**
```rust
// HTTP methods: ~9 values (GET, POST, PUT, DELETE, etc.)
labels.insert("method".to_string(), "GET".to_string());

// Status codes: ~60 values (200, 404, 500, etc.)
labels.insert("status".to_string(), "200".to_string());

// Environment: ~3 values (dev, staging, prod)
labels.insert("environment".to_string(), "prod".to_string());
```

**High Cardinality (Avoid):**
```rust
// ✗ User IDs: Millions of possible values
labels.insert("user_id".to_string(), user_id.to_string());

// ✗ Request IDs: Infinite possible values
labels.insert("request_id".to_string(), request_id.to_string());

// ✗ Timestamps: Constantly changing
labels.insert("timestamp".to_string(), timestamp.to_string());
```

**Impact of High Cardinality:**
- Increased memory usage
- Slower query performance
- Higher storage costs
- Potential metric explosion

**Solution: Use Aggregation**
```rust
// Instead of per-user metrics, use aggregates
let total_users = monitor.gauge("active_users_total", ..);
total_users.set(active_user_count);

// Store user-specific data in logs or traces, not metrics
```

### Label Naming Conventions

```rust
// Standard label names (follow Prometheus conventions)
let mut labels = HashMap::new();

// Service identification
labels.insert("service".to_string(), "api".to_string());
labels.insert("instance".to_string(), "api-1".to_string());
labels.insert("job".to_string(), "api-server".to_string());

// HTTP-specific
labels.insert("method".to_string(), "GET".to_string());
labels.insert("endpoint".to_string(), "/api/users".to_string());
labels.insert("status_code".to_string(), "200".to_string());

// Error tracking
labels.insert("error_type".to_string(), "timeout".to_string());
labels.insert("error_code".to_string(), "ETIMEDOUT".to_string());

// Geographic
labels.insert("region".to_string(), "us-east-1".to_string());
labels.insert("zone".to_string(), "us-east-1a".to_string());
```

## Collection Strategies

### Pull-Based Collection

```rust
// Expose metrics endpoint
let monitor = Arc::new(Monitor::new());
monitor.start()?;

// Periodically collect and expose
loop {
    let metrics = monitor.collect();

    // Convert to Prometheus format
    let exporter = PrometheusExporter::new();
    let output = exporter.export(&metrics)?;

    // Serve via HTTP endpoint
    // (e.g., at /metrics)

    std::thread::sleep(Duration::from_secs(15));
}
```

### Push-Based Collection

```rust
use std::time::Duration;

let config = MonitorConfig::new("myapp")
    .with_interval(Duration::from_secs(10))
    .with_auto_collect(true);

let monitor = Monitor::with_config(config);
monitor.start()?;

// Automatic collection every 10 seconds
```

### On-Demand Collection

```rust
// Collect metrics when needed
fn get_metrics_snapshot(monitor: &Monitor) -> Vec<Metric> {
    monitor.collect()
}

// Use in API endpoint
async fn metrics_endpoint(monitor: Arc<Monitor>) -> String {
    let metrics = monitor.collect();
    let exporter = PrometheusExporter::new();
    exporter.export(&metrics).unwrap()
}
```

## Export Formats

### Prometheus Text Format

```rust
use rust_monitoring_system::prelude::*;

let monitor = Monitor::new();
// ... create and update metrics ...

let metrics = monitor.collect();
let exporter = PrometheusExporter::new();
let output = exporter.export(&metrics)?;

println!("{}", output);
```

**Output Example:**
```
# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",endpoint="/api/users"} 1543
http_requests_total{method="POST",endpoint="/api/users"} 234

# HELP active_connections Current number of active connections
# TYPE active_connections gauge
active_connections 42

# HELP request_duration_seconds Request duration
# TYPE request_duration_seconds histogram
request_duration_seconds_bucket{le="0.01"} 123
request_duration_seconds_bucket{le="0.1"} 456
request_duration_seconds_bucket{le="1.0"} 789
request_duration_seconds_bucket{le="+Inf"} 800
request_duration_seconds_sum 234.56
request_duration_seconds_count 800
```

### JSON Format (Custom)

```rust
use serde_json;

let metrics = monitor.collect();
let json = serde_json::to_string_pretty(&metrics)?;
println!("{}", json);
```

**Output Example:**
```json
[
  {
    "name": "http_requests_total",
    "type": "Counter",
    "description": "Total HTTP requests",
    "labels": {
      "method": "GET",
      "endpoint": "/api/users"
    },
    "value": 1543
  }
]
```

## System Metrics

### Using SystemCollector

```rust
use rust_monitoring_system::prelude::*;
use std::sync::Arc;

let monitor = Arc::new(Monitor::new());
monitor.start()?;

// Create system collector
let collector = SystemCollector::new(monitor.clone())?;

// Collect system metrics
collector.collect()?;

// Metrics are now available
let metrics = monitor.collect();
```

### Available System Metrics

```rust
// CPU metrics
system_cpu_usage_percent        // Current CPU usage (0-100)
system_cpu_cores                // Number of CPU cores

// Memory metrics
system_memory_total_bytes       // Total system memory
system_memory_usage_bytes       // Current memory usage
system_memory_available_bytes   // Available memory
system_memory_usage_percent     // Memory usage percentage

// System metrics
system_uptime_seconds           // Monitor uptime in seconds
```

### Custom System Metrics

```rust
use rust_monitoring_system::prelude::*;

struct CustomSystemCollector {
    monitor: Arc<Monitor>,
    disk_usage: Gauge,
    network_rx: Counter,
    network_tx: Counter,
}

impl CustomSystemCollector {
    fn new(monitor: Arc<Monitor>) -> Self {
        let disk_usage = monitor.gauge(
            "system_disk_usage_bytes",
            "Disk usage in bytes",
            HashMap::new()
        );

        let network_rx = monitor.counter(
            "system_network_rx_bytes",
            "Network bytes received",
            HashMap::new()
        );

        let network_tx = monitor.counter(
            "system_network_tx_bytes",
            "Network bytes transmitted",
            HashMap::new()
        );

        Self {
            monitor,
            disk_usage,
            network_rx,
            network_tx,
        }
    }

    fn collect(&self) -> Result<()> {
        // Collect disk usage
        if let Ok(disk_info) = get_disk_info() {
            self.disk_usage.set(disk_info.used_bytes as i64);
        }

        // Collect network stats
        if let Ok(net_info) = get_network_info() {
            self.network_rx.inc_by(net_info.rx_bytes);
            self.network_tx.inc_by(net_info.tx_bytes);
        }

        Ok(())
    }
}
```

## Best Practices

### Naming Conventions

```rust
// ✓ Good metric names
"http_requests_total"           // Counter: _total suffix
"active_connections"            // Gauge: current state
"request_duration_seconds"      // Histogram: _seconds suffix
"response_size_bytes"           // _bytes suffix for sizes

// ✗ Bad metric names
"requests"                      // Unclear if total or current
"latency"                       // Missing unit
"RequestDuration"               // Wrong case
"req_dur_s"                     // Too abbreviated
```

### Unit Suffixes

```rust
// Time durations
"_seconds"
"_milliseconds"
"_microseconds"

// Sizes
"_bytes"
"_kilobytes"
"_megabytes"

// Percentages
"_percent"
"_ratio"      // 0-1 scale

// Counts
"_total"      // Cumulative count (counter)
// (no suffix)  // Current count (gauge)
```

### Metric Lifecycle

```rust
// 1. Create monitor at application startup
let monitor = Arc::new(Monitor::new());
monitor.start()?;

// 2. Create metrics early (cache them)
struct AppMetrics {
    requests: Counter,
    errors: Counter,
    latency: Histogram,
}

impl AppMetrics {
    fn new(monitor: &Monitor) -> Self {
        Self {
            requests: monitor.counter(
                "app_requests_total",
                "Total requests",
                HashMap::new()
            ),
            errors: monitor.counter(
                "app_errors_total",
                "Total errors",
                HashMap::new()
            ),
            latency: monitor.histogram(
                "app_request_duration_seconds",
                "Request duration",
                vec![0.01, 0.1, 0.5, 1.0],
                HashMap::new()
            ),
        }
    }
}

// 3. Use metrics throughout application lifetime
let metrics = AppMetrics::new(&monitor);

// 4. Stop monitor on shutdown
monitor.stop()?;
```

### Performance Considerations

```rust
// ✓ Cache metric instances
struct Handler {
    requests_metric: Counter,  // Cached
}

impl Handler {
    fn handle(&self) {
        self.requests_metric.inc();  // Fast
    }
}

// ✗ Don't recreate metrics
fn handle_request(monitor: &Monitor) {
    let counter = monitor.counter(  // Slow!
        "requests",
        "Requests",
        HashMap::new()
    );
    counter.inc();
}
```

### Error Handling

```rust
// Record both success and failure metrics
match perform_operation() {
    Ok(result) => {
        success_counter.inc();
        duration_histogram.observe(elapsed.as_secs_f64());
    }
    Err(error) => {
        error_counter.inc();

        let mut error_labels = HashMap::new();
        error_labels.insert("type".to_string(), error_type(&error));

        let error_metric = monitor.counter(
            "operation_errors_total",
            "Operation errors",
            error_labels
        );
        error_metric.inc();
    }
}
```

## Integration Examples

### Web Server Integration (Axum)

```rust
use axum::{
    routing::get,
    Router,
    extract::State,
};
use rust_monitoring_system::prelude::*;
use std::sync::Arc;

async fn metrics_handler(
    State(monitor): State<Arc<Monitor>>
) -> String {
    let metrics = monitor.collect();
    let exporter = PrometheusExporter::new();
    exporter.export(&metrics).unwrap_or_default()
}

#[tokio::main]
async fn main() {
    let monitor = Arc::new(Monitor::new());
    monitor.start().unwrap();

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(monitor);

    axum::Server::bind(&"0.0.0.0:9090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

### Middleware Integration

```rust
async fn metrics_middleware(
    State(monitor): State<Arc<Monitor>>,
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status().as_u16();

    // Record metrics
    let mut labels = HashMap::new();
    labels.insert("method".to_string(), method);
    labels.insert("endpoint".to_string(), path);
    labels.insert("status".to_string(), status.to_string());

    let counter = monitor.counter(
        "http_requests_total",
        "HTTP requests",
        labels.clone()
    );
    counter.inc();

    let histogram = monitor.histogram(
        "http_request_duration_seconds",
        "HTTP request duration",
        vec![0.001, 0.01, 0.1, 1.0],
        labels
    );
    histogram.observe(duration.as_secs_f64());

    response
}
```

---

*Metrics Guide Version 1.0*
*Last Updated: 2025-10-16*
