# Rust Monitoring System - Improvement Plan

> **Languages**: English | [한국어](./IMPROVEMENTS.ko.md)

## Overview

This document outlines proposed improvements and enhancements for the Rust Monitoring System. While the current implementation is solid with good use of Arc and clear separation of concerns, there are opportunities to enhance functionality, performance, and production readiness.

## Enhancement Opportunities

### 1. Metric Aggregation and Downsampling

**Opportunity**: Add support for metric aggregation windows and downsampling to reduce storage and improve query performance for long-term metrics.

**Current State**:
```rust
// Metrics are collected at full resolution
// No built-in aggregation or downsampling
pub fn collect(&self) -> Vec<Metric> {
    // Returns all raw metrics
}
```

**Proposed Enhancement**:

```rust
// TODO: Add metric aggregation and downsampling support

#[derive(Debug, Clone)]
pub enum AggregationWindow {
    Seconds(u64),
    Minutes(u64),
    Hours(u64),
    Days(u64),
}

#[derive(Debug, Clone)]
pub enum AggregationFunction {
    Sum,
    Average,
    Min,
    Max,
    Count,
    P50,    // 50th percentile
    P95,    // 95th percentile
    P99,    // 99th percentile
}

pub struct AggregationConfig {
    pub window: AggregationWindow,
    pub functions: Vec<AggregationFunction>,
    pub keep_raw: bool,  // Whether to keep raw metrics alongside aggregates
}

pub struct MetricAggregator {
    config: AggregationConfig,
    buffers: HashMap<String, MetricBuffer>,
}

impl MetricAggregator {
    pub fn aggregate(&mut self, metrics: Vec<Metric>) -> Vec<AggregatedMetric> {
        let now = SystemTime::now();
        let window_start = self.calculate_window_start(now);

        for metric in metrics {
            let buffer = self.buffers
                .entry(metric.name.clone())
                .or_insert_with(|| MetricBuffer::new(window_start));

            buffer.push(metric);
        }

        // Flush completed windows
        self.flush_completed_windows(now)
    }

    fn flush_completed_windows(&mut self, now: SystemTime) -> Vec<AggregatedMetric> {
        let mut aggregated = Vec::new();

        for (name, buffer) in &mut self.buffers {
            if buffer.window_complete(now, &self.config.window) {
                let window_metrics = buffer.drain();

                for func in &self.config.functions {
                    let value = self.apply_function(func, &window_metrics);
                    aggregated.push(AggregatedMetric {
                        name: format!("{}_{:?}", name, func),
                        value,
                        window_start: buffer.window_start,
                        window_end: now,
                        function: func.clone(),
                    });
                }
            }
        }

        aggregated
    }

    fn apply_function(&self, func: &AggregationFunction, metrics: &[Metric]) -> f64 {
        match func {
            AggregationFunction::Sum => {
                metrics.iter().map(|m| m.value()).sum()
            }
            AggregationFunction::Average => {
                let sum: f64 = metrics.iter().map(|m| m.value()).sum();
                sum / metrics.len() as f64
            }
            AggregationFunction::Min => {
                metrics.iter()
                    .map(|m| m.value())
                    .fold(f64::INFINITY, f64::min)
            }
            AggregationFunction::Max => {
                metrics.iter()
                    .map(|m| m.value())
                    .fold(f64::NEG_INFINITY, f64::max)
            }
            AggregationFunction::P95 => {
                self.calculate_percentile(metrics, 0.95)
            }
            AggregationFunction::P99 => {
                self.calculate_percentile(metrics, 0.99)
            }
            _ => 0.0,
        }
    }

    fn calculate_percentile(&self, metrics: &[Metric], percentile: f64) -> f64 {
        let mut values: Vec<f64> = metrics.iter().map(|m| m.value()).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let index = (values.len() as f64 * percentile) as usize;
        values.get(index).copied().unwrap_or(0.0)
    }
}

// Usage:
let aggregator = MetricAggregator::new(AggregationConfig {
    window: AggregationWindow::Minutes(5),
    functions: vec![
        AggregationFunction::Average,
        AggregationFunction::P95,
        AggregationFunction::Max,
    ],
    keep_raw: false,
});

let aggregated = aggregator.aggregate(monitor.collect());
```

**Benefits**:
- Reduced storage requirements for long-term metrics
- Faster queries on historical data
- Automatic calculation of percentiles and statistics
- Configurable retention policies

