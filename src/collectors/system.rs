//! System metrics collector

use crate::core::error::Result;
use crate::core::metric::{Gauge, Labels};
use crate::core::Monitor;
#[cfg(target_os = "linux")]
use parking_lot::Mutex;
use std::sync::Arc;

/// CPU stat for delta calculation (Linux only)
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, Default)]
struct CpuStat {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
}

#[cfg(target_os = "linux")]
impl CpuStat {
    fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle
    }

    fn active(&self) -> u64 {
        self.user + self.nice + self.system
    }
}

/// System metrics collector
pub struct SystemCollector {
    monitor: Arc<Monitor>,
    cpu_usage: Gauge,
    memory_usage: Gauge,
    memory_total: Gauge,
    uptime: Gauge,
    /// Previous CPU statistics for delta calculation
    #[cfg(target_os = "linux")]
    prev_cpu_stat: Arc<Mutex<Option<CpuStat>>>,
}

impl SystemCollector {
    /// Create a new system collector
    pub fn new(monitor: Arc<Monitor>) -> Result<Self> {
        let cpu_usage = monitor.gauge("system_cpu_usage_percent", Labels::new());

        let memory_usage = monitor.gauge("system_memory_usage_bytes", Labels::new());

        let memory_total = monitor.gauge("system_memory_total_bytes", Labels::new());

        let uptime = monitor.gauge("system_uptime_seconds", Labels::new());

        Ok(Self {
            monitor,
            cpu_usage,
            memory_usage,
            memory_total,
            uptime,
            #[cfg(target_os = "linux")]
            prev_cpu_stat: Arc::new(Mutex::new(None)),
        })
    }

    /// Collect system metrics
    pub fn collect(&self) -> Result<()> {
        // Update CPU usage
        #[cfg(target_os = "linux")]
        {
            if let Ok(cpu) = self.get_cpu_usage_linux() {
                self.cpu_usage.set((cpu * 100.0) as i64);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(cpu) = Self::get_cpu_usage_macos() {
                self.cpu_usage.set((cpu * 100.0) as i64);
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Placeholder for Windows implementation
            self.cpu_usage.set(0);
        }

        // Update memory usage
        #[cfg(target_os = "linux")]
        {
            if let Ok((used, total)) = Self::get_memory_linux() {
                self.memory_usage.set(used as i64);
                self.memory_total.set(total as i64);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok((used, total)) = Self::get_memory_macos() {
                self.memory_usage.set(used as i64);
                self.memory_total.set(total as i64);
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Placeholder for Windows implementation
            self.memory_usage.set(0);
            self.memory_total.set(0);
        }

        // Update uptime
        self.uptime.set(self.monitor.uptime() as i64);

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn get_cpu_usage_linux(&self) -> Result<f64> {
        use std::fs;

        // Read current CPU statistics from /proc/stat
        let contents = fs::read_to_string("/proc/stat")
            .map_err(|e| crate::core::error::MonitoringError::collection(e.to_string()))?;

        let current_stat = if let Some(line) = contents.lines().next() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 4 && parts[0] == "cpu" {
                CpuStat {
                    user: parts[1].parse().unwrap_or(0),
                    nice: parts[2].parse().unwrap_or(0),
                    system: parts[3].parse().unwrap_or(0),
                    idle: parts[4].parse().unwrap_or(0),
                }
            } else {
                return Ok(0.0);
            }
        } else {
            return Ok(0.0);
        };

        // Get previous stat and calculate delta
        let mut prev = self.prev_cpu_stat.lock();

        let usage = if let Some(prev_stat) = *prev {
            // Calculate delta between current and previous measurements
            let total_delta = current_stat.total().saturating_sub(prev_stat.total());
            let active_delta = current_stat.active().saturating_sub(prev_stat.active());

            if total_delta > 0 {
                active_delta as f64 / total_delta as f64
            } else {
                // No time has passed, return 0
                0.0
            }
        } else {
            // First call, no previous data - return 0
            // Next call will have meaningful delta
            0.0
        };

        // Store current stat for next calculation
        *prev = Some(current_stat);

        Ok(usage)
    }

    #[cfg(target_os = "macos")]
    fn get_cpu_usage_macos() -> Result<f64> {
        // PLATFORM LIMITATION: macOS system metrics not yet implemented
        //
        // This is a known limitation. The function returns 0.0 as a placeholder.
        // Production systems on macOS should:
        // 1. Use an external monitoring solution (Prometheus node_exporter, etc.)
        // 2. Implement using macOS-specific APIs:
        //    - host_statistics() for CPU info
        //    - sysctl() with CTL_HW for hardware info
        // 3. Consider the `sysinfo` crate for cross-platform metrics
        //
        // For now, this allows the code to compile and run on macOS,
        // but system CPU metrics will always be 0.
        Ok(0.0)
    }

    #[cfg(target_os = "linux")]
    fn get_memory_linux() -> Result<(u64, u64)> {
        use std::fs;

        let contents = fs::read_to_string("/proc/meminfo")
            .map_err(|e| crate::core::error::MonitoringError::collection(e.to_string()))?;

        let mut total = 0u64;
        let mut available = 0u64;

        for line in contents.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(value) = line.split_whitespace().nth(1) {
                    total = value.parse::<u64>().unwrap_or(0) * 1024; // Convert KB to bytes
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(value) = line.split_whitespace().nth(1) {
                    available = value.parse::<u64>().unwrap_or(0) * 1024;
                }
            }
        }

        let used = total.saturating_sub(available);
        Ok((used, total))
    }

    #[cfg(target_os = "macos")]
    fn get_memory_macos() -> Result<(u64, u64)> {
        // PLATFORM LIMITATION: macOS memory metrics not yet implemented
        //
        // This is a known limitation. The function returns (0, 0) as a placeholder.
        // Production systems on macOS should:
        // 1. Use an external monitoring solution (Prometheus node_exporter, etc.)
        // 2. Implement using macOS-specific APIs:
        //    - vm_statistics64() for memory statistics
        //    - sysctl() with CTL_HW.HW_MEMSIZE for total memory
        // 3. Consider the `sysinfo` crate for cross-platform metrics
        //
        // For now, this allows the code to compile and run on macOS,
        // but system memory metrics will always be 0.
        Ok((0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_collector() {
        let monitor = Arc::new(Monitor::new());
        let collector =
            SystemCollector::new(monitor.clone()).expect("Failed to create system collector");

        let result = collector.collect();
        assert!(result.is_ok());

        // Check that metrics were updated
        let metrics = monitor.collect();
        assert!(metrics.len() >= 4);
    }
}
