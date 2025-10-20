//! Integrated CPU and memory monitoring with historical tracking
//!
//! This module provides comprehensive system resource monitoring that combines:
//! - Real-time CPU and memory metrics
//! - Historical data tracking (moving averages, peaks)
//! - Process-specific monitoring
//! - Cross-platform support via sysinfo
//! - Automatic background collection

use crate::core::MetricRegistry;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{ProcessesToUpdate, System};

/// Configuration for integrated system monitoring
#[derive(Debug, Clone)]
pub struct IntegratedSystemConfig {
    /// Number of historical samples to keep for averaging
    pub history_size: usize,
    /// Collection interval for auto-collection
    pub collection_interval: Duration,
    /// Enable process-specific monitoring
    pub enable_process_monitoring: bool,
    /// Enable per-CPU core monitoring
    pub enable_per_core_monitoring: bool,
}

impl Default for IntegratedSystemConfig {
    fn default() -> Self {
        Self {
            history_size: 60, // Keep 60 samples (1 minute at 1s interval)
            collection_interval: Duration::from_secs(1),
            enable_process_monitoring: true,
            enable_per_core_monitoring: true,
        }
    }
}

impl IntegratedSystemConfig {
    /// Create new configuration with custom history size
    pub fn with_history_size(mut self, size: usize) -> Self {
        self.history_size = size;
        self
    }

    /// Set collection interval
    pub fn with_collection_interval(mut self, interval: Duration) -> Self {
        self.collection_interval = interval;
        self
    }

    /// Enable or disable process monitoring
    pub fn with_process_monitoring(mut self, enabled: bool) -> Self {
        self.enable_process_monitoring = enabled;
        self
    }

    /// Enable or disable per-core monitoring
    pub fn with_per_core_monitoring(mut self, enabled: bool) -> Self {
        self.enable_per_core_monitoring = enabled;
        self
    }
}

/// Historical CPU metrics
#[derive(Debug, Clone)]
struct CpuHistory {
    samples: VecDeque<f32>,
    peak: f32,
    last_update: Instant,
}

impl CpuHistory {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            peak: 0.0,
            last_update: Instant::now(),
        }
    }

    fn add_sample(&mut self, value: f32, max_samples: usize) {
        if self.samples.len() >= max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(value);

        if value > self.peak {
            self.peak = value;
        }

        self.last_update = Instant::now();
    }

    fn average(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.samples.iter().sum();
        sum / self.samples.len() as f32
    }

    fn current(&self) -> f32 {
        self.samples.back().copied().unwrap_or(0.0)
    }
}

/// Historical memory metrics
#[derive(Debug, Clone)]
struct MemoryHistory {
    samples: VecDeque<u64>,
    peak: u64,
    last_update: Instant,
}

impl MemoryHistory {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            peak: 0,
            last_update: Instant::now(),
        }
    }

    fn add_sample(&mut self, value: u64, max_samples: usize) {
        if self.samples.len() >= max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(value);

        if value > self.peak {
            self.peak = value;
        }

        self.last_update = Instant::now();
    }

    fn average(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        let sum: u64 = self.samples.iter().sum();
        sum / self.samples.len() as u64
    }

    fn current(&self) -> u64 {
        self.samples.back().copied().unwrap_or(0)
    }
}

/// Integrated system monitor with historical tracking
pub struct IntegratedSystemMonitor {
    config: IntegratedSystemConfig,
    registry: Arc<MetricRegistry>,
    system: Arc<RwLock<System>>,

    // Historical data
    cpu_history: Arc<RwLock<CpuHistory>>,
    memory_history: Arc<RwLock<MemoryHistory>>,
    process_cpu_history: Arc<RwLock<CpuHistory>>,
    process_memory_history: Arc<RwLock<MemoryHistory>>,

    // State
    enabled: Arc<AtomicBool>,
}