**Priority**: Medium
**Estimated Effort**: Large (2-3 weeks)

### 2. Alerting and Threshold Monitoring

**Opportunity**: Add built-in alerting capabilities to notify operators when metrics exceed thresholds or exhibit anomalous behavior.

**Proposed Enhancement**:

```rust
// TODO: Add alerting and threshold monitoring

#[derive(Debug, Clone)]
pub enum Threshold {
    Above(f64),
    Below(f64),
    Range { min: f64, max: f64 },
    RateOfChange { max_delta: f64, window: Duration },
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

pub struct AlertRule {
    pub name: String,
    pub metric_pattern: String,  // Glob pattern for metric names
    pub threshold: Threshold,
    pub severity: AlertSeverity,
    pub duration: Option<Duration>,  // Sustained violation duration
    pub cooldown: Duration,          // Time between repeated alerts
}

pub struct Alert {
    pub rule: AlertRule,
    pub metric_name: String,
    pub current_value: f64,
    pub timestamp: SystemTime,
    pub message: String,
}

pub struct AlertManager {
    rules: Vec<AlertRule>,
    alert_states: HashMap<String, AlertState>,
    handlers: Vec<Box<dyn AlertHandler>>,
}

struct AlertState {
    violation_started: Option<SystemTime>,
    last_alert_sent: Option<SystemTime>,
}

pub trait AlertHandler: Send + Sync {
    fn handle(&self, alert: &Alert);
}

impl AlertManager {
    pub fn add_rule(&mut self, rule: AlertRule) {
        self.rules.push(rule);
    }

    pub fn add_handler(&mut self, handler: Box<dyn AlertHandler>) {
        self.handlers.push(handler);
    }

    pub fn check_metrics(&mut self, metrics: &[Metric]) {
        let now = SystemTime::now();

        for rule in &self.rules {
            for metric in metrics {
                if self.matches_pattern(&metric.name, &rule.metric_pattern) {
                    self.check_metric(rule, metric, now);
                }
            }
        }
    }

    fn check_metric(&mut self, rule: &AlertRule, metric: &Metric, now: SystemTime) {
        let violates = match &rule.threshold {
            Threshold::Above(limit) => metric.value() > *limit,
            Threshold::Below(limit) => metric.value() < *limit,
            Threshold::Range { min, max } => {
                metric.value() < *min || metric.value() > *max
            }
            Threshold::RateOfChange { max_delta, window } => {
                self.check_rate_of_change(metric, *max_delta, *window)
            }
        };

        let state_key = format!("{}:{}", rule.name, metric.name);
        let state = self.alert_states
            .entry(state_key.clone())
            .or_insert_with(|| AlertState {
                violation_started: None,
                last_alert_sent: None,
            });

        if violates {
            // Mark violation start time
            if state.violation_started.is_none() {
                state.violation_started = Some(now);
            }

            // Check if sustained duration met
            let violation_duration = now
                .duration_since(state.violation_started.unwrap())
                .unwrap();

            let should_alert = if let Some(required_duration) = rule.duration {
                violation_duration >= required_duration
            } else {
                true
            };

            // Check cooldown
            let cooldown_elapsed = state.last_alert_sent
                .map(|last| now.duration_since(last).unwrap() >= rule.cooldown)
                .unwrap_or(true);

            if should_alert && cooldown_elapsed {
                let alert = Alert {
                    rule: rule.clone(),
                    metric_name: metric.name.clone(),
                    current_value: metric.value(),
                    timestamp: now,
                    message: format!(
                        "Metric {} {} (current: {})",
                        metric.name,
                        self.format_threshold_violation(&rule.threshold, metric.value()),
                        metric.value()
                    ),
                };

                // Send to all handlers
                for handler in &self.handlers {
                    handler.handle(&alert);
                }

                state.last_alert_sent = Some(now);
            }
        } else {
            // Reset violation state
            state.violation_started = None;
        }
    }
}

// Example alert handlers
pub struct LogAlertHandler;

impl AlertHandler for LogAlertHandler {
    fn handle(&self, alert: &Alert) {
        log::warn!(
            "[{:?}] {}",
            alert.rule.severity,
            alert.message
        );
    }
}

pub struct EmailAlertHandler {
    smtp_config: SmtpConfig,
    recipients: Vec<String>,
}

impl AlertHandler for EmailAlertHandler {
    fn handle(&self, alert: &Alert) {
        // Send email notification
        self.send_email(alert);
    }
}

pub struct WebhookAlertHandler {
    url: String,
}

impl AlertHandler for WebhookAlertHandler {
    fn handle(&self, alert: &Alert) {
        // POST alert to webhook
        self.post_webhook(alert);
    }
}

// Usage:
let mut alert_mgr = AlertManager::new();

alert_mgr.add_rule(AlertRule {
    name: "high_cpu".to_string(),
    metric_pattern: "system_cpu_usage_percent".to_string(),
    threshold: Threshold::Above(80.0),
    severity: AlertSeverity::Warning,
    duration: Some(Duration::from_secs(60)),  // Sustained for 1 minute
    cooldown: Duration::from_secs(300),       // Alert every 5 minutes max
});

alert_mgr.add_handler(Box::new(LogAlertHandler));
alert_mgr.add_handler(Box::new(WebhookAlertHandler {
    url: "https://alerts.example.com/webhook".to_string(),
}));

// Check metrics periodically
let metrics = monitor.collect();
alert_mgr.check_metrics(&metrics);
```

