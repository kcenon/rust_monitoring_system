//! Metric types and definitions

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

/// Type of metric
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    /// Counter metric (monotonically increasing)
    Counter,
    /// Gauge metric (can go up or down)
    Gauge,
    /// Histogram metric (distribution of values)
    Histogram,
    /// Summary metric (statistical summary)
    Summary,
    /// Timer metric (duration measurements)
    Timer,
}

impl MetricType {
    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            MetricType::Counter => "counter",
            MetricType::Gauge => "gauge",
            MetricType::Histogram => "histogram",
            MetricType::Summary => "summary",
            MetricType::Timer => "timer",
        }
    }
}

/// Metric value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    /// Integer value
    Int(i64),
    /// Unsigned integer value
    Uint(u64),
    /// Float value
    Float(f64),
    /// Histogram buckets
    Histogram(HistogramData),
    /// Summary statistics
    Summary(SummaryData),
}

/// Histogram bucket data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramData {
    /// Bucket boundaries
    pub buckets: Vec<f64>,
    /// Count per bucket
    pub counts: Vec<u64>,
    /// Total sum
    pub sum: f64,
    /// Total count
    pub count: u64,
}

impl HistogramData {
    /// Create a new histogram with specified buckets
    pub fn new(buckets: Vec<f64>) -> Self {
        let len = buckets.len();
        Self {
            buckets,
            counts: vec![0; len + 1], // +1 for overflow bucket
            sum: 0.0,
            count: 0,
        }
    }

    /// Observe a value
    pub fn observe(&mut self, value: f64) {
        // Validate input: reject NaN and Infinity to prevent metric corruption
        if !value.is_finite() {
            return;
        }

        // Use saturating arithmetic to prevent overflow
        self.count = self.count.saturating_add(1);

        // Safe f64 addition (finite values won't overflow to Infinity)
        self.sum += value;

        // Find appropriate bucket using binary search (O(log n) instead of O(n))
        // partition_point finds the first bucket where value <= bucket
        // For sorted buckets [0.1, 0.5, 1.0], value 0.3 will get index 1 (bucket 0.5)
        let idx = self.buckets.partition_point(|&bucket| value > bucket);
        self.counts[idx] = self.counts[idx].saturating_add(1);
    }
}

/// Summary statistics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryData {
    /// Total sum
    pub sum: f64,
    /// Total count
    pub count: u64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Mean value
    pub mean: f64,
}

impl SummaryData {
    /// Create a new summary
    pub fn new() -> Self {
        Self {
            sum: 0.0,
            count: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            mean: 0.0,
        }
    }

    /// Observe a value
    pub fn observe(&mut self, value: f64) {
        // Validate input: reject NaN and Infinity to prevent metric corruption
        if !value.is_finite() {
            return;
        }

        // Use saturating arithmetic to prevent overflow
        self.count = self.count.saturating_add(1);

        // Safe f64 addition (finite values won't overflow to Infinity)
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);

        // Safe division: check count > 0 to prevent division by zero
        if self.count > 0 {
            self.mean = self.sum / self.count as f64;
        } else {
            self.mean = 0.0;
        }
    }
}

impl Default for SummaryData {
    fn default() -> Self {
        Self::new()
    }
}

/// Labels attached to a metric
pub type Labels = HashMap<String, String>;

/// A metric with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    /// Metric name
    pub name: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Help text
    pub help: String,
    /// Labels
    pub labels: Labels,
    /// Metric value
    pub value: MetricValue,
    /// Timestamp (Unix timestamp in milliseconds)
    pub timestamp: i64,
}

impl Metric {
    /// Create a new metric
    pub fn new<S: Into<String>>(name: S, metric_type: MetricType, help: S, labels: Labels) -> Self {
        let value = match metric_type {
            MetricType::Counter => MetricValue::Uint(0),
            MetricType::Gauge => MetricValue::Float(0.0),
            MetricType::Histogram => MetricValue::Histogram(HistogramData::new(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ])),
            MetricType::Summary => MetricValue::Summary(SummaryData::new()),
            MetricType::Timer => MetricValue::Summary(SummaryData::new()),
        };

        Self {
            name: name.into(),
            metric_type,
            help: help.into(),
            labels,
            value,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Update timestamp to current time
    pub fn update_timestamp(&mut self) {
        self.timestamp = chrono::Utc::now().timestamp_millis();
    }
}

/// Thread-safe counter metric
#[derive(Debug)]
pub struct Counter {
    value: Arc<AtomicU64>,
}

impl Counter {
    /// Create a new counter
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Increment the counter by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Release);
    }

    /// Increment the counter by a specific amount
    pub fn inc_by(&self, amount: u64) {
        self.value.fetch_add(amount, Ordering::Release);
    }

    /// Get the current value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Acquire)
    }

    /// Reset the counter to zero
    pub fn reset(&self) {
        self.value.store(0, Ordering::Release);
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Counter {
    fn clone(&self) -> Self {
        Self {
            value: Arc::clone(&self.value),
        }
    }
}

/// Thread-safe gauge metric
#[derive(Debug)]
pub struct Gauge {
    value: Arc<AtomicI64>,
}

impl Gauge {
    /// Create a new gauge
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Set the gauge to a specific value
    pub fn set(&self, value: i64) {
        self.value.store(value, Ordering::Release);
    }

    /// Increment the gauge by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Release);
    }

    /// Increment the gauge by a specific amount
    pub fn inc_by(&self, amount: i64) {
        self.value.fetch_add(amount, Ordering::Release);
    }

    /// Decrement the gauge by 1
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Release);
    }

    /// Decrement the gauge by a specific amount
    pub fn dec_by(&self, amount: i64) {
        self.value.fetch_sub(amount, Ordering::Release);
    }

    /// Get the current value
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Acquire)
    }

    /// Reset the gauge to zero
    pub fn reset(&self) {
        self.value.store(0, Ordering::Release);
    }
}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Gauge {
    fn clone(&self) -> Self {
        Self {
            value: Arc::clone(&self.value),
        }
    }
}

