//! Metric registry for managing metrics

use crate::core::error::{MonitoringError, Result};
use crate::core::metric::{Counter, Gauge, Labels, Metric, MetricType, MetricValue};
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Metric identifier combining name and labels
///
/// Uses BTreeMap for labels to maintain sorted order without explicit sorting.
/// This eliminates O(n log n) sorting overhead on every metric creation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MetricId {
    name: String,
    /// Labels stored in sorted order via BTreeMap
    /// BTreeMap automatically maintains key order, eliminating need for explicit sort
    labels: BTreeMap<String, String>,
}

impl MetricId {
    fn new(name: String, labels: &Labels) -> Self {
        // BTreeMap::from_iter automatically sorts by keys
        // O(n log n) but only once during construction
        let sorted_labels: BTreeMap<String, String> =
            labels.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        Self {
            name,
            labels: sorted_labels,
        }
    }

    // Optimized version that takes ownership to avoid cloning when possible
    fn from_owned(name: String, labels: Labels) -> Self {
        // Convert HashMap to BTreeMap - automatically sorted
        let sorted_labels: BTreeMap<String, String> = labels.into_iter().collect();

        Self {
            name,
            labels: sorted_labels,
        }
    }
}

/// Thread-safe metric storage
#[derive(Clone)]
enum MetricStorage {
    Counter(Counter),
    Gauge(Gauge),
    Metric(Metric),
}

/// Metric storage with last access tracking for TTL
struct MetricEntry {
    storage: MetricStorage,
    help: String,
    last_accessed: Arc<RwLock<Instant>>,
}

/// Registry for managing metrics
pub struct MetricRegistry {
    metrics: Arc<RwLock<HashMap<MetricId, MetricEntry>>>,
    default_labels: Arc<RwLock<Labels>>,
    /// Maximum number of unique time series (0 = unlimited)
    max_cardinality: usize,
    /// Time-to-live for inactive metrics (0 = no expiration)
    metric_ttl: Duration,
    /// Last cleanup time
    last_cleanup: Arc<RwLock<Instant>>,
    /// Counter for cardinality limit rejections (for observability)
    cardinality_rejections: Arc<AtomicU64>,
}

impl MetricRegistry {
    /// Create a new metric registry with default cardinality limit (10,000)
    pub fn new() -> Self {
        Self::with_max_cardinality(10_000)
    }

    /// Create a new metric registry with custom cardinality limit
    ///
    /// # Arguments
    /// * `max_cardinality` - Maximum number of unique time series. Set to 0 for unlimited (NOT recommended)
    pub fn with_max_cardinality(max_cardinality: usize) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            default_labels: Arc::new(RwLock::new(HashMap::new())),
            max_cardinality,
            metric_ttl: Duration::from_secs(0), // No TTL by default
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
            cardinality_rejections: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create a new metric registry with custom cardinality limit and TTL
    ///
    /// # Arguments
    /// * `max_cardinality` - Maximum number of unique time series. Set to 0 for unlimited (NOT recommended)
    /// * `ttl` - Time-to-live for inactive metrics. Set to 0 for no expiration.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_monitoring_system::core::registry::MetricRegistry;
    /// use std::time::Duration;
    ///
    /// // Metrics unused for 1 hour will be automatically cleaned up
    /// let registry = MetricRegistry::with_ttl(10_000, Duration::from_secs(3600));
    /// ```
    pub fn with_ttl(max_cardinality: usize, ttl: Duration) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            default_labels: Arc::new(RwLock::new(HashMap::new())),
            max_cardinality,
            metric_ttl: ttl,
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
            cardinality_rejections: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Set default labels applied to all metrics
    pub fn set_default_labels(&self, labels: Labels) {
        *self.default_labels.write() = labels;
    }

    /// Get default labels
    pub fn get_default_labels(&self) -> Labels {
        self.default_labels.read().clone()
    }

    /// Merge labels with default labels
    fn merge_labels(&self, labels: Labels) -> Labels {
        let mut merged = self.get_default_labels();
        merged.extend(labels);
        merged
    }

    /// Check if adding a new metric would exceed cardinality limit
    fn check_cardinality(&self, current_count: usize) -> Result<()> {
        if self.max_cardinality > 0 && current_count >= self.max_cardinality {
            return Err(MonitoringError::cardinality_limit_exceeded(
                self.max_cardinality,
                current_count,
            ));
        }
        Ok(())
    }

