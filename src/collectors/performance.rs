//! Performance monitoring collector

use crate::core::MetricRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, System};

/// Performance metrics collector
pub struct PerformanceCollector {
    registry: Arc<MetricRegistry>,
    system: Arc<parking_lot::RwLock<System>>,
    enabled: bool,
}

impl PerformanceCollector {
    /// Create a new performance collector
    pub fn new(registry: Arc<MetricRegistry>) -> Self {
        Self {
            registry,
            system: Arc::new(parking_lot::RwLock::new(System::new_all())),
            enabled: true,
        }
    }

    /// Enable or disable collection
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Collect all performance metrics
    pub fn collect(&self) {
        if !self.enabled {
            return;
        }

        self.collect_cpu_metrics();
        self.collect_memory_metrics();
        self.collect_process_metrics();
    }

    /// Collect CPU metrics
    fn collect_cpu_metrics(&self) {
        let mut system = self.system.write();
        system.refresh_cpu_all();

        // Global CPU usage
        let global_cpu = system.global_cpu_usage() as i64;
        self.registry
            .get_or_create_gauge("system_cpu_usage_percent", HashMap::new())
            .set(global_cpu);

        // Per-CPU usage
        for (i, cpu) in system.cpus().iter().enumerate() {
            let mut labels = HashMap::new();
            labels.insert("cpu".to_string(), i.to_string());

            let usage = cpu.cpu_usage() as i64;
            self.registry
                .get_or_create_gauge("system_cpu_core_usage_percent", labels)
                .set(usage);
        }
    }

    /// Collect memory metrics
    fn collect_memory_metrics(&self) {
        let mut system = self.system.write();
        system.refresh_memory();

        // Total memory
        self.registry
            .get_or_create_gauge("system_memory_total_bytes", HashMap::new())
            .set(system.total_memory() as i64);

        // Used memory
        self.registry
            .get_or_create_gauge("system_memory_used_bytes", HashMap::new())
            .set(system.used_memory() as i64);

        // Available memory
        self.registry
            .get_or_create_gauge("system_memory_available_bytes", HashMap::new())
            .set(system.available_memory() as i64);

        // Memory usage percentage (protect against division by zero)
        let usage_percent = if system.total_memory() > 0 {
            (system.used_memory() as f64 / system.total_memory() as f64 * 100.0) as i64
        } else {
            0
        };
        self.registry
            .get_or_create_gauge("system_memory_usage_percent", HashMap::new())
            .set(usage_percent);

        // Swap memory
        self.registry
            .get_or_create_gauge("system_swap_total_bytes", HashMap::new())
            .set(system.total_swap() as i64);

        self.registry
            .get_or_create_gauge("system_swap_used_bytes", HashMap::new())
            .set(system.used_swap() as i64);
    }

    /// Collect process-specific metrics
    fn collect_process_metrics(&self) {
        let mut system = self.system.write();
        system.refresh_processes(ProcessesToUpdate::All);

        // Get PID safely - return early if unavailable (rare but possible in sandboxed environments)
        let pid = match sysinfo::get_current_pid() {
            Ok(pid) => pid,
            Err(_) => {
                tracing::warn!("Failed to get current process PID - skipping process metrics");
                return;
            }
        };

        if let Some(process) = system.process(pid) {
            // Process CPU usage
            let cpu_usage = process.cpu_usage() as i64;
            self.registry
                .get_or_create_gauge("process_cpu_usage_percent", HashMap::new())
                .set(cpu_usage);

            // Process memory usage
            self.registry
                .get_or_create_gauge("process_memory_bytes", HashMap::new())
                .set(process.memory() as i64);

            // Virtual memory
            self.registry
                .get_or_create_gauge("process_virtual_memory_bytes", HashMap::new())
                .set(process.virtual_memory() as i64);

            // Thread count
            // Note: sysinfo doesn't provide thread count directly
            // This would need platform-specific implementation
        }
    }