**Benefits**:
- Proactive problem detection
- Automated operator notifications
- Configurable alert rules without code changes
- Multiple notification channels (email, Slack, PagerDuty, etc.)

**Priority**: High
**Estimated Effort**: Large (2-3 weeks)

### 3. Metric Persistence and Historical Queries

**Opportunity**: Add persistence layer to store metrics for historical analysis and graphing.

**Proposed Enhancement**:

```rust
// TODO: Add metric persistence and historical query support

pub trait MetricStorage: Send + Sync {
    fn write(&mut self, metrics: &[Metric]) -> Result<(), StorageError>;
    fn query(&self, query: &MetricQuery) -> Result<Vec<Metric>, StorageError>;
}

#[derive(Debug, Clone)]
pub struct MetricQuery {
    pub metric_name: String,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub labels: HashMap<String, String>,  // Filter by labels
    pub aggregation: Option<AggregationFunction>,
}

// Time-series database backend
pub struct TimeSeriesStorage {
    db: Connection,  // SQLite, PostgreSQL, TimescaleDB, etc.
}

impl MetricStorage for TimeSeriesStorage {
    fn write(&mut self, metrics: &[Metric]) -> Result<(), StorageError> {
        let tx = self.db.transaction()?;

        for metric in metrics {
            tx.execute(
                "INSERT INTO metrics (name, value, timestamp, labels) VALUES (?1, ?2, ?3, ?4)",
                params![
                    metric.name,
                    metric.value(),
                    metric.timestamp,
                    serde_json::to_string(&metric.labels)?,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn query(&self, query: &MetricQuery) -> Result<Vec<Metric>, StorageError> {
        let mut stmt = self.db.prepare(
            "SELECT name, value, timestamp, labels
             FROM metrics
             WHERE name = ?1
               AND timestamp BETWEEN ?2 AND ?3
             ORDER BY timestamp ASC"
        )?;

        let metrics = stmt.query_map(
            params![
                query.metric_name,
                query.start_time,
                query.end_time,
            ],
            |row| {
                Ok(Metric {
                    name: row.get(0)?,
                    value: row.get(1)?,
                    timestamp: row.get(2)?,
                    labels: serde_json::from_str(&row.get::<_, String>(3)?).unwrap(),
                })
            },
        )?;

        metrics.collect()
    }
}

// Monitor integration
impl Monitor {
    pub fn with_storage(storage: Box<dyn MetricStorage>) -> Self {
        // ... initialization
    }

    pub fn collect_and_store(&mut self) -> Result<(), StorageError> {
        let metrics = self.collect();
        self.storage.write(&metrics)?;
        Ok(())
    }

    pub fn query_historical(&self, query: MetricQuery) -> Result<Vec<Metric>, StorageError> {
        self.storage.query(&query)
    }
}

// Usage:
let storage = Box::new(TimeSeriesStorage::new("metrics.db")?);
let mut monitor = Monitor::with_storage(storage);

// Collect and store
monitor.collect_and_store()?;

// Query historical data
let query = MetricQuery {
    metric_name: "system_cpu_usage_percent".to_string(),
    start_time: SystemTime::now() - Duration::from_secs(3600),  // Last hour
    end_time: SystemTime::now(),
    labels: HashMap::new(),
    aggregation: Some(AggregationFunction::Average),
};

let historical_metrics = monitor.query_historical(query)?;
```