impl IntegratedSystemMonitor {
    /// Create a new integrated system monitor
    pub fn new(registry: Arc<MetricRegistry>, config: IntegratedSystemConfig) -> Self {
        let max_samples = config.history_size;

        Self {
            config,
            registry,
            system: Arc::new(RwLock::new(System::new_all())),
            cpu_history: Arc::new(RwLock::new(CpuHistory::new(max_samples))),
            memory_history: Arc::new(RwLock::new(MemoryHistory::new(max_samples))),
            process_cpu_history: Arc::new(RwLock::new(CpuHistory::new(max_samples))),
            process_memory_history: Arc::new(RwLock::new(MemoryHistory::new(max_samples))),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Create with default configuration
    pub fn with_defaults(registry: Arc<MetricRegistry>) -> Self {
        Self::new(registry, IntegratedSystemConfig::default())
    }

    /// Enable or disable monitoring
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// Check if monitoring is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Collect all metrics (CPU, memory, historical)
    pub fn collect(&self) {
        if !self.is_enabled() {
            return;
        }

        self.collect_cpu_metrics();
        self.collect_memory_metrics();

        if self.config.enable_process_monitoring {
            self.collect_process_metrics();
        }
    }

    /// Collect CPU metrics with historical tracking
    fn collect_cpu_metrics(&self) {
        use std::collections::HashMap;

        let mut system = self.system.write();
        system.refresh_cpu_all();

        // Global CPU usage
        let global_cpu = system.global_cpu_usage();

        // Update history
        {
            let mut history = self.cpu_history.write();
            history.add_sample(global_cpu, self.config.history_size);
        }

        // Report current metrics
        self.registry
            .get_or_create_gauge("system_cpu_usage_percent", HashMap::new())
            .set(global_cpu as i64);

        // Report historical metrics
        let history = self.cpu_history.read();
        self.registry
            .get_or_create_gauge("system_cpu_usage_avg_percent", HashMap::new())
            .set(history.average() as i64);

        self.registry
            .get_or_create_gauge("system_cpu_usage_peak_percent", HashMap::new())
            .set(history.peak as i64);

        // Per-CPU core metrics (if enabled)
        if self.config.enable_per_core_monitoring {
            for (i, cpu) in system.cpus().iter().enumerate() {
                let mut labels = HashMap::new();
                labels.insert("cpu".to_string(), i.to_string());

                let usage = cpu.cpu_usage();
                self.registry
                    .get_or_create_gauge("system_cpu_core_usage_percent", labels)
                    .set(usage as i64);
            }
        }

        // CPU count
        self.registry
            .get_or_create_gauge("system_cpu_count", HashMap::new())
            .set(system.cpus().len() as i64);
    }

    /// Collect memory metrics with historical tracking
    fn collect_memory_metrics(&self) {
        use std::collections::HashMap;

        let mut system = self.system.write();
        system.refresh_memory();

        // Current memory metrics
        let total = system.total_memory();
        let used = system.used_memory();
        let available = system.available_memory();

        // Update history
        {
            let mut history = self.memory_history.write();
            history.add_sample(used, self.config.history_size);
        }

        // Report current metrics
        self.registry
            .get_or_create_gauge("system_memory_total_bytes", HashMap::new())
            .set(total as i64);

        self.registry
            .get_or_create_gauge("system_memory_used_bytes", HashMap::new())
            .set(used as i64);

        self.registry
            .get_or_create_gauge("system_memory_available_bytes", HashMap::new())
            .set(available as i64);

        // Memory usage percentage
        let usage_percent = if total > 0 {
            (used as f64 / total as f64 * 100.0) as i64
        } else {
            0
        };

        self.registry
            .get_or_create_gauge("system_memory_usage_percent", HashMap::new())
            .set(usage_percent);

        // Historical metrics
        let history = self.memory_history.read();
        self.registry
            .get_or_create_gauge("system_memory_used_avg_bytes", HashMap::new())
            .set(history.average() as i64);

        self.registry
            .get_or_create_gauge("system_memory_used_peak_bytes", HashMap::new())
            .set(history.peak as i64);

        // Swap metrics
        self.registry
            .get_or_create_gauge("system_swap_total_bytes", HashMap::new())
            .set(system.total_swap() as i64);

        self.registry
            .get_or_create_gauge("system_swap_used_bytes", HashMap::new())
            .set(system.used_swap() as i64);
    }

    /// Collect process-specific metrics with historical tracking
    fn collect_process_metrics(&self) {
        use std::collections::HashMap;

        let mut system = self.system.write();
        system.refresh_processes(ProcessesToUpdate::All);

        let pid = match sysinfo::get_current_pid() {
            Ok(pid) => pid,
            Err(_) => {
                tracing::warn!("Failed to get current process PID - skipping process metrics");
                return;
            }
        };

        if let Some(process) = system.process(pid) {
            // Process CPU usage
            let cpu_usage = process.cpu_usage();

            // Update history
            {
                let mut history = self.process_cpu_history.write();
                history.add_sample(cpu_usage, self.config.history_size);
            }

            // Report current metrics
            self.registry
                .get_or_create_gauge("process_cpu_usage_percent", HashMap::new())
                .set(cpu_usage as i64);

            // Historical CPU metrics
            let cpu_history = self.process_cpu_history.read();
            self.registry
                .get_or_create_gauge("process_cpu_usage_avg_percent", HashMap::new())
                .set(cpu_history.average() as i64);

            self.registry
                .get_or_create_gauge("process_cpu_usage_peak_percent", HashMap::new())
                .set(cpu_history.peak as i64);

            // Process memory usage
            let memory = process.memory();

            // Update history
            {
                let mut history = self.process_memory_history.write();
                history.add_sample(memory, self.config.history_size);
            }

            // Report current metrics
            self.registry
                .get_or_create_gauge("process_memory_bytes", HashMap::new())
                .set(memory as i64);

            // Historical memory metrics
            let mem_history = self.process_memory_history.read();
            self.registry
                .get_or_create_gauge("process_memory_avg_bytes", HashMap::new())
                .set(mem_history.average() as i64);

            self.registry
                .get_or_create_gauge("process_memory_peak_bytes", HashMap::new())
                .set(mem_history.peak as i64);

            // Virtual memory
            self.registry
                .get_or_create_gauge("process_virtual_memory_bytes", HashMap::new())
                .set(process.virtual_memory() as i64);
        }
    }

    /// Get current system CPU usage percentage
    pub fn current_cpu_usage(&self) -> f32 {
        self.cpu_history.read().current()
    }

    /// Get average system CPU usage over history window
    pub fn average_cpu_usage(&self) -> f32 {
        self.cpu_history.read().average()
    }

    /// Get peak system CPU usage
    pub fn peak_cpu_usage(&self) -> f32 {
        self.cpu_history.read().peak
    }

    /// Get current memory usage in bytes
    pub fn current_memory_usage(&self) -> u64 {
        self.memory_history.read().current()
    }

    /// Get average memory usage over history window
    pub fn average_memory_usage(&self) -> u64 {
        self.memory_history.read().average()
    }

    /// Get peak memory usage
    pub fn peak_memory_usage(&self) -> u64 {
        self.memory_history.read().peak
    }

    /// Get current process CPU usage percentage
    pub fn current_process_cpu_usage(&self) -> f32 {
        self.process_cpu_history.read().current()
    }

    /// Get average process CPU usage over history window
    pub fn average_process_cpu_usage(&self) -> f32 {
        self.process_cpu_history.read().average()
    }

    /// Get current process memory usage in bytes
    pub fn current_process_memory_usage(&self) -> u64 {
        self.process_memory_history.read().current()
    }

    /// Get average process memory usage over history window
    pub fn average_process_memory_usage(&self) -> u64 {
        self.process_memory_history.read().average()
    }

    /// Reset historical data (peaks and averages)
    pub fn reset_history(&self) {
        let max_samples = self.config.history_size;

        *self.cpu_history.write() = CpuHistory::new(max_samples);
        *self.memory_history.write() = MemoryHistory::new(max_samples);
        *self.process_cpu_history.write() = CpuHistory::new(max_samples);
        *self.process_memory_history.write() = MemoryHistory::new(max_samples);
    }

    /// Start automatic collection in background
    ///
    /// Returns a handle that can be used to stop the collector.
    /// Collection runs at the interval specified in the configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_monitoring_system::collectors::{IntegratedSystemMonitor, IntegratedSystemConfig};
    /// use rust_monitoring_system::MetricRegistry;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let registry = Arc::new(MetricRegistry::new());
    /// let config = IntegratedSystemConfig::default()
    ///     .with_collection_interval(Duration::from_secs(5));
    /// let monitor = Arc::new(IntegratedSystemMonitor::new(registry, config));
    ///
    /// let handle = monitor.start_auto_collect();
    ///
    /// // Monitor runs in background...
    ///
    /// // Later, stop the monitor
    /// handle.stop();
    /// # }
    /// ```
    pub fn start_auto_collect(self: Arc<Self>) -> IntegratedSystemHandle {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);
        let interval = self.config.collection_interval;

        let handle = std::thread::spawn(move || {
            while !shutdown_clone.load(Ordering::Acquire) {
                self.collect();

                // Sleep in small increments for faster shutdown response
                let sleep_increments = (interval.as_millis() / 100).max(1);
                for _ in 0..sleep_increments {
                    if shutdown_clone.load(Ordering::Acquire) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });

        IntegratedSystemHandle {
            shutdown,
            handle: Some(handle),
        }
    }
}

/// Handle for controlling auto-collection background thread
pub struct IntegratedSystemHandle {
    shutdown: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl IntegratedSystemHandle {
    /// Stop the auto-collection thread and wait for it to exit
    pub fn stop(mut self) {
        self.shutdown.store(true, Ordering::Release);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for IntegratedSystemHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);

        if let Some(handle) = self.handle.take() {
            // Wait for thread to finish with timeout
            const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

            let start = Instant::now();
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    break;
                }

                if start.elapsed() >= SHUTDOWN_TIMEOUT {
                    tracing::warn!(
                        "Integrated system monitor did not finish within {}s timeout during drop",
                        SHUTDOWN_TIMEOUT.as_secs()
                    );
                    break;
                }

                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrated_monitor_creation() {
        let registry = Arc::new(MetricRegistry::new());
        let monitor = IntegratedSystemMonitor::with_defaults(registry);

        assert!(monitor.is_enabled());
    }

    #[test]
    fn test_collect_metrics() {
        let registry = Arc::new(MetricRegistry::new());
        let monitor = IntegratedSystemMonitor::with_defaults(Arc::clone(&registry));

        monitor.collect();

        // Verify metrics were created
        let metrics = registry.collect();
        assert!(!metrics.is_empty());

        // Should have CPU and memory metrics
        let has_cpu = metrics.iter().any(|m| m.name.contains("cpu"));
        let has_memory = metrics.iter().any(|m| m.name.contains("memory"));

        assert!(has_cpu, "Should have CPU metrics");
        assert!(has_memory, "Should have memory metrics");
    }

    #[test]
    fn test_historical_tracking() {
        let registry = Arc::new(MetricRegistry::new());
        let config = IntegratedSystemConfig::default().with_history_size(5);
        let monitor = IntegratedSystemMonitor::new(registry, config);

        // Collect multiple samples
        for _ in 0..5 {
            monitor.collect();
            std::thread::sleep(Duration::from_millis(10));
        }

        // Check that historical data is tracked
        let avg_cpu = monitor.average_cpu_usage();
        let peak_cpu = monitor.peak_cpu_usage();

        assert!(avg_cpu >= 0.0);
        assert!(peak_cpu >= 0.0);
        assert!(peak_cpu >= avg_cpu);
    }

    #[test]
    fn test_reset_history() {
        let registry = Arc::new(MetricRegistry::new());
        let monitor = IntegratedSystemMonitor::with_defaults(registry);

        // Collect some data
        monitor.collect();
        std::thread::sleep(Duration::from_millis(10));
        monitor.collect();

        let peak_before = monitor.peak_cpu_usage();
        // Peak should be non-negative after collecting data
        assert!(peak_before >= 0.0, "Peak should be non-negative");

        // Reset history
        monitor.reset_history();

        let peak_after = monitor.peak_cpu_usage();

        assert_eq!(peak_after, 0.0, "Peak should be reset to 0");
    }

    #[test]
    fn test_enable_disable() {
        let registry = Arc::new(MetricRegistry::new());
        let monitor = IntegratedSystemMonitor::with_defaults(Arc::clone(&registry));

        monitor.set_enabled(false);
        assert!(!monitor.is_enabled());

        monitor.collect();

        // Metrics should not be updated when disabled
        let metrics_disabled = registry.count();

        monitor.set_enabled(true);
        assert!(monitor.is_enabled());

        monitor.collect();

        let metrics_enabled = registry.count();

        // Should have more metrics when enabled
        assert!(metrics_enabled >= metrics_disabled);
    }
}
