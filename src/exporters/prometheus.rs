//! Prometheus format exporter

use crate::core::error::Result;
use crate::core::metric::{Metric, MetricType, MetricValue};

/// Prometheus exporter
pub struct PrometheusExporter;

impl PrometheusExporter {
    /// Create a new Prometheus exporter
    pub fn new() -> Self {
        Self
    }

    /// Export metrics in Prometheus text format
    pub fn export(&self, metrics: &[Metric]) -> Result<String> {
        let mut output = String::new();

        for metric in metrics {
            let safe_name = Self::escape_metric_name(&metric.name);

            // Add HELP line (help text should also be escaped)
            if !metric.help.is_empty() {
                let safe_help = metric.help.replace('\\', "\\\\").replace('\n', "\\n");
                output.push_str(&format!("# HELP {} {}\n", safe_name, safe_help));
            }

            // Add TYPE line
            let metric_type = match metric.metric_type {
                MetricType::Counter => "counter",
                MetricType::Gauge => "gauge",
                MetricType::Histogram => "histogram",
                MetricType::Summary => "summary",
                MetricType::Timer => "summary",
            };
            output.push_str(&format!("# TYPE {} {}\n", safe_name, metric_type));

            // Add metric value(s)
            match &metric.value {
                MetricValue::Int(v) => {
                    output.push_str(&Self::format_metric_line(
                        &metric.name,
                        &metric.labels,
                        *v as f64,
                    ));
                }
                MetricValue::Uint(v) => {
                    output.push_str(&Self::format_metric_line(
                        &metric.name,
                        &metric.labels,
                        *v as f64,
                    ));
                }
                MetricValue::Float(v) => {
                    output.push_str(&Self::format_metric_line(&metric.name, &metric.labels, *v));
                }
                MetricValue::Histogram(hist) => {
                    // Export histogram buckets
                    for (i, &boundary) in hist.buckets.iter().enumerate() {
                        let mut bucket_labels = metric.labels.clone();
                        bucket_labels.insert("le".to_string(), boundary.to_string());

                        let cumulative: u64 = hist.counts[..=i].iter().sum();
                        output.push_str(&Self::format_metric_line(
                            &format!("{}_bucket", metric.name),
                            &bucket_labels,
                            cumulative as f64,
                        ));
                    }

                    // +Inf bucket
                    let mut inf_labels = metric.labels.clone();
                    inf_labels.insert("le".to_string(), "+Inf".to_string());
                    output.push_str(&Self::format_metric_line(
                        &format!("{}_bucket", metric.name),
                        &inf_labels,
                        hist.count as f64,
                    ));

                    // Sum
                    output.push_str(&Self::format_metric_line(
                        &format!("{}_sum", metric.name),
                        &metric.labels,
                        hist.sum,
                    ));

                    // Count
                    output.push_str(&Self::format_metric_line(
                        &format!("{}_count", metric.name),
                        &metric.labels,
                        hist.count as f64,
                    ));
                }
                MetricValue::Summary(summary) => {
                    // Sum
                    output.push_str(&Self::format_metric_line(
                        &format!("{}_sum", metric.name),
                        &metric.labels,
                        summary.sum,
                    ));

                    // Count
                    output.push_str(&Self::format_metric_line(
                        &format!("{}_count", metric.name),
                        &metric.labels,
                        summary.count as f64,
                    ));
                }
            }

            output.push('\n');
        }

        Ok(output)
    }

    /// Escape label value to prevent Prometheus format injection
    ///
    /// Prometheus label values must escape backslashes, double quotes, and newlines
    /// to prevent metric injection attacks.
    fn escape_label_value(value: &str) -> String {
        value
            .replace('\\', "\\\\") // Backslash must be escaped first
            .replace('"', "\\\"") // Escape double quotes
            .replace('\n', "\\n") // Escape newlines
    }

    /// Escape label name to prevent invalid Prometheus format
    ///
    /// Label names should only contain [a-zA-Z0-9_]
    fn escape_label_name(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Escape metric name to prevent invalid Prometheus format
    ///
    /// Metric names should only contain [a-zA-Z0-9_:]
    fn escape_metric_name(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == ':' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Format a single metric line
    fn format_metric_line(
        name: &str,
        labels: &std::collections::HashMap<String, String>,
        value: f64,
    ) -> String {
        let safe_name = Self::escape_metric_name(name);

        if labels.is_empty() {
            format!("{} {}\n", safe_name, value)
        } else {
            let label_str = labels
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}=\"{}\"",
                        Self::escape_label_name(k),
                        Self::escape_label_value(v)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{}{{{}}} {}\n", safe_name, label_str, value)
        }
    }
}

impl Default for PrometheusExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    #[test]
    fn test_export_counter() {
        let exporter = PrometheusExporter::new();

        let metric = Metric {
            name: "test_counter".to_string(),
            metric_type: MetricType::Counter,
            help: "A test counter".to_string(),
            labels: HashMap::new(),
            value: MetricValue::Uint(42),
            timestamp: 0,
        };

        let output = exporter
            .export(&[metric])
            .expect("Failed to export counter metric");

        assert!(output.contains("# HELP test_counter A test counter"));
        assert!(output.contains("# TYPE test_counter counter"));
        assert!(output.contains("test_counter 42"));
    }