**Benefits**:
- Long-term metric retention
- Historical trend analysis
- Capacity planning support
- Incident retrospectives

**Priority**: Medium
**Estimated Effort**: Large (2-3 weeks)

### 4. Metric Cardinality Protection

**Opportunity**: Prevent metric explosion from unbounded label values.

**Proposed Enhancement**:

```rust
// TODO: Add cardinality limits to prevent metric explosion

pub struct CardinalityConfig {
    pub max_metrics: usize,
    pub max_label_values: usize,
    pub warn_threshold: f64,  // Warn at % of max
}

pub struct CardinalityTracker {
    config: CardinalityConfig,
    metric_counts: HashMap<String, HashSet<LabelSet>>,
}

impl CardinalityTracker {
    pub fn check_cardinality(
        &mut self,
        metric_name: &str,
        labels: &HashMap<String, String>,
    ) -> Result<(), CardinalityError> {
        let label_set = LabelSet::from(labels);

        let metric_variants = self.metric_counts
            .entry(metric_name.to_string())
            .or_insert_with(HashSet::new);

        // Check if adding this variant exceeds limits
        if !metric_variants.contains(&label_set) {
            if metric_variants.len() >= self.config.max_label_values {
                return Err(CardinalityError::TooManyVariants {
                    metric_name: metric_name.to_string(),
                    current: metric_variants.len(),
                    max: self.config.max_label_values,
                });
            }

            // Warn if approaching limit
            let usage_percent = metric_variants.len() as f64
                / self.config.max_label_values as f64;

            if usage_percent >= self.config.warn_threshold {
                log::warn!(
                    "Metric {} cardinality at {:.1}% ({}/{})",
                    metric_name,
                    usage_percent * 100.0,
                    metric_variants.len(),
                    self.config.max_label_values
                );
            }

            metric_variants.insert(label_set);
        }

        // Check total metric count
        let total_variants: usize = self.metric_counts
            .values()
            .map(|v| v.len())
            .sum();

        if total_variants >= self.config.max_metrics {
            return Err(CardinalityError::TooManyMetrics {
                current: total_variants,
                max: self.config.max_metrics,
            });
        }

        Ok(())
    }

    pub fn report(&self) -> CardinalityReport {
        let mut top_metrics: Vec<_> = self.metric_counts
            .iter()
            .map(|(name, variants)| (name.clone(), variants.len()))
            .collect();

        top_metrics.sort_by(|a, b| b.1.cmp(&a.1));

        CardinalityReport {
            total_metrics: self.metric_counts.len(),
            total_variants: top_metrics.iter().map(|(_, count)| count).sum(),
            top_metrics: top_metrics.into_iter().take(10).collect(),
        }
    }
}

#[derive(Debug)]
pub enum CardinalityError {
    TooManyVariants { metric_name: String, current: usize, max: usize },
    TooManyMetrics { current: usize, max: usize },
}

// Integrate with Monitor
impl Monitor {
    pub fn counter_checked(
        &self,
        name: &str,
        help: &str,
        labels: HashMap<String, String>,
    ) -> Result<Counter, CardinalityError> {
        self.cardinality_tracker.check_cardinality(name, &labels)?;
        Ok(self.counter(name, help, labels))
    }
}
```

**Benefits**:
- Prevents memory exhaustion from metric explosion
- Early warning of cardinality issues
- Enforces best practices for metric labeling
- Visibility into cardinality usage

**Priority**: High
**Estimated Effort**: Medium (1 week)

## Additional Enhancements

### 5. Push Gateway Support

**Suggestion**: Add support for pushing metrics to Prometheus Push Gateway for short-lived jobs:

```rust
// TODO: Add Prometheus Push Gateway support

pub struct PushGatewayConfig {
    pub url: String,
    pub job_name: String,
    pub instance: Option<String>,
    pub auth: Option<BasicAuth>,
}

impl Monitor {
    pub fn push_to_gateway(&self, config: &PushGatewayConfig) -> Result<(), PushError> {
        let metrics = self.collect();
        let exporter = PrometheusExporter::new();
        let body = exporter.export(&metrics)?;

        let url = format!(
            "{}/metrics/job/{}/instance/{}",
            config.url,
            config.job_name,
            config.instance.as_deref().unwrap_or("unknown")
        );

        let client = reqwest::blocking::Client::new();
        let mut request = client.post(&url).body(body);

        if let Some(auth) = &config.auth {
            request = request.basic_auth(&auth.username, Some(&auth.password));
        }

        request.send()?;
        Ok(())
    }
}
```

