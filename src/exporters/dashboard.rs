//! Real-time metrics dashboard exporter

use crate::core::MetricRegistry;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Dashboard data point
#[derive(Debug, Clone, serde::Serialize)]
pub struct DataPoint {
    /// Unix timestamp in seconds
    pub timestamp: u64,
    /// Metric value
    pub value: i64,
}

/// Time series data
#[derive(Debug, Clone, serde::Serialize)]
pub struct TimeSeries {
    /// Metric name
    pub name: String,
    /// Metric labels (key-value pairs)
    pub labels: std::collections::HashMap<String, String>,
    /// Time-stamped data points
    pub data_points: Vec<DataPoint>,
}

/// Dashboard panel configuration
#[derive(Debug, Clone, serde::Serialize)]
pub struct Panel {
    /// Unique panel identifier
    pub id: String,
    /// Panel display title
    pub title: String,
    /// Name of the metric to display
    pub metric_name: String,
    /// Type of visualization panel
    pub panel_type: PanelType,
}

/// Panel types
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelType {
    /// Time-series graph visualization
    Graph,
    /// Gauge (percentage) visualization
    Gauge,
    /// Counter (numeric) visualization
    Counter,
    /// Table visualization
    Table,
}

/// Dashboard configuration
#[derive(Debug, Clone, serde::Serialize)]
pub struct Dashboard {
    /// Dashboard title
    pub title: String,
    /// List of dashboard panels
    pub panels: Vec<Panel>,
    /// Auto-refresh interval in seconds
    pub refresh_interval_seconds: u64,
}

impl Dashboard {
    /// Create a new dashboard
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            panels: Vec::new(),
            refresh_interval_seconds: 5,
        }
    }

    /// Add a panel
    pub fn add_panel(&mut self, panel: Panel) {
        self.panels.push(panel);
    }

    /// Set refresh interval
    pub fn with_refresh_interval(mut self, seconds: u64) -> Self {
        self.refresh_interval_seconds = seconds;
        self
    }
}

/// Dashboard exporter
pub struct DashboardExporter {
    registry: Arc<MetricRegistry>,
    dashboard: Dashboard,
}

impl DashboardExporter {
    /// Create a new dashboard exporter
    pub fn new(registry: Arc<MetricRegistry>, dashboard: Dashboard) -> Self {
        Self {
            registry,
            dashboard,
        }
    }

    /// Create a default system dashboard
    pub fn default_system_dashboard(registry: Arc<MetricRegistry>) -> Self {
        let mut dashboard = Dashboard::new("System Metrics");

        dashboard.add_panel(Panel {
            id: "cpu_usage".to_string(),
            title: "CPU Usage".to_string(),
            metric_name: "system_cpu_usage_percent".to_string(),
            panel_type: PanelType::Gauge,
        });

        dashboard.add_panel(Panel {
            id: "memory_usage".to_string(),
            title: "Memory Usage".to_string(),
            metric_name: "system_memory_usage_percent".to_string(),
            panel_type: PanelType::Gauge,
        });

        dashboard.add_panel(Panel {
            id: "request_rate".to_string(),
            title: "Request Rate".to_string(),
            metric_name: "request_count_total".to_string(),
            panel_type: PanelType::Graph,
        });

        dashboard.add_panel(Panel {
            id: "error_rate".to_string(),
            title: "Error Rate".to_string(),
            metric_name: "error_count_total".to_string(),
            panel_type: PanelType::Graph,
        });

        Self::new(registry, dashboard)
    }

