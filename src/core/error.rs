//! Error types for the monitoring system

/// Result type for monitoring operations
pub type Result<T> = std::result::Result<T, MonitoringError>;

/// Errors that can occur in the monitoring system
#[derive(Debug, thiserror::Error)]
pub enum MonitoringError {
    /// Configuration error with component details
    #[error("Configuration error in {component}: {message}")]
    ConfigError {
        /// Component that has configuration error
        component: String,
        /// Error message
        message: String,
    },

    /// Metric registration error with metric name
    #[error("Failed to register metric '{metric_name}': {message}")]
    RegistrationError {
        /// Name of the metric
        metric_name: String,
        /// Error message
        message: String,
    },

    /// Metric not found with name and labels
    #[error("Metric not found: {name} (labels: {labels})")]
    MetricNotFound {
        /// Name of the metric
        name: String,
        /// Labels as string
        labels: String,
    },

    /// Invalid metric type with details
    #[error("Invalid metric type for '{metric_name}': expected {expected}, found {found}")]
    InvalidMetricType {
        /// Name of the metric
        metric_name: String,
        /// Expected type
        expected: String,
        /// Found type
        found: String,
    },

    /// Invalid metric value with constraints
    #[error("Invalid metric value for '{metric_name}': {message}")]
    InvalidValue {
        /// Name of the metric
        metric_name: String,
        /// Error message
        message: String,
    },

    /// Storage error with operation details
    #[error("Storage error during {operation}: {message}")]
    StorageError {
        /// Operation being performed
        operation: String,
        /// Error message
        message: String,
    },

    /// Export error with format details
    #[error("Export error to {format}: {message}")]
    ExportError {
        /// Export format (e.g., "Prometheus")
        format: String,
        /// Error message
        message: String,
    },

    /// Collection error with collector details
    #[error("Collection error in {collector}: {message}")]
    CollectionError {
        /// Name of the collector
        collector: String,
        /// Error message
        message: String,
    },

    /// Alert error with alert name
    #[error("Alert error for '{alert_name}': {message}")]
    AlertError {
        /// Name of the alert
        alert_name: String,
        /// Error message
        message: String,
    },

    /// Metric already exists with full identification
    #[error("Metric already exists: {name} (labels: {labels})")]
    AlreadyExists {
        /// Name of the metric
        name: String,
        /// Labels as string
        labels: String,
    },

    /// Registry lock error
    #[error("Failed to acquire registry lock: {operation}")]
    LockError {
        /// Operation that failed to acquire lock
        operation: String,
    },

    /// Not initialized
    #[error("Monitoring system not initialized")]
    NotInitialized,

    /// Already initialized
    #[error("Monitoring system already initialized")]
    AlreadyInitialized,

    /// Cardinality limit exceeded
    #[error(
        "Cardinality limit exceeded: limit={limit}, current={current}. Cannot create new metrics."
    )]
    CardinalityLimitExceeded {
        /// Maximum allowed cardinality
        limit: usize,
        /// Current cardinality
        current: usize,
    },

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// General error
    #[error("{0}")]
    Other(String),
}

impl MonitoringError {
    /// Create a configuration error
    pub fn config(component: impl Into<String>, message: impl Into<String>) -> Self {
        MonitoringError::ConfigError {
            component: component.into(),
            message: message.into(),
        }
    }

    /// Create a registration error
    pub fn registration(metric_name: impl Into<String>, message: impl Into<String>) -> Self {
        MonitoringError::RegistrationError {
            metric_name: metric_name.into(),
            message: message.into(),
        }
    }

    /// Create a metric not found error
    pub fn not_found(name: impl Into<String>, labels: impl Into<String>) -> Self {
        MonitoringError::MetricNotFound {
            name: name.into(),
            labels: labels.into(),
        }
    }

    /// Create an invalid metric type error
    pub fn invalid_type(
        metric_name: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        MonitoringError::InvalidMetricType {
            metric_name: metric_name.into(),
            expected: expected.into(),
            found: found.into(),
        }
    }

    /// Create an invalid value error
    pub fn invalid_value(metric_name: impl Into<String>, message: impl Into<String>) -> Self {
        MonitoringError::InvalidValue {
            metric_name: metric_name.into(),
            message: message.into(),
        }
    }

    /// Create a storage error
    pub fn storage(operation: impl Into<String>, message: impl Into<String>) -> Self {
        MonitoringError::StorageError {
            operation: operation.into(),
            message: message.into(),
        }
    }

    /// Create an export error
    pub fn export(format: impl Into<String>, message: impl Into<String>) -> Self {
        MonitoringError::ExportError {
            format: format.into(),
            message: message.into(),
        }
    }

    /// Create a collection error
    pub fn collection(collector: impl Into<String>, message: impl Into<String>) -> Self {
        MonitoringError::CollectionError {
            collector: collector.into(),
            message: message.into(),
        }
    }

    /// Create an alert error
    pub fn alert(alert_name: impl Into<String>, message: impl Into<String>) -> Self {
        MonitoringError::AlertError {
            alert_name: alert_name.into(),
            message: message.into(),
        }
    }

    /// Create an already exists error
    pub fn already_exists(name: impl Into<String>, labels: impl Into<String>) -> Self {
        MonitoringError::AlreadyExists {
            name: name.into(),
            labels: labels.into(),
        }
    }

    /// Create a lock error
    pub fn lock(operation: impl Into<String>) -> Self {
        MonitoringError::LockError {
            operation: operation.into(),
        }
    }

    /// Create a cardinality limit exceeded error
    pub fn cardinality_limit_exceeded(limit: usize, current: usize) -> Self {
        MonitoringError::CardinalityLimitExceeded { limit, current }
    }

    /// Create a generic error
    pub fn other<S: Into<String>>(msg: S) -> Self {
        MonitoringError::Other(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = MonitoringError::config("MetricRegistry", "Invalid scrape interval");
        assert!(matches!(err, MonitoringError::ConfigError { .. }));

        let err = MonitoringError::not_found("http_requests_total", "method=GET");
        assert!(matches!(err, MonitoringError::MetricNotFound { .. }));

        let err = MonitoringError::invalid_type("cpu_usage", "Gauge", "Counter");
        assert!(matches!(err, MonitoringError::InvalidMetricType { .. }));
    }

    #[test]
    fn test_error_display() {
        let err = MonitoringError::registration("http_requests", "Duplicate metric");
        assert_eq!(
            err.to_string(),
            "Failed to register metric 'http_requests': Duplicate metric"
        );

        let err = MonitoringError::export("Prometheus", "Connection refused");
        assert_eq!(
            err.to_string(),
            "Export error to Prometheus: Connection refused"
        );

        let err = MonitoringError::lock("write");
        assert_eq!(err.to_string(), "Failed to acquire registry lock: write");
    }
}