**Priority**: Low
**Estimated Effort**: Small (2-3 days)

### 6. OpenTelemetry Integration

**Suggestion**: Add OpenTelemetry exporter for cloud-native observability:

```rust
// TODO: Add OpenTelemetry exporter

pub struct OtelExporter {
    endpoint: String,
    service_name: String,
}

impl OtelExporter {
    pub fn export(&self, metrics: &[Metric]) -> Result<(), OtelError> {
        // Convert to OpenTelemetry format
        // Send via OTLP protocol
    }
}
```

**Priority**: Low
**Estimated Effort**: Medium (1 week)

## Testing Requirements

### New Tests Needed:

1. **Aggregation Tests**:
   ```rust
   #[test]
   fn test_metric_aggregation() {
       let mut aggregator = MetricAggregator::new(AggregationConfig {
           window: AggregationWindow::Seconds(60),
           functions: vec![AggregationFunction::Average, AggregationFunction::P95],
           keep_raw: false,
       });

       // Generate metrics over time
       let metrics = generate_test_metrics(100);
       let aggregated = aggregator.aggregate(metrics);

       assert!(!aggregated.is_empty());
       assert!(aggregated.iter().any(|m| m.name.contains("Average")));
       assert!(aggregated.iter().any(|m| m.name.contains("P95")));
   }
   ```

2. **Alert Tests**:
   ```rust
   #[test]
   fn test_alert_threshold() {
       let mut alert_mgr = AlertManager::new();
       let mut alerts_received = Vec::new();

       alert_mgr.add_rule(AlertRule {
           name: "test_alert".to_string(),
           metric_pattern: "test_metric".to_string(),
           threshold: Threshold::Above(100.0),
           severity: AlertSeverity::Warning,
           duration: None,
           cooldown: Duration::from_secs(0),
       });

       alert_mgr.add_handler(Box::new(move |alert: &Alert| {
           alerts_received.push(alert.clone());
       }));

       // Metric below threshold - no alert
       alert_mgr.check_metrics(&[Metric::new("test_metric", 50.0)]);
       assert_eq!(alerts_received.len(), 0);

       // Metric above threshold - alert
       alert_mgr.check_metrics(&[Metric::new("test_metric", 150.0)]);
       assert_eq!(alerts_received.len(), 1);
   }
   ```

3. **Cardinality Tests**:
   ```rust
   #[test]
   fn test_cardinality_limits() {
       let mut tracker = CardinalityTracker::new(CardinalityConfig {
           max_metrics: 100,
           max_label_values: 10,
           warn_threshold: 0.8,
       });

       // Should succeed for first 10 variants
       for i in 0..10 {
           let mut labels = HashMap::new();
           labels.insert("id".to_string(), i.to_string());
           assert!(tracker.check_cardinality("test", &labels).is_ok());
       }

       // Should fail for 11th variant
       let mut labels = HashMap::new();
       labels.insert("id".to_string(), "11".to_string());
       assert!(tracker.check_cardinality("test", &labels).is_err());
   }
   ```

## Implementation Roadmap

### Phase 1: Core Enhancements (Sprint 1-2)
- [ ] Implement cardinality protection
- [ ] Add alerting framework
- [ ] Create alert handler implementations
- [ ] Add comprehensive tests

### Phase 2: Aggregation and Storage (Sprint 3-4)
- [ ] Implement metric aggregation
- [ ] Add persistence layer
- [ ] Support historical queries
- [ ] Add downsampling

### Phase 3: Integration and Export (Sprint 5)
- [ ] Add Push Gateway support
- [ ] Implement OpenTelemetry exporter
- [ ] Create integration examples
- [ ] Update documentation

## References

- Code Analysis: Monitoring System Review 2025-10-16
- Prometheus best practices: https://prometheus.io/docs/practices/naming/
- OpenTelemetry specification: https://opentelemetry.io/docs/specs/otel/
- Time-series database comparisons: https://prometheus.io/docs/prometheus/latest/storage/

---

*Improvement Plan Version 1.0*
*Last Updated: 2025-10-17*
