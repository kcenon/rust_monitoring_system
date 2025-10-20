//! # Rust Monitoring System
//!
//! A production-ready, high-performance Rust monitoring framework for system observability.
//!
//! ## Features
//!
//! - **Real-Time Metrics**: Collect and track system and application metrics
//! - **Multiple Metric Types**: Counter, Gauge, Histogram, Summary, Timer
//! - **Thread-Safe**: All operations are thread-safe using atomic operations
//! - **Flexible Labels**: Support for multi-dimensional metrics with labels
//! - **Prometheus Export**: Built-in Prometheus text format exporter
//! - **System Collectors**: Pre-built collectors for CPU, memory, and uptime
//! - **Low Overhead**: Optimized for minimal performance impact
//!
//! ## Quick Start
//!
//! ```rust
//! use rust_monitoring_system::prelude::*;
//! use std::collections::HashMap;
//!
//! # fn main() -> Result<()> {
//! // Create a monitor
//! let monitor = Monitor::new();
//! monitor.start()?;
//!
//! // Register and use a counter
//! let counter = monitor.counter(
//!     "requests_total",
//!     HashMap::new()
//! );
//! counter.inc();
//!
//! // Register and use a gauge
//! let gauge = monitor.gauge(
//!     "active_connections",
//!     HashMap::new()
//! );
//! gauge.set(42);
//!
//! // Collect and export metrics
//! let metrics = monitor.collect();
//! let exporter = PrometheusExporter::new();
//! let output = exporter.export(&metrics)?;
//!
//! println!("{}", output);
//! # Ok(())
//! # }
//! ```
//!
//! ## Integrated System Monitoring (Recommended)
//!
//! The `IntegratedSystemMonitor` provides comprehensive CPU and memory monitoring
//! with historical tracking, peak detection, and automatic collection.
//!
//! ```rust
//! use rust_monitoring_system::{MetricRegistry, IntegratedSystemMonitor, IntegratedSystemConfig};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! # fn main() {
//! let registry = Arc::new(MetricRegistry::new());
//!
//! // Configure monitoring with historical tracking
//! let config = IntegratedSystemConfig::default()
//!     .with_history_size(60)  // Keep 60 samples
//!     .with_collection_interval(Duration::from_secs(1))
//!     .with_process_monitoring(true)
//!     .with_per_core_monitoring(true);
//!
//! let monitor = Arc::new(IntegratedSystemMonitor::new(registry.clone(), config));
//!
//! // Start automatic background collection
//! let handle = monitor.clone().start_auto_collect();
//!
//! // Access real-time and historical metrics
//! std::thread::sleep(Duration::from_secs(2));
//! println!("Current CPU: {:.2}%", monitor.current_cpu_usage());
//! println!("Average CPU: {:.2}%", monitor.average_cpu_usage());
//! println!("Peak CPU: {:.2}%", monitor.peak_cpu_usage());
//!
//! // Clean shutdown
//! handle.stop();
//! # }
//! ```
//!
//! ## System Monitoring (Legacy)
//!
//! ```rust
//! use rust_monitoring_system::prelude::*;
//! use std::sync::Arc;
//!
//! # fn main() -> Result<()> {
//! let monitor = Arc::new(Monitor::new());
//! monitor.start()?;
//!
//! // Create system collector
//! let collector = SystemCollector::new(monitor.clone())?;
//!
//! // Collect system metrics
//! collector.collect()?;
//!
//! // View collected metrics
//! let metrics = monitor.collect();
//! for metric in metrics {
//!     println!("{}: {:?}", metric.name, metric.value);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Metrics with Labels
//!
//! ```rust
//! use rust_monitoring_system::prelude::*;
//! use std::collections::HashMap;
//!
//! # fn main() -> Result<()> {
//! let monitor = Monitor::new();
//!
//! // Create counter with labels
//! let mut labels = HashMap::new();
//! labels.insert("method".to_string(), "GET".to_string());
//! labels.insert("endpoint".to_string(), "/api/users".to_string());
//!
//! let counter = monitor.counter(
//!     "http_requests_total",
//!     labels
//! );
//!
//! counter.inc();
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod collectors;
pub mod core;
pub mod exporters;
pub mod prelude;
pub mod scaling;

pub use collectors::{
    IntegratedSystemConfig, IntegratedSystemHandle, IntegratedSystemMonitor, PerformanceCollector,
    RuntimeMetrics, SystemCollector,
};
pub use core::{
    Counter, Gauge, HistogramData, Labels, Metric, MetricRegistry, MetricType, MetricValue,
    Monitor, MonitoringError, Result, SummaryData,
};
pub use exporters::{Dashboard, DashboardExporter, PrometheusExporter};
pub use scaling::{AutoScaler, AutoScalerConfig, PredictiveScaler, ScalingDecision, ScalingRule};
