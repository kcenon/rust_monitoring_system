//! Main monitoring interface

use crate::core::error::{MonitoringError, Result};
use crate::core::metric::{Counter, Gauge, Labels, Metric};
use crate::core::registry::MetricRegistry;
use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Monitor configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Service name
    pub service_name: String,
    /// Collection interval
    pub collection_interval: Duration,
    /// Enable automatic collection
    pub auto_collect: bool,
    /// Default labels
    pub default_labels: Labels,
    /// Maximum number of unique metric time series (cardinality limit)
    ///
    /// This protects against memory exhaustion from unbounded metric creation.
    /// When the limit is reached, new metric creation will fail.
    /// Default: 10,000 (a reasonable limit for most applications)
    /// Set to 0 to disable the limit (NOT recommended for production)
    pub max_cardinality: usize,
    /// Time-to-live for inactive metrics
    ///
    /// Metrics that haven't been accessed for this duration will be automatically cleaned up.
    /// Default: 1 hour (recommended for production to prevent memory leaks)
    /// Set to 0 to disable automatic cleanup (NOT recommended for production)
    pub metric_ttl: Duration,
    /// Cleanup interval for expired metrics
    ///
    /// How often to check and remove expired metrics.
    /// Default: 60 seconds
    pub cleanup_interval: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            service_name: "default".to_string(),
            collection_interval: Duration::from_secs(60),
            auto_collect: false,
            default_labels: Labels::new(),
            max_cardinality: 10_000, // Default: 10,000 time series
            metric_ttl: Duration::from_secs(3600), // Default: 1 hour TTL
            cleanup_interval: Duration::from_secs(60), // Default: cleanup every 60 seconds
        }
    }
}

impl MonitorConfig {
    /// Create a new configuration with service name
    #[must_use]
    pub fn new<S: Into<String>>(service_name: S) -> Self {
        Self {
            service_name: service_name.into(),
            ..Default::default()
        }
    }

    /// Set collection interval
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.collection_interval = interval;
        self
    }

    /// Enable automatic collection
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_auto_collect(mut self, enabled: bool) -> Self {
        self.auto_collect = enabled;
        self
    }

    /// Set default labels
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_labels(mut self, labels: Labels) -> Self {
        self.default_labels = labels;
        self
    }

    /// Set maximum cardinality (max number of unique time series)
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_max_cardinality(mut self, max_cardinality: usize) -> Self {
        self.max_cardinality = max_cardinality;
        self
    }

    /// Set metric TTL (time-to-live for inactive metrics)
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_metric_ttl(mut self, ttl: Duration) -> Self {
        self.metric_ttl = ttl;
        self
    }

    /// Set cleanup interval
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_cleanup_interval(mut self, interval: Duration) -> Self {
        self.cleanup_interval = interval;
        self
    }
}

/// Main monitoring system
pub struct Monitor {
    config: Arc<RwLock<MonitorConfig>>,
    registry: MetricRegistry,
    running: Arc<AtomicBool>,
    start_time: Instant,
    cleanup_shutdown: Arc<AtomicBool>,
    cleanup_thread: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Monitor {
    /// Create a new monitor with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(MonitorConfig::default())
    }