    #[test]
    fn test_export_with_labels() {
        let exporter = PrometheusExporter::new();

        let mut labels = HashMap::new();
        labels.insert("method".to_string(), "GET".to_string());
        labels.insert("status".to_string(), "200".to_string());

        let metric = Metric {
            name: "http_requests".to_string(),
            metric_type: MetricType::Counter,
            help: "HTTP requests".to_string(),
            labels,
            value: MetricValue::Uint(100),
            timestamp: 0,
        };

        let output = exporter
            .export(&[metric])
            .expect("Failed to export metric with labels");

        assert!(output.contains("http_requests{"));
        assert!(output.contains("method=\"GET\""));
        assert!(output.contains("status=\"200\""));
    }

    #[test]
    fn test_export_histogram() {
        let exporter = PrometheusExporter::new();

        let mut hist = crate::core::metric::HistogramData::new(vec![0.1, 1.0, 10.0]);
        hist.observe(0.05);
        hist.observe(0.5);
        hist.observe(5.0);
        hist.observe(50.0);

        let metric = Metric {
            name: "test_histogram".to_string(),
            metric_type: MetricType::Histogram,
            help: "A test histogram".to_string(),
            labels: HashMap::new(),
            value: MetricValue::Histogram(hist),
            timestamp: 0,
        };

        let output = exporter
            .export(&[metric])
            .expect("Failed to export histogram metric");

        assert!(output.contains("test_histogram_bucket{le=\"0.1\"}"));
        assert!(output.contains("test_histogram_bucket{le=\"+Inf\"}"));
        assert!(output.contains("test_histogram_sum"));
        assert!(output.contains("test_histogram_count"));
    }

    #[test]
    fn test_label_escaping() {
        let exporter = PrometheusExporter::new();

        let mut labels = HashMap::new();
        // Test injection attack attempt
        labels.insert(
            "user".to_string(),
            "test\"} malicious_metric 1\n#".to_string(),
        );
        labels.insert("path".to_string(), "/api\\test".to_string());

        let metric = Metric {
            name: "test_metric".to_string(),
            metric_type: MetricType::Counter,
            help: "Test metric".to_string(),
            labels,
            value: MetricValue::Uint(42),
            timestamp: 0,
        };

        let output = exporter.export(&[metric]).expect("Failed to export metric");

        // Verify escaping
        assert!(output.contains("user=\"test\\\"} malicious_metric 1\\n#\""));
        assert!(output.contains("path=\"/api\\\\test\""));
        // Ensure no actual injection occurred
        assert!(!output.contains("malicious_metric 1\n"));
    }

    #[test]
    fn test_metric_name_sanitization() {
        let exporter = PrometheusExporter::new();

        let metric = Metric {
            name: "test-metric.with!invalid@chars".to_string(),
            metric_type: MetricType::Counter,
            help: "Test with newline\nand backslash\\".to_string(),
            labels: HashMap::new(),
            value: MetricValue::Uint(1),
            timestamp: 0,
        };

        let output = exporter.export(&[metric]).expect("Failed to export metric");

        // Metric name should be sanitized
        assert!(output.contains("test_metric_with_invalid_chars"));
        // Help should be escaped
        assert!(output.contains("Test with newline\\nand backslash\\\\"));
    }

    #[test]
    fn test_label_name_sanitization() {
        let exporter = PrometheusExporter::new();

        let mut labels = HashMap::new();
        labels.insert("invalid-label.name!".to_string(), "value".to_string());

        let metric = Metric {
            name: "test".to_string(),
            metric_type: MetricType::Counter,
            help: "".to_string(),
            labels,
            value: MetricValue::Uint(1),
            timestamp: 0,
        };

        let output = exporter.export(&[metric]).expect("Failed to export metric");

        // Label name should be sanitized
        assert!(output.contains("invalid_label_name_=\"value\""));
    }
}
