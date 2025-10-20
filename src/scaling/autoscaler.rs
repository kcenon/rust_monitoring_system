//! Auto-scaling based on metrics

use crate::core::MetricRegistry;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Scaling direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingDirection {
    /// Scale up (increase instances)
    Up,
    /// Scale down (decrease instances)
    Down,
    /// No scaling action needed
    None,
}

/// Scaling decision
#[derive(Debug, Clone)]
pub struct ScalingDecision {
    /// Direction to scale
    pub direction: ScalingDirection,
    /// Current instance count
    pub current_instances: usize,
    /// Target instance count
    pub target_instances: usize,
    /// Reason for scaling
    pub reason: String,
}

/// Metric-based scaling rule
#[derive(Debug, Clone)]
pub struct ScalingRule {
    /// Metric name to monitor
    pub metric_name: String,
    /// Threshold for scaling up
    pub scale_up_threshold: i64,
    /// Threshold for scaling down
    pub scale_down_threshold: i64,
    /// Number of consecutive breaches before scaling
    pub breach_duration: usize,
}

impl ScalingRule {
    /// Create a new scaling rule
    pub fn new(
        metric_name: impl Into<String>,
        scale_up_threshold: i64,
        scale_down_threshold: i64,
    ) -> Self {
        Self {
            metric_name: metric_name.into(),
            scale_up_threshold,
            scale_down_threshold,
            breach_duration: 3,
        }
    }

    /// Set breach duration
    pub fn with_breach_duration(mut self, duration: usize) -> Self {
        self.breach_duration = duration;
        self
    }
}

/// Auto-scaler configuration
#[derive(Debug, Clone)]
pub struct AutoScalerConfig {
    /// Minimum number of instances
    pub min_instances: usize,
    /// Maximum number of instances
    pub max_instances: usize,
    /// Cooldown period after scaling
    pub cooldown: Duration,
    /// Check interval
    pub check_interval: Duration,
    /// Scaling rules
    pub rules: Vec<ScalingRule>,
}

impl Default for AutoScalerConfig {
    fn default() -> Self {
        Self {
            min_instances: 1,
            max_instances: 10,
            cooldown: Duration::from_secs(300),
            check_interval: Duration::from_secs(60),
            rules: Vec::new(),
        }
    }
}

impl AutoScalerConfig {
    /// Create a new configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum instances
    pub fn with_min_instances(mut self, min: usize) -> Self {
        self.min_instances = min;
        self
    }

    /// Set maximum instances
    pub fn with_max_instances(mut self, max: usize) -> Self {
        self.max_instances = max;
        self
    }

    /// Set cooldown period
    pub fn with_cooldown(mut self, cooldown: Duration) -> Self {
        self.cooldown = cooldown;
        self
    }

    /// Add a scaling rule
    pub fn add_rule(mut self, rule: ScalingRule) -> Self {
        self.rules.push(rule);
        self
    }
}

/// Auto-scaler state
struct ScalerState {
    current_instances: AtomicUsize,
    last_scale_time: parking_lot::Mutex<std::time::Instant>,
    breach_counts: parking_lot::Mutex<std::collections::HashMap<String, usize>>,
}

/// Auto-scaler for dynamic capacity management
pub struct AutoScaler {
    config: AutoScalerConfig,
    registry: Arc<MetricRegistry>,
    state: Arc<ScalerState>,
}

impl AutoScaler {
    /// Create a new auto-scaler
    pub fn new(config: AutoScalerConfig, registry: Arc<MetricRegistry>) -> Self {
        let initial_instances = config.min_instances;

        Self {
            config,
            registry,
            state: Arc::new(ScalerState {
                current_instances: AtomicUsize::new(initial_instances),
                last_scale_time: parking_lot::Mutex::new(std::time::Instant::now()),
                breach_counts: parking_lot::Mutex::new(std::collections::HashMap::new()),
            }),
        }
    }

    /// Get current instance count
    pub fn current_instances(&self) -> usize {
        self.state.current_instances.load(Ordering::Acquire)
    }

    /// Set instance count manually
    pub fn set_instances(&self, count: usize) {
        let count = count.clamp(self.config.min_instances, self.config.max_instances);
        self.state.current_instances.store(count, Ordering::Release);
        *self.state.last_scale_time.lock() = std::time::Instant::now();
    }

    /// Check if in cooldown period
    fn is_in_cooldown(&self) -> bool {
        let last_scale = self.state.last_scale_time.lock();
        last_scale.elapsed() < self.config.cooldown
    }

    /// Evaluate scaling decision
    pub fn evaluate(&self) -> Option<ScalingDecision> {
        if self.is_in_cooldown() {
            return None;
        }

        let metrics = self.registry.collect();

        for rule in &self.config.rules {
            // Find the metric value
            let metric_value = metrics
                .iter()
                .find(|m| m.name == rule.metric_name && m.labels.is_empty())
                .and_then(|m| Self::metric_value_to_i64(&m.value));

            if let Some(value) = metric_value {
                if let Some(decision) = self.evaluate_rule(rule, value) {
                    return Some(decision);
                }
            }
        }

        None
    }