    /// Create a new monitor with custom configuration
    #[must_use]
    pub fn with_config(config: MonitorConfig) -> Self {
        let registry = MetricRegistry::with_ttl(config.max_cardinality, config.metric_ttl);
        registry.set_default_labels(config.default_labels.clone());

        Self {
            config: Arc::new(RwLock::new(config)),
            registry,
            running: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
            cleanup_shutdown: Arc::new(AtomicBool::new(false)),
            cleanup_thread: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the metric registry
    pub fn registry(&self) -> &MetricRegistry {
        &self.registry
    }

    /// Start the monitoring system
    pub fn start(&self) -> Result<()> {
        // FIXED: Use compare_exchange to prevent race condition
        // Multiple threads calling start() simultaneously will be serialized
        if self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(MonitoringError::AlreadyInitialized);
        }

        // FIXED: Reset cleanup_shutdown flag to allow restart after stop()
        self.cleanup_shutdown.store(false, Ordering::Release);

        // Start automatic cleanup thread if TTL is enabled
        let cleanup_interval = {
            let config = self.config.read();
            if config.metric_ttl.as_secs() > 0 {
                Some(config.cleanup_interval)
            } else {
                None
            }
        };

        if let Some(cleanup_interval) = cleanup_interval {
            let registry = self.registry.clone();
            let shutdown = Arc::clone(&self.cleanup_shutdown);

            let handle = std::thread::Builder::new()
                .name("metrics-cleanup".to_string())
                .spawn(move || {
                    tracing::debug!("Metrics cleanup thread started");

                    let effective_interval = if cleanup_interval.is_zero() {
                        Duration::from_secs(1)
                    } else {
                        cleanup_interval
                    };
                    let check_interval = Duration::from_millis(500);

                    loop {
                        let mut slept = Duration::ZERO;
                        while slept < effective_interval {
                            let remaining = effective_interval
                                .checked_sub(slept)
                                .unwrap_or(Duration::ZERO);

                            if remaining.is_zero() {
                                break;
                            }

                            let step = if remaining > check_interval {
                                check_interval
                            } else {
                                remaining
                            };

                            std::thread::sleep(step);
                            slept += step;

                            if shutdown.load(Ordering::Acquire) {
                                tracing::debug!("Metrics cleanup thread shutdown complete");
                                return;
                            }
                        }

                        let removed = registry.cleanup_expired();
                        if removed > 0 {
                            tracing::info!(
                                removed = removed,
                                ttl = ?registry.ttl(),
                                "Removed expired metrics"
                            );
                        }
                    }
                })
                .map_err(|e| {
                    // FIXED: Rollback running flag if thread spawn fails
                    self.running.store(false, Ordering::Release);
                    MonitoringError::Other(format!("Failed to spawn cleanup thread: {}", e))
                })?;

            // Store thread handle for proper cleanup
            *self.cleanup_thread.lock() = Some(handle);
        }

        Ok(())
    }

    /// Stop the monitoring system
    pub fn stop(&self) -> Result<()> {
        if !self.running.load(Ordering::Acquire) {
            return Err(MonitoringError::NotInitialized);
        }

        self.running.store(false, Ordering::Release);

        // Signal cleanup thread to shutdown
        self.cleanup_shutdown.store(true, Ordering::Release);

        // Wait for cleanup thread to finish
        if let Some(handle) = self.cleanup_thread.lock().take() {
            // Use a timeout to avoid hanging indefinitely
            let join_timeout = Duration::from_secs(5);
            let start = Instant::now();

            // Try to join with timeout simulation (std::thread::JoinHandle doesn't support timeout directly)
            // We rely on the cleanup thread checking shutdown flag regularly
            match handle.join() {
                Ok(()) => {
                    tracing::debug!("Cleanup thread stopped successfully");
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Cleanup thread panicked during shutdown");
                    // Continue with shutdown even if thread panicked
                }
            }

            let elapsed = start.elapsed();
            if elapsed > join_timeout {
                tracing::warn!(
                    elapsed = ?elapsed,
                    expected = ?join_timeout,
                    "Cleanup thread took longer than expected to stop"
                );
            }
        }

        Ok(())
    }

    /// Check if monitoring is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Get uptime in seconds
    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Register a counter metric
    pub fn register_counter<S: Into<String>>(
        &self,
        name: S,
        help: S,
        labels: Labels,
    ) -> Result<Counter> {
        self.registry.register_counter(name, help, labels)
    }

    /// Register a gauge metric
    pub fn register_gauge<S: Into<String>>(
        &self,
        name: S,
        help: S,
        labels: Labels,
    ) -> Result<Gauge> {
        self.registry.register_gauge(name, help, labels)
    }

    /// Get or create a counter
    ///
    /// Note: This method does not store help text. If you need help text,
    /// use `register_counter()` instead.
    pub fn counter<S: Into<String>>(&self, name: S, labels: Labels) -> Counter {
        self.registry.get_or_create_counter(name, labels)
    }

    /// Get or create a gauge
    ///
    /// Note: This method does not store help text. If you need help text,
    /// use `register_gauge()` instead.
    pub fn gauge<S: Into<String>>(&self, name: S, labels: Labels) -> Gauge {
        self.registry.get_or_create_gauge(name, labels)
    }

    /// Collect all metrics
    pub fn collect(&self) -> Vec<Metric> {
        self.registry.collect()
    }

    /// Get the number of registered metrics
    pub fn metric_count(&self) -> usize {
        self.registry.count()
    }

    /// Get the number of cardinality limit rejections
    ///
    /// This counter tracks how many times metric creation failed due to
    /// reaching the cardinality limit. Each rejection indicates an untracked
    /// metric was returned that won't appear in collection/export.
    ///
    /// **A non-zero count indicates**:
    /// - You may be losing metric data
    /// - You should increase `max_cardinality` in config
    /// - Consider reducing label cardinality
    /// - Use `register_*()` methods that return `Result` for critical metrics
    ///
    /// # Example
    ///
    /// ```
    /// use rust_monitoring_system::Monitor;
    /// use std::collections::HashMap;
    ///
    /// let monitor = Monitor::new();
    /// monitor.start().expect("Failed to start");
    ///
    /// // Create some metrics...
    ///
    /// // Check for rejections
    /// let rejections = monitor.cardinality_rejections();
    /// if rejections > 0 {
    ///     eprintln!("Warning: {} metrics rejected due to cardinality limit", rejections);
    /// }
    /// # monitor.stop().ok();
    /// ```
    pub fn cardinality_rejections(&self) -> u64 {
        self.registry.cardinality_rejections()
    }

    /// Clear all metrics
    pub fn clear(&self) {
        self.registry.clear();
    }

    /// Get configuration
    pub fn config(&self) -> MonitorConfig {
        self.config.read().clone()
    }

    /// Update configuration
    pub fn update_config<F>(&self, f: F)
    where
        F: FnOnce(&mut MonitorConfig),
    {
        let mut config = self.config.write();
        f(&mut config);
        self.registry
            .set_default_labels(config.default_labels.clone());
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Emergency cleanup when Monitor is dropped without explicit stop()
///
/// # Design Decision
///
/// Unlike some systems that avoid cleanup in Drop, Monitor performs emergency cleanup because:
/// 1. **Thread leak prevention**: The cleanup thread must be stopped to prevent resource leaks
/// 2. **Graceful degradation**: If stop() was not called, we still attempt proper cleanup
/// 3. **Diagnostic value**: Drop logs warnings to help identify incorrect usage patterns
///
/// # Behavior
///
/// When a `Monitor` is dropped:
/// - If `stop()` was already called: No-op (thread already joined)
/// - If `stop()` was NOT called: Emergency shutdown with diagnostic logging
///
/// # Best Practices
///
/// ```no_run
/// use rust_monitoring_system::core::Monitor;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let monitor = Monitor::new();
/// monitor.start()?;
///
/// // Use the monitor...
///
/// // ✅ CORRECT: Explicit stop before drop
/// monitor.stop()?;
/// // Drop occurs here with no cleanup needed
///
/// // ❌ LESS IDEAL: Let monitor drop while running
/// // Emergency cleanup will occur, but explicit stop is preferred
/// # Ok(())
/// # }
/// ```
impl Drop for Monitor {
    fn drop(&mut self) {
        // Check if we're still running (stop() was not called)
        if self.running.load(Ordering::Acquire) {
            eprintln!(
                "[MONITOR WARNING] Monitor dropped while still running. \
                 Performing emergency cleanup. \
                 Best practice: call stop() explicitly before dropping."
            );

            // Signal shutdown
            self.cleanup_shutdown.store(true, Ordering::Release);

            // Try to join the cleanup thread
            if let Some(handle) = self.cleanup_thread.lock().take() {
                match handle.join() {
                    Ok(()) => {
                        eprintln!("[MONITOR] Emergency cleanup: thread stopped successfully");
                    }
                    Err(e) => {
                        eprintln!(
                            "[MONITOR ERROR] Emergency cleanup: thread panicked: {:?}",
                            e
                        );
                    }
                }
            }

            self.running.store(false, Ordering::Release);
        }
        // If stop() was already called, cleanup_thread will be None and nothing happens here
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_monitor_lifecycle() {
        let monitor = Monitor::new();

        assert!(!monitor.is_running());

        monitor.start().expect("Failed to start monitor");
        assert!(monitor.is_running());

        monitor.stop().expect("Failed to stop monitor");
        assert!(!monitor.is_running());
    }

    #[test]
    fn test_monitor_restart() {
        let monitor = Monitor::new();

        monitor.start().expect("Failed to start monitor");
        monitor.stop().expect("Failed to stop monitor");

        monitor.start().expect("Failed to restart monitor");
        monitor
            .stop()
            .expect("Failed to stop monitor after restart");
    }

    #[test]
    fn test_monitor_metrics() {
        let monitor = Monitor::new();

        let counter = monitor
            .register_counter("requests", "Request count", HashMap::new())
            .expect("Failed to register counter");

        counter.inc_by(10);

        let metrics = monitor.collect();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "requests");
    }

    #[test]
    fn test_monitor_config() {
        let config = MonitorConfig::new("my_service")
            .with_interval(Duration::from_secs(30))
            .with_auto_collect(true);

        let monitor = Monitor::with_config(config);

        let cfg = monitor.config();
        assert_eq!(cfg.service_name, "my_service");
        assert_eq!(cfg.collection_interval, Duration::from_secs(30));
        assert!(cfg.auto_collect);
    }

    #[test]
    fn test_monitor_uptime() {
        let monitor = Monitor::new();
        std::thread::sleep(Duration::from_millis(100));

        // FIXED: Removed absurd comparison (u64 is always >= 0)
        // The test verifies uptime() can be called without panicking
        // Since we slept 100ms and uptime returns seconds, uptime will be 0
        let _uptime = monitor.uptime();
    }

    #[test]
    fn test_get_or_create() {
        let monitor = Monitor::new();

        let counter1 = monitor.counter("counter", HashMap::new());
        counter1.inc();

        let counter2 = monitor.counter("counter", HashMap::new());
        assert_eq!(counter2.get(), 1);
    }
}