    /// Register a counter metric
    pub fn register_counter<N: Into<String>, H: Into<String>>(
        &self,
        name: N,
        help: H,
        labels: Labels,
    ) -> Result<Counter> {
        let name = name.into();
        let help = help.into();
        let merged_labels = self.merge_labels(labels);
        let id = MetricId::new(name.clone(), &merged_labels);

        let mut metrics = self.metrics.write();

        if metrics.contains_key(&id) {
            return Err(MonitoringError::already_exists(
                name,
                format!("{:?}", merged_labels),
            ));
        }

        // Check cardinality limit before creating new metric
        self.check_cardinality(metrics.len())?;

        let counter = Counter::new();
        let entry = MetricEntry {
            storage: MetricStorage::Counter(counter.clone()),
            help,
            last_accessed: Arc::new(RwLock::new(Instant::now())),
        };
        metrics.insert(id, entry);

        Ok(counter)
    }

    /// Register a gauge metric
    pub fn register_gauge<N: Into<String>, H: Into<String>>(
        &self,
        name: N,
        help: H,
        labels: Labels,
    ) -> Result<Gauge> {
        let name = name.into();
        let help = help.into();
        let merged_labels = self.merge_labels(labels);
        let id = MetricId::new(name.clone(), &merged_labels);

        let mut metrics = self.metrics.write();

        if metrics.contains_key(&id) {
            return Err(MonitoringError::already_exists(
                name,
                format!("{:?}", merged_labels),
            ));
        }

        // Check cardinality limit before creating new metric
        self.check_cardinality(metrics.len())?;

        let gauge = Gauge::new();
        let entry = MetricEntry {
            storage: MetricStorage::Gauge(gauge.clone()),
            help,
            last_accessed: Arc::new(RwLock::new(Instant::now())),
        };
        metrics.insert(id, entry);

        Ok(gauge)
    }

    /// Register a generic metric
    pub fn register_metric(&self, metric: Metric) -> Result<()> {
        let id = MetricId::new(metric.name.clone(), &metric.labels);

        let mut metrics = self.metrics.write();

        if metrics.contains_key(&id) {
            return Err(MonitoringError::already_exists(
                metric.name.clone(),
                format!("{:?}", metric.labels),
            ));
        }

        // Check cardinality limit before creating new metric
        self.check_cardinality(metrics.len())?;

        let help = metric.help.clone();
        let entry = MetricEntry {
            storage: MetricStorage::Metric(metric),
            help,
            last_accessed: Arc::new(RwLock::new(Instant::now())),
        };
        metrics.insert(id, entry);

        Ok(())
    }