    /// Convert MetricValue to i64 for comparison
    fn metric_value_to_i64(value: &crate::core::MetricValue) -> Option<i64> {
        use crate::core::MetricValue;
        match value {
            MetricValue::Int(v) => Some(*v),
            MetricValue::Uint(v) => i64::try_from(*v).ok(),
            MetricValue::Float(v) => Some(*v as i64),
            MetricValue::Histogram(_) | MetricValue::Summary(_) => None,
        }
    }

    /// Evaluate a single rule
    fn evaluate_rule(&self, rule: &ScalingRule, value: i64) -> Option<ScalingDecision> {
        let current = self.current_instances();

        // Check if we should scale up
        if value >= rule.scale_up_threshold {
            let mut breach_counts = self.state.breach_counts.lock();
            let count = breach_counts
                .entry(format!("{}_up", rule.metric_name))
                .or_insert(0);
            *count += 1;

            if *count >= rule.breach_duration {
                *count = 0;
                let target = (current + 1).min(self.config.max_instances);

                if target > current {
                    return Some(ScalingDecision {
                        direction: ScalingDirection::Up,
                        current_instances: current,
                        target_instances: target,
                        reason: format!(
                            "{} ({}) >= threshold ({})",
                            rule.metric_name, value, rule.scale_up_threshold
                        ),
                    });
                }
            }
        }
        // Check if we should scale down
        else if value <= rule.scale_down_threshold {
            let mut breach_counts = self.state.breach_counts.lock();
            let count = breach_counts
                .entry(format!("{}_down", rule.metric_name))
                .or_insert(0);
            *count += 1;

            if *count >= rule.breach_duration {
                *count = 0;
                let target = current.saturating_sub(1).max(self.config.min_instances);

                if target < current {
                    return Some(ScalingDecision {
                        direction: ScalingDirection::Down,
                        current_instances: current,
                        target_instances: target,
                        reason: format!(
                            "{} ({}) <= threshold ({})",
                            rule.metric_name, value, rule.scale_down_threshold
                        ),
                    });
                }
            }
        } else {
            // Reset breach counts if value is in normal range
            let mut breach_counts = self.state.breach_counts.lock();
            breach_counts.remove(&format!("{}_up", rule.metric_name));
            breach_counts.remove(&format!("{}_down", rule.metric_name));
        }

        None
    }

    /// Apply a scaling decision
    pub fn apply_decision(&self, decision: &ScalingDecision) {
        self.set_instances(decision.target_instances);
    }

    /// Start auto-scaling loop
    pub fn start<F>(self: Arc<Self>, scale_fn: F) -> tokio::task::JoinHandle<()>
    where
        F: Fn(ScalingDecision) + Send + Sync + 'static,
    {
        let scale_fn = Arc::new(scale_fn);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.config.check_interval);

            loop {
                interval.tick().await;

                if let Some(decision) = self.evaluate() {
                    println!("🔄 Scaling decision: {:?}", decision);
                    scale_fn(decision.clone());
                    self.apply_decision(&decision);
                }
            }
        })
    }
}

/// Predictive auto-scaler using historical data
pub struct PredictiveScaler {
    base_scaler: AutoScaler,
    history: Arc<parking_lot::Mutex<Vec<(std::time::Instant, usize)>>>,
    prediction_window: Duration,
}

impl PredictiveScaler {
    /// Create a new predictive scaler
    pub fn new(
        config: AutoScalerConfig,
        registry: Arc<MetricRegistry>,
        prediction_window: Duration,
    ) -> Self {
        Self {
            base_scaler: AutoScaler::new(config, registry),
            history: Arc::new(parking_lot::Mutex::new(Vec::new())),
            prediction_window,
        }
    }

    /// Record current load
    pub fn record_load(&self, load: usize) {
        let mut history = self.history.lock();
        history.push((std::time::Instant::now(), load));

        // Keep only recent history
        let cutoff = std::time::Instant::now() - self.prediction_window;
        history.retain(|(time, _)| *time > cutoff);
    }