    /// Export dashboard data as JSON
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        let snapshot = self.get_snapshot();
        serde_json::to_string_pretty(&snapshot)
    }

    /// Get current dashboard snapshot
    pub fn get_snapshot(&self) -> DashboardSnapshot {
        let metrics = self.registry.collect();
        // Get timestamp safely - use 0 if system time is before Unix epoch (should never happen)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|e| {
                tracing::warn!("System time before Unix epoch: {} - using 0", e);
                0
            });

        let mut time_series_data = Vec::new();

        for metric in metrics {
            // Convert MetricValue to i64 for dashboard display
            let value = match metric.value {
                crate::core::metric::MetricValue::Int(v) => v,
                crate::core::metric::MetricValue::Uint(v) => v as i64,
                crate::core::metric::MetricValue::Float(v) => v as i64,
                _ => 0, // Histogram/Summary not supported in simple dashboard
            };

            let data_point = DataPoint { timestamp, value };

            time_series_data.push(TimeSeries {
                name: metric.name.clone(),
                labels: metric.labels.clone(),
                data_points: vec![data_point],
            });
        }

        DashboardSnapshot {
            dashboard: self.dashboard.clone(),
            time_series: time_series_data,
            timestamp,
        }
    }

    /// Export as HTML dashboard
    pub fn export_html(&self) -> String {
        let snapshot = self.get_snapshot();
        let json_data = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background-color: #1a1a1a;
            color: #ffffff;
        }}
        .dashboard {{
            max-width: 1200px;
            margin: 0 auto;
        }}
        h1 {{
            margin-bottom: 30px;
            color: #4fc3f7;
        }}
        .panels {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
        }}
        .panel {{
            background-color: #2d2d2d;
            border-radius: 8px;
            padding: 20px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.3);
        }}
        .panel h2 {{
            margin-top: 0;
            color: #81c784;
            font-size: 16px;
            font-weight: 500;
        }}
        .metric-value {{
            font-size: 36px;
            font-weight: bold;
            color: #4fc3f7;
            margin: 20px 0;
        }}
        .metric-label {{
            font-size: 12px;
            color: #9e9e9e;
            margin-top: 10px;
        }}
        .gauge {{
            width: 100%;
            height: 10px;
            background-color: #424242;
            border-radius: 5px;
            overflow: hidden;
            margin-top: 10px;
        }}
        .gauge-fill {{
            height: 100%;
            background: linear-gradient(90deg, #4fc3f7, #81c784);
            transition: width 0.3s ease;
        }}
        .timestamp {{
            text-align: center;
            margin-top: 30px;
            color: #9e9e9e;
            font-size: 12px;
        }}
    </style>
    <script>
        let dashboardData = {};

        function updateDashboard() {{
            const panels = document.querySelectorAll('.panel');
            panels.forEach(panel => {{
                const metricName = panel.dataset.metric;
                const series = dashboardData.time_series?.find(s => s.name === metricName);

                if (series && series.data_points.length > 0) {{
                    const value = series.data_points[0].value;
                    const valueEl = panel.querySelector('.metric-value');
                    if (valueEl) {{
                        valueEl.textContent = value;
                    }}

                    const gaugeFill = panel.querySelector('.gauge-fill');
                    if (gaugeFill) {{
                        const percent = Math.min(100, Math.max(0, value));
                        gaugeFill.style.width = percent + '%';
                    }}
                }}
            }});

            const timestampEl = document.querySelector('.timestamp');
            if (timestampEl && dashboardData.timestamp) {{
                const date = new Date(dashboardData.timestamp * 1000);
                timestampEl.textContent = 'Last updated: ' + date.toLocaleString();
            }}
        }}

        function loadData() {{
            // In a real implementation, this would fetch from an API endpoint
            dashboardData = JSON.parse('{}');
            updateDashboard();
        }}

        window.addEventListener('load', () => {{
            loadData();
            setInterval(loadData, {} * 1000);
        }});
    </script>
</head>
<body>
    <div class="dashboard">
        <h1>{}</h1>
        <div class="panels">
"#,
            self.dashboard.title,
            json_data.replace('\'', "\\'"),
            json_data.replace('\'', "\\'"),
            self.dashboard.refresh_interval_seconds,
            self.dashboard.title
        ) + &self
            .dashboard
            .panels
            .iter()
            .map(|panel| self.render_panel(panel))
            .collect::<Vec<_>>()
            .join("\n")
            + r#"
        </div>
        <div class="timestamp">Loading...</div>
    </div>
</body>
</html>"#
    }

    fn render_panel(&self, panel: &Panel) -> String {
        match panel.panel_type {
            PanelType::Gauge => format!(
                r#"            <div class="panel" data-metric="{}">
                <h2>{}</h2>
                <div class="metric-value">--</div>
                <div class="gauge">
                    <div class="gauge-fill" style="width: 0%"></div>
                </div>
                <div class="metric-label">{}</div>
            </div>"#,
                panel.metric_name, panel.title, panel.metric_name
            ),
            PanelType::Counter => format!(
                r#"            <div class="panel" data-metric="{}">
                <h2>{}</h2>
                <div class="metric-value">--</div>
                <div class="metric-label">{}</div>
            </div>"#,
                panel.metric_name, panel.title, panel.metric_name
            ),
            _ => format!(
                r#"            <div class="panel" data-metric="{}">
                <h2>{}</h2>
                <div class="metric-value">--</div>
                <div class="metric-label">{}</div>
            </div>"#,
                panel.metric_name, panel.title, panel.metric_name
            ),
        }
    }
}

/// Dashboard snapshot
#[derive(Debug, Clone, serde::Serialize)]
pub struct DashboardSnapshot {
    /// Dashboard configuration
    pub dashboard: Dashboard,
    /// Current time series data
    pub time_series: Vec<TimeSeries>,
    /// Snapshot timestamp (Unix seconds)
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_creation() {
        let mut dashboard = Dashboard::new("Test Dashboard");
        dashboard.add_panel(Panel {
            id: "test".to_string(),
            title: "Test Panel".to_string(),
            metric_name: "test_metric".to_string(),
            panel_type: PanelType::Gauge,
        });

        assert_eq!(dashboard.title, "Test Dashboard");
        assert_eq!(dashboard.panels.len(), 1);
    }

    #[test]
    fn test_default_dashboard() {
        let registry = Arc::new(MetricRegistry::new());
        let exporter = DashboardExporter::default_system_dashboard(registry);

        assert_eq!(exporter.dashboard.title, "System Metrics");
        assert!(!exporter.dashboard.panels.is_empty());
    }

    #[test]
    fn test_export_json() {
        let registry = Arc::new(MetricRegistry::new());

        // Add some test metrics
        registry
            .get_or_create_gauge("test_metric", std::collections::HashMap::new())
            .set(42);

        let exporter = DashboardExporter::default_system_dashboard(registry);
        let json = exporter.export_json().expect("Failed to export JSON");

        assert!(json.contains("System Metrics"));
    }

    #[test]
    fn test_export_html() {
        let registry = Arc::new(MetricRegistry::new());
        let exporter = DashboardExporter::default_system_dashboard(registry);

        let html = exporter.export_html();

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("System Metrics"));
    }

    #[test]
    fn test_snapshot() {
        let registry = Arc::new(MetricRegistry::new());

        registry
            .get_or_create_gauge("cpu_usage", std::collections::HashMap::new())
            .set(50);

        let exporter = DashboardExporter::default_system_dashboard(registry);
        let snapshot = exporter.get_snapshot();

        assert!(!snapshot.time_series.is_empty());
        assert!(snapshot.timestamp > 0);
    }
}