    /// Start automatic collection in background
    ///
    /// Returns a shutdown handle that can be used to stop the collector thread.
    /// When the handle is dropped or `stop()` is called, the background thread will exit gracefully.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_monitoring_system::collectors::PerformanceCollector;
    /// use rust_monitoring_system::MetricRegistry;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// let registry = Arc::new(MetricRegistry::new());
    /// let collector = Arc::new(PerformanceCollector::new(registry));
    /// let handle = collector.start_auto_collect(Duration::from_secs(5));
    ///
    /// // Later, stop the collector
    /// handle.stop();
    /// ```
    pub fn start_auto_collect(self: Arc<Self>, interval: Duration) -> AutoCollectHandle {
        use std::sync::atomic::{AtomicBool, Ordering};

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = std::thread::spawn(move || {
            while !shutdown_clone.load(Ordering::Acquire) {
                self.collect();

                // Sleep in small increments to allow faster shutdown response
                let sleep_increments = interval.as_millis() / 100;
                for _ in 0..sleep_increments.max(1) {
                    if shutdown_clone.load(Ordering::Acquire) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });

        AutoCollectHandle {
            shutdown,
            handle: Some(handle),
        }
    }
}

/// Handle for controlling auto-collection background thread
///
/// When dropped, signals the background thread to shutdown and waits for it to exit.
pub struct AutoCollectHandle {
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl AutoCollectHandle {
    /// Stop the auto-collection thread and wait for it to exit
    pub fn stop(mut self) {
        use std::sync::atomic::Ordering;

        self.shutdown.store(true, Ordering::Release);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AutoCollectHandle {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;

        self.shutdown.store(true, Ordering::Release);

        if let Some(handle) = self.handle.take() {
            // Wait for thread to finish with timeout
            const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

            let start = std::time::Instant::now();
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    break;
                }

                if start.elapsed() >= SHUTDOWN_TIMEOUT {
                    tracing::warn!(
                        "Auto-collect thread did not finish within {}s timeout during drop",
                        SHUTDOWN_TIMEOUT.as_secs()
                    );
                    break;
                }

                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// Runtime performance metrics
pub struct RuntimeMetrics {
    registry: Arc<MetricRegistry>,
}

impl RuntimeMetrics {
    /// Create a new runtime metrics collector
    pub fn new(registry: Arc<MetricRegistry>) -> Self {
        Self { registry }
    }

    /// Record request duration
    pub fn record_request_duration(&self, operation: &str, duration_ms: u64) {
        let mut labels = HashMap::new();
        labels.insert("operation".to_string(), operation.to_string());

        self.registry
            .get_or_create_counter("request_duration_ms_total", labels.clone())
            .inc_by(duration_ms);

        self.registry
            .get_or_create_counter("request_count_total", labels)
            .inc();
    }

    /// Record error
    pub fn record_error(&self, operation: &str, error_type: &str) {
        let mut labels = HashMap::new();
        labels.insert("operation".to_string(), operation.to_string());
        labels.insert("error_type".to_string(), error_type.to_string());

        self.registry
            .get_or_create_counter("error_count_total", labels)
            .inc();
    }

    /// Record success
    pub fn record_success(&self, operation: &str) {
        let mut labels = HashMap::new();
        labels.insert("operation".to_string(), operation.to_string());
        labels.insert("status".to_string(), "success".to_string());

        self.registry
            .get_or_create_counter("operation_count_total", labels)
            .inc();
    }

    /// Set active connections
    pub fn set_active_connections(&self, count: i64) {
        self.registry
            .get_or_create_gauge("active_connections", HashMap::new())
            .set(count);
    }

    /// Set queue size
    pub fn set_queue_size(&self, queue_name: &str, size: i64) {
        let mut labels = HashMap::new();
        labels.insert("queue".to_string(), queue_name.to_string());

        self.registry
            .get_or_create_gauge("queue_size", labels)
            .set(size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_collector() {
        let registry = Arc::new(MetricRegistry::new());
        let collector = PerformanceCollector::new(Arc::clone(&registry));

        collector.collect();

        // Check that metrics were created
        let metrics = registry.collect();
        assert!(!metrics.is_empty());

        // Should have CPU and memory metrics
        let has_cpu = metrics.iter().any(|m| m.name.contains("cpu"));
        let has_memory = metrics.iter().any(|m| m.name.contains("memory"));

        assert!(has_cpu, "Should have CPU metrics");
        assert!(has_memory, "Should have memory metrics");
    }

    #[test]
    fn test_runtime_metrics() {
        let registry = Arc::new(MetricRegistry::new());
        let runtime = RuntimeMetrics::new(Arc::clone(&registry));

        runtime.record_request_duration("api_call", 150);
        runtime.record_success("api_call");
        runtime.record_error("db_query", "timeout");

        let metrics = registry.collect();
        assert!(!metrics.is_empty());

        // Check specific metrics
        let request_duration = metrics
            .iter()
            .find(|m| m.name == "request_duration_ms_total");
        assert!(request_duration.is_some());

        let errors = metrics.iter().find(|m| m.name == "error_count_total");
        assert!(errors.is_some());
    }

    #[test]
    fn test_disable_collector() {
        let registry = Arc::new(MetricRegistry::new());
        let mut collector = PerformanceCollector::new(Arc::clone(&registry));

        collector.set_enabled(false);
        collector.collect();

        // No metrics should be collected when disabled
        // (Initial state might have some, so we check count doesn't increase)
        let count_before = registry.count();

        collector.collect();
        let count_after = registry.count();

        assert_eq!(
            count_before, count_after,
            "Should not collect when disabled"
        );
    }
}