/// Thread-safe histogram metric
pub struct Histogram {
    data: Arc<Mutex<HistogramData>>,
}

impl Histogram {
    /// Create a new histogram with default buckets
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HistogramData::new(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]))),
        }
    }

    /// Create a new histogram with custom buckets
    #[must_use]
    pub fn with_buckets(buckets: Vec<f64>) -> Self {
        Self {
            data: Arc::new(Mutex::new(HistogramData::new(buckets))),
        }
    }

    /// Observe a value
    pub fn observe(&self, value: f64) {
        self.data.lock().observe(value);
    }

    /// Get a snapshot of the current histogram data
    pub fn snapshot(&self) -> HistogramData {
        self.data.lock().clone()
    }

    /// Reset the histogram
    pub fn reset(&self) {
        let mut data = self.data.lock();
        let buckets = data.buckets.clone();
        *data = HistogramData::new(buckets);
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Histogram {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}

/// Thread-safe summary metric
pub struct Summary {
    data: Arc<Mutex<SummaryData>>,
}

impl Summary {
    /// Create a new summary
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(SummaryData::new())),
        }
    }

    /// Observe a value
    pub fn observe(&self, value: f64) {
        self.data.lock().observe(value);
    }

    /// Get a snapshot of the current summary data
    pub fn snapshot(&self) -> SummaryData {
        self.data.lock().clone()
    }

    /// Reset the summary
    pub fn reset(&self) {
        *self.data.lock() = SummaryData::new();
    }
}

impl Default for Summary {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Summary {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.inc_by(10);
        assert_eq!(counter.get(), 11);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_gauge() {
        let gauge = Gauge::new();
        assert_eq!(gauge.get(), 0);

        gauge.set(42);
        assert_eq!(gauge.get(), 42);

        gauge.inc();
        assert_eq!(gauge.get(), 43);

        gauge.dec_by(3);
        assert_eq!(gauge.get(), 40);

        gauge.reset();
        assert_eq!(gauge.get(), 0);
    }

    #[test]
    fn test_histogram() {
        let mut hist = HistogramData::new(vec![1.0, 5.0, 10.0]);

        hist.observe(0.5);
        hist.observe(3.0);
        hist.observe(7.0);
        hist.observe(15.0);

        assert_eq!(hist.count, 4);
        assert_eq!(hist.sum, 25.5);
        assert_eq!(hist.counts[0], 1); // 0.5 in first bucket
        assert_eq!(hist.counts[1], 1); // 3.0 in second bucket
        assert_eq!(hist.counts[2], 1); // 7.0 in third bucket
        assert_eq!(hist.counts[3], 1); // 15.0 in overflow bucket
    }

    #[test]
    fn test_summary() {
        let mut summary = SummaryData::new();

        summary.observe(1.0);
        summary.observe(2.0);
        summary.observe(3.0);

        assert_eq!(summary.count, 3);
        assert_eq!(summary.sum, 6.0);
        assert_eq!(summary.min, 1.0);
        assert_eq!(summary.max, 3.0);
        assert_eq!(summary.mean, 2.0);
    }

    #[test]
    fn test_thread_safe_histogram() {
        let hist = Histogram::new();

        hist.observe(0.5);
        hist.observe(3.0);
        hist.observe(7.0);
        hist.observe(15.0);

        let snapshot = hist.snapshot();
        assert_eq!(snapshot.count, 4);
        assert_eq!(snapshot.sum, 25.5);

        hist.reset();
        let snapshot = hist.snapshot();
        assert_eq!(snapshot.count, 0);
        assert_eq!(snapshot.sum, 0.0);
    }

    #[test]
    fn test_thread_safe_summary() {
        let summary = Summary::new();

        summary.observe(1.0);
        summary.observe(2.0);
        summary.observe(3.0);

        let snapshot = summary.snapshot();
        assert_eq!(snapshot.count, 3);
        assert_eq!(snapshot.sum, 6.0);
        assert_eq!(snapshot.min, 1.0);
        assert_eq!(snapshot.max, 3.0);
        assert_eq!(snapshot.mean, 2.0);

        summary.reset();
        let snapshot = summary.snapshot();
        assert_eq!(snapshot.count, 0);
    }

    #[test]
    fn test_concurrent_histogram() {
        use std::thread;

        let hist = Histogram::new();
        let mut handles = vec![];

        for i in 0..10 {
            let hist_clone = hist.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    hist_clone.observe(i as f64);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let snapshot = hist.snapshot();
        assert_eq!(snapshot.count, 1000);
    }

    #[test]
    fn test_concurrent_summary() {
        use std::thread;

        let summary = Summary::new();
        let mut handles = vec![];

        for i in 1..=10 {
            let summary_clone = summary.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    summary_clone.observe(i as f64);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let snapshot = summary.snapshot();
        assert_eq!(snapshot.count, 1000);
    }
}