    /// Predict future load based on trend
    pub fn predict_load(&self) -> Option<f64> {
        let history = self.history.lock();

        if history.len() < 2 {
            return None;
        }

        // Simple linear regression
        let n = history.len() as f64;
        let sum_x: f64 = (0..history.len()).map(|i| i as f64).sum();
        let sum_y: f64 = history.iter().map(|(_, load)| *load as f64).sum();
        let sum_xy: f64 = history
            .iter()
            .enumerate()
            .map(|(i, (_, load))| i as f64 * *load as f64)
            .sum();
        let sum_x2: f64 = (0..history.len()).map(|i| (i as f64).powi(2)).sum();

        // Check for zero variance to prevent division by zero
        let denominator = n * sum_x2 - sum_x.powi(2);
        if denominator.abs() < f64::EPSILON {
            tracing::warn!("Cannot compute linear regression: zero variance in data");
            return None;
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;
        let intercept = (sum_y - slope * sum_x) / n;

        // Predict next value
        Some(slope * n + intercept)
    }

    /// Get current instance count
    pub fn current_instances(&self) -> usize {
        self.base_scaler.current_instances()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaling_rule() {
        let rule = ScalingRule::new("cpu_usage", 80, 20).with_breach_duration(2);

        assert_eq!(rule.metric_name, "cpu_usage");
        assert_eq!(rule.scale_up_threshold, 80);
        assert_eq!(rule.scale_down_threshold, 20);
        assert_eq!(rule.breach_duration, 2);
    }

    #[test]
    fn test_autoscaler_creation() {
        let registry = Arc::new(MetricRegistry::new());
        let config = AutoScalerConfig::new()
            .with_min_instances(2)
            .with_max_instances(10);

        let scaler = AutoScaler::new(config, registry);

        assert_eq!(scaler.current_instances(), 2);
    }

    #[test]
    fn test_set_instances() {
        let registry = Arc::new(MetricRegistry::new());
        let config = AutoScalerConfig::new()
            .with_min_instances(1)
            .with_max_instances(5);

        let scaler = AutoScaler::new(config, registry);

        scaler.set_instances(3);
        assert_eq!(scaler.current_instances(), 3);

        // Should clamp to max
        scaler.set_instances(10);
        assert_eq!(scaler.current_instances(), 5);

        // Should clamp to min
        scaler.set_instances(0);
        assert_eq!(scaler.current_instances(), 1);
    }

    #[test]
    fn test_scale_up_decision() {
        let registry = Arc::new(MetricRegistry::new());

        // Set high CPU usage
        registry
            .get_or_create_gauge("cpu_usage", std::collections::HashMap::new())
            .set(85);

        let rule = ScalingRule::new("cpu_usage", 80, 20).with_breach_duration(1);

        let config = AutoScalerConfig::new()
            .with_min_instances(1)
            .with_max_instances(10)
            .with_cooldown(Duration::from_secs(0))
            .add_rule(rule);

        let scaler = AutoScaler::new(config, Arc::clone(&registry));

        let decision = scaler.evaluate();
        assert!(decision.is_some());

        let decision = decision.expect("Expected a scaling decision");
        assert_eq!(decision.direction, ScalingDirection::Up);
        assert_eq!(decision.target_instances, 2);
    }

    #[test]
    fn test_scale_down_decision() {
        let registry = Arc::new(MetricRegistry::new());

        // Set low CPU usage
        registry
            .get_or_create_gauge("cpu_usage", std::collections::HashMap::new())
            .set(10);

        let rule = ScalingRule::new("cpu_usage", 80, 20).with_breach_duration(1);

        let config = AutoScalerConfig::new()
            .with_min_instances(1)
            .with_max_instances(10)
            .with_cooldown(Duration::from_secs(0))
            .add_rule(rule);

        let scaler = AutoScaler::new(config, Arc::clone(&registry));
        scaler.set_instances(5);

        let decision = scaler.evaluate();
        assert!(decision.is_some());

        let decision = decision.expect("Expected a scaling down decision");
        assert_eq!(decision.direction, ScalingDirection::Down);
        assert_eq!(decision.target_instances, 4);
    }

    #[test]
    fn test_predictive_scaler() {
        let registry = Arc::new(MetricRegistry::new());
        let config = AutoScalerConfig::new();

        let scaler = PredictiveScaler::new(config, registry, Duration::from_secs(300));

        scaler.record_load(10);
        scaler.record_load(20);
        scaler.record_load(30);

        let prediction = scaler.predict_load();
        assert!(prediction.is_some());
        assert!(prediction.expect("Expected a load prediction") > 30.0); // Should predict upward trend
    }

    #[test]
    fn test_breach_duration() {
        let registry = Arc::new(MetricRegistry::new());

        registry
            .get_or_create_gauge("cpu_usage", std::collections::HashMap::new())
            .set(85);

        let rule = ScalingRule::new("cpu_usage", 80, 20).with_breach_duration(3);

        let config = AutoScalerConfig::new()
            .with_cooldown(Duration::from_secs(0))
            .add_rule(rule);

        let scaler = AutoScaler::new(config, Arc::clone(&registry));

        // First two evaluations should not scale
        assert!(scaler.evaluate().is_none());
        assert!(scaler.evaluate().is_none());

        // Third evaluation should trigger scale
        let decision = scaler.evaluate();
        assert!(decision.is_some());
    }
}