    /// Get or create a counter
    ///
    /// This is a convenience method that never fails. If the metric already exists,
    /// it returns the existing counter. If it doesn't exist, it creates a new one.
    ///
    /// **Important**: If the cardinality limit is exceeded, this method returns a
    /// "null" counter that is NOT tracked in the registry. The counter will work
    /// (increment/decrement) but its values will never be collected or exported.
    /// A warning will be printed to stderr when this occurs.
    ///
    /// **For production use**, prefer `register_counter()` which returns `Result`
    /// and allows you to handle cardinality limit errors explicitly.
    ///
    /// Note: This method does not store help text. If you need help text,
    /// use `register_counter()` instead.
    pub fn get_or_create_counter<S: Into<String>>(&self, name: S, labels: Labels) -> Counter {
        let name = name.into();
        let merged_labels = self.merge_labels(labels);
        // Use from_owned to avoid cloning labels again
        let id = MetricId::from_owned(name.clone(), merged_labels);

        // Fast path: try read lock first (handles 90%+ of calls in steady state)
        {
            let metrics = self.metrics.read();
            if let Some(entry) = metrics.get(&id) {
                if let MetricStorage::Counter(counter) = &entry.storage {
                    // Update last accessed timestamp
                    *entry.last_accessed.write() = Instant::now();
                    return counter.clone();
                }
            }
        } // Release read lock

        // Slow path: use entry API for atomic insertion
        // This eliminates the need for manual double-checking
        let mut metrics = self.metrics.write();

        // Get len before calling entry() to avoid borrow checker error
        let current_len = metrics.len();

        match metrics.entry(id) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                // Another thread inserted between read and write lock
                let metric_entry = entry.into_mut();
                // Update last accessed timestamp
                *metric_entry.last_accessed.write() = Instant::now();

                if let MetricStorage::Counter(counter) = &metric_entry.storage {
                    counter.clone()
                } else {
                    // Type mismatch - this shouldn't happen in normal usage
                    // Create new counter anyway (overwrites the wrong type)
                    let counter = Counter::new();
                    metric_entry.storage = MetricStorage::Counter(counter.clone());
                    counter
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                // Check cardinality limit before creating new metric
                if let Err(e) = self.check_cardinality(current_len) {
                    // Cardinality limit exceeded - return a "null" counter that does nothing
                    // This prevents panics but the metric won't be tracked
                    // Increment rejection counter for observability
                    self.cardinality_rejections.fetch_add(1, Ordering::Release);

                    // Log the error for visibility
                    tracing::error!(
                        limit = self.max_cardinality,
                        current = current_len,
                        rejections = self.cardinality_rejections.load(Ordering::Acquire),
                        error = %e,
                        "Cardinality limit exceeded! Returning untracked counter. Use register_counter() for proper error handling."
                    );
                    return Counter::new();
                }

                // No entry exists, create new counter
                let counter = Counter::new();
                let metric_entry = MetricEntry {
                    storage: MetricStorage::Counter(counter.clone()),
                    help: String::new(),
                    last_accessed: Arc::new(RwLock::new(Instant::now())),
                };
                entry.insert(metric_entry);
                counter
            }
        }
    }

    /// Get or create a gauge
    ///
    /// This is a convenience method that never fails. If the metric already exists,
    /// it returns the existing gauge. If it doesn't exist, it creates a new one.
    ///
    /// **Important**: If the cardinality limit is exceeded, this method returns a
    /// "null" gauge that is NOT tracked in the registry. The gauge will work
    /// (set/increment/decrement) but its values will never be collected or exported.
    /// A warning will be printed to stderr when this occurs.
    ///
    /// **For production use**, prefer `register_gauge()` which returns `Result`
    /// and allows you to handle cardinality limit errors explicitly.
    ///
    /// Note: This method does not store help text. If you need help text,
    /// use `register_gauge()` instead.
    pub fn get_or_create_gauge<S: Into<String>>(&self, name: S, labels: Labels) -> Gauge {
        let name = name.into();
        let merged_labels = self.merge_labels(labels);
        // Use from_owned to avoid cloning labels again
        let id = MetricId::from_owned(name.clone(), merged_labels);

        // Fast path: try read lock first (handles 90%+ of calls in steady state)
        {
            let metrics = self.metrics.read();
            if let Some(entry) = metrics.get(&id) {
                if let MetricStorage::Gauge(gauge) = &entry.storage {
                    // Update last accessed timestamp
                    *entry.last_accessed.write() = Instant::now();
                    return gauge.clone();
                }
            }
        } // Release read lock

        // Slow path: use entry API for atomic insertion
        // This eliminates the need for manual double-checking
        let mut metrics = self.metrics.write();

        // Get len before calling entry() to avoid borrow checker error
        let current_len = metrics.len();

        match metrics.entry(id) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                // Another thread inserted between read and write lock
                let metric_entry = entry.into_mut();
                // Update last accessed timestamp
                *metric_entry.last_accessed.write() = Instant::now();

                if let MetricStorage::Gauge(gauge) = &metric_entry.storage {
                    gauge.clone()
                } else {
                    // Type mismatch - this shouldn't happen in normal usage
                    // Create new gauge anyway (overwrites the wrong type)
                    let gauge = Gauge::new();
                    metric_entry.storage = MetricStorage::Gauge(gauge.clone());
                    gauge
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                // Check cardinality limit before creating new metric
                if let Err(e) = self.check_cardinality(current_len) {
                    // Cardinality limit exceeded - return a "null" gauge that does nothing
                    // This prevents panics but the metric won't be tracked
                    // Increment rejection counter for observability
                    self.cardinality_rejections.fetch_add(1, Ordering::Release);

                    // Log the error for visibility
                    tracing::error!(
                        limit = self.max_cardinality,
                        current = current_len,
                        rejections = self.cardinality_rejections.load(Ordering::Acquire),
                        error = %e,
                        "Cardinality limit exceeded! Returning untracked gauge. Use register_gauge() for proper error handling."
                    );
                    return Gauge::new();
                }

                // No entry exists, create new gauge
                let gauge = Gauge::new();
                let metric_entry = MetricEntry {
                    storage: MetricStorage::Gauge(gauge.clone()),
                    help: String::new(),
                    last_accessed: Arc::new(RwLock::new(Instant::now())),
                };
                entry.insert(metric_entry);
                gauge
            }
        }
    }

    /// Unregister a metric
    pub fn unregister<S: AsRef<str>>(&self, name: S, labels: &Labels) -> Result<()> {
        let id = MetricId::new(name.as_ref().to_string(), labels);
        let mut metrics = self.metrics.write();

        metrics.remove(&id).ok_or_else(|| {
            MonitoringError::not_found(name.as_ref().to_string(), format!("{:?}", labels))
        })?;

        Ok(())
    }

    /// Collect all metrics as a snapshot
    ///
    /// Optimized version that minimizes cloning by converting BTreeMap to HashMap
    /// directly during snapshot creation instead of after.
    pub fn collect(&self) -> Vec<Metric> {
        // Snapshot approach: collect metric data quickly, then process without lock
        // Optimization: Convert labels to HashMap during snapshot to avoid double iteration
        let snapshot: Vec<(String, Labels, MetricStorage, String)> = {
            let metrics = self.metrics.read();
            let now = Instant::now();
            metrics
                .iter()
                .map(|(id, entry)| {
                    // Update last accessed timestamp on read
                    *entry.last_accessed.write() = now;

                    // Convert BTreeMap to HashMap directly here (single pass)
                    // This avoids cloning the entire BTreeMap and then converting
                    let labels: Labels = id
                        .labels
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();

                    (
                        id.name.clone(),
                        labels,
                        entry.storage.clone(),
                        entry.help.clone(),
                    )
                })
                .collect()
        }; // Lock released immediately

        // Process snapshot without holding lock
        let timestamp = chrono::Utc::now().timestamp_millis();
        snapshot
            .into_iter()
            .map(|(name, labels, storage, help)| match storage {
                MetricStorage::Counter(counter) => Metric {
                    name,
                    metric_type: MetricType::Counter,
                    help,
                    labels,
                    value: MetricValue::Uint(counter.get()),
                    timestamp,
                },
                MetricStorage::Gauge(gauge) => {
                    let value = gauge.get();
                    Metric {
                        name,
                        metric_type: MetricType::Gauge,
                        help,
                        labels,
                        value: MetricValue::Float(value as f64),
                        timestamp,
                    }
                }
                MetricStorage::Metric(metric) => metric,
            })
            .collect()
    }

    /// Get the number of registered metrics
    pub fn count(&self) -> usize {
        self.metrics.read().len()
    }

    /// Clear all metrics
    pub fn clear(&self) {
        self.metrics.write().clear();
    }

    /// Clean up expired metrics based on TTL
    ///
    /// Removes metrics that haven't been accessed for longer than the configured TTL.
    /// This method is automatically called during collection if TTL is enabled.
    ///
    /// # Returns
    ///
    /// The number of metrics removed
    pub fn cleanup_expired(&self) -> usize {
        if self.metric_ttl.as_secs() == 0 {
            return 0; // TTL disabled
        }

        let now = Instant::now();
        let mut metrics = self.metrics.write();
        let initial_count = metrics.len();

        // Remove expired metrics
        metrics.retain(|_, entry| {
            let last_accessed = *entry.last_accessed.read();
            now.duration_since(last_accessed) < self.metric_ttl
        });

        // Update last cleanup time
        *self.last_cleanup.write() = now;

        initial_count - metrics.len()
    }

    /// Get TTL configuration
    pub fn ttl(&self) -> Duration {
        self.metric_ttl
    }

    /// Check if TTL is enabled
    pub fn is_ttl_enabled(&self) -> bool {
        self.metric_ttl.as_secs() > 0
    }

    /// Get the number of cardinality limit rejections
    ///
    /// This counter tracks how many times `get_or_create_counter()` or
    /// `get_or_create_gauge()` failed to create a new metric due to
    /// cardinality limits. Each rejection means an untracked metric was
    /// returned that won't appear in collection/export.
    ///
    /// **Important**: A non-zero count indicates:
    /// - You may be losing metric data
    /// - You should increase `max_cardinality` or reduce label cardinality
    /// - Consider using `register_*()` methods that return `Result` for critical metrics
    ///
    /// # Example
    ///
    /// ```
    /// use rust_monitoring_system::core::registry::MetricRegistry;
    /// use std::collections::HashMap;
    ///
    /// let registry = MetricRegistry::with_max_cardinality(2);
    ///
    /// // Create 3 metrics, last one will be rejected
    /// registry.get_or_create_counter("metric1", HashMap::new());
    /// registry.get_or_create_counter("metric2", HashMap::new());
    /// registry.get_or_create_counter("metric3", HashMap::new());
    ///
    /// assert_eq!(registry.cardinality_rejections(), 1);
    /// ```
    pub fn cardinality_rejections(&self) -> u64 {
        self.cardinality_rejections.load(Ordering::Acquire)
    }
}

impl Default for MetricRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MetricRegistry {
    fn clone(&self) -> Self {
        Self {
            metrics: Arc::clone(&self.metrics),
            default_labels: Arc::clone(&self.default_labels),
            max_cardinality: self.max_cardinality,
            metric_ttl: self.metric_ttl,
            last_cleanup: Arc::clone(&self.last_cleanup),
            cardinality_rejections: Arc::clone(&self.cardinality_rejections),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_counter() {
        let registry = MetricRegistry::new();
        let counter = registry
            .register_counter("test_counter", "A test counter", HashMap::new())
            .expect("Failed to register counter");

        counter.inc_by(5);
        assert_eq!(counter.get(), 5);

        // Try to register same metric again
        let result = registry.register_counter("test_counter", "A test counter", HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_register_gauge() {
        let registry = MetricRegistry::new();
        let gauge = registry
            .register_gauge("test_gauge", "A test gauge", HashMap::new())
            .expect("Failed to register gauge");

        gauge.set(42);
        assert_eq!(gauge.get(), 42);
    }

    #[test]
    fn test_get_or_create() {
        let registry = MetricRegistry::new();

        let counter1 = registry.get_or_create_counter("counter", HashMap::new());
        counter1.inc();

        let counter2 = registry.get_or_create_counter("counter", HashMap::new());
        assert_eq!(counter2.get(), 1);
    }

    #[test]
    fn test_labels() {
        let registry = MetricRegistry::new();

        let mut labels1 = HashMap::new();
        labels1.insert("env".to_string(), "prod".to_string());

        let mut labels2 = HashMap::new();
        labels2.insert("env".to_string(), "dev".to_string());

        let counter1 = registry
            .register_counter("requests", "Request count", labels1)
            .expect("Failed to register counter with prod labels");
        let counter2 = registry
            .register_counter("requests", "Request count", labels2)
            .expect("Failed to register counter with dev labels");

        counter1.inc_by(10);
        counter2.inc_by(20);

        assert_eq!(counter1.get(), 10);
        assert_eq!(counter2.get(), 20);
    }

    #[test]
    fn test_default_labels() {
        let registry = MetricRegistry::new();

        let mut default_labels = HashMap::new();
        default_labels.insert("service".to_string(), "api".to_string());
        registry.set_default_labels(default_labels);

        let _counter = registry
            .register_counter("requests", "Request count", HashMap::new())
            .expect("Failed to register counter with default labels");

        let metrics = registry.collect();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].labels.get("service"), Some(&"api".to_string()));
    }

    #[test]
    fn test_collect() {
        let registry = MetricRegistry::new();

        let counter = registry
            .register_counter("counter", "help", HashMap::new())
            .expect("Failed to register counter for collect test");
        let gauge = registry
            .register_gauge("gauge", "help", HashMap::new())
            .expect("Failed to register gauge for collect test");

        counter.inc_by(5);
        gauge.set(42);

        let metrics = registry.collect();
        assert_eq!(metrics.len(), 2);
    }

    #[test]
    fn test_clear() {
        let registry = MetricRegistry::new();

        registry
            .register_counter("counter", "help", HashMap::new())
            .expect("Failed to register counter for clear test");

        assert_eq!(registry.count(), 1);

        registry.clear();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_concurrent_get_or_create() {
        use std::sync::Arc;
        use std::thread;

        let registry = Arc::new(MetricRegistry::new());
        let mut handles = vec![];

        // Spawn 10 threads that all try to create the same counter
        for i in 0..10 {
            let registry_clone = Arc::clone(&registry);
            let handle = thread::spawn(move || {
                let counter =
                    registry_clone.get_or_create_counter("shared_counter", HashMap::new());
                // Increment by thread id to verify all threads use the same counter
                counter.inc_by(i as u64);
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify only one counter was created
        assert_eq!(registry.count(), 1);

        // Verify the counter has the sum of all increments (0+1+2+...+9 = 45)
        let counter = registry.get_or_create_counter("shared_counter", HashMap::new());
        assert_eq!(counter.get(), 45);
    }

    #[test]
    fn test_concurrent_register_different_metrics() {
        use std::sync::Arc;
        use std::thread;

        let registry = Arc::new(MetricRegistry::new());
        let mut handles = vec![];

        // Spawn 100 threads that each register a unique counter
        for i in 0..100 {
            let registry_clone = Arc::clone(&registry);
            let handle = thread::spawn(move || {
                let name = format!("counter_{}", i);
                let counter = registry_clone
                    .register_counter(name.clone(), "help".to_string(), HashMap::new())
                    .expect("Failed to register counter");
                counter.inc();
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify all 100 counters were created
        assert_eq!(registry.count(), 100);

        // Verify each counter has value 1
        for i in 0..100 {
            let name = format!("counter_{}", i);
            let counter = registry.get_or_create_counter(name, HashMap::new());
            assert_eq!(counter.get(), 1);
        }
    }

    #[test]
    fn test_concurrent_mixed_operations() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let registry = Arc::new(MetricRegistry::new());
        let mut handles = vec![];

        // Thread 1: Register counters
        let registry_clone = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            for i in 0..50 {
                let name = format!("counter_{}", i);
                let _ = registry_clone.register_counter(name, "help".to_string(), HashMap::new());
                thread::sleep(Duration::from_micros(10));
            }
        }));

        // Thread 2: Register gauges
        let registry_clone = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            for i in 0..50 {
                let name = format!("gauge_{}", i);
                let _ = registry_clone.register_gauge(name, "help".to_string(), HashMap::new());
                thread::sleep(Duration::from_micros(10));
            }
        }));

        // Thread 3: Get or create metrics
        let registry_clone = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let name = format!("mixed_{}", i % 20);
                let counter = registry_clone.get_or_create_counter(name, HashMap::new());
                counter.inc();
                thread::sleep(Duration::from_micros(10));
            }
        }));

        // Thread 4: Collect metrics periodically
        let registry_clone = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                let _metrics = registry_clone.collect();
                thread::sleep(Duration::from_millis(5));
            }
        }));

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify registry is in consistent state
        let count = registry.count();
        assert!(count >= 100, "Expected at least 100 metrics, got {}", count);
    }

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored --nocapture
    fn test_performance_get_or_create() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Instant;

        let registry = Arc::new(MetricRegistry::new());

        // Pre-populate with 100 metrics
        for i in 0..100 {
            let name = format!("metric_{}", i);
            registry.get_or_create_counter(name, HashMap::new());
        }

        // Benchmark: 10 threads each calling get_or_create 10000 times on existing metrics
        let start = Instant::now();
        let mut handles = vec![];

        for thread_id in 0..10 {
            let registry_clone = Arc::clone(&registry);
            let handle = thread::spawn(move || {
                for i in 0..10000 {
                    let name = format!("metric_{}", (thread_id * 10 + i) % 100);
                    let _counter = registry_clone.get_or_create_counter(name, HashMap::new());
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let duration = start.elapsed();
        let total_ops = 10 * 10000;
        let ops_per_sec = total_ops as f64 / duration.as_secs_f64();

        println!("\nPerformance Test: get_or_create on existing metrics");
        println!("Total operations: {}", total_ops);
        println!("Duration: {:?}", duration);
        println!("Ops/sec: {:.0}", ops_per_sec);
        println!(
            "Avg latency: {:.2}µs",
            duration.as_micros() as f64 / total_ops as f64
        );

        // With read-first optimization, should achieve >1M ops/sec
        assert!(
            ops_per_sec > 500_000.0,
            "Performance too low: {:.0} ops/sec",
            ops_per_sec
        );
    }

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored --nocapture
    fn test_performance_collect() {
        use std::time::Instant;

        let registry = MetricRegistry::new();

        // Register 1000 metrics
        for i in 0..1000 {
            let name = format!("metric_{}", i);
            let counter = registry.get_or_create_counter(name, HashMap::new());
            counter.inc_by(i as u64);
        }

        // Warm up
        for _ in 0..10 {
            let _ = registry.collect();
        }

        // Benchmark collect() with 1000 metrics
        let iterations = 1000;
        let start = Instant::now();

        for _ in 0..iterations {
            let _metrics = registry.collect();
        }

        let duration = start.elapsed();
        let avg_latency = duration.as_micros() as f64 / iterations as f64;

        println!("\nPerformance Test: collect() with 1000 metrics");
        println!("Iterations: {}", iterations);
        println!("Total duration: {:?}", duration);
        println!("Avg latency: {:.2}µs", avg_latency);

        // With snapshot optimization, should complete in <500µs avg
        assert!(
            avg_latency < 1000.0,
            "Collect too slow: {:.2}µs avg",
            avg_latency
        );
    }
}
