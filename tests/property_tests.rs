//! Property-based tests for rust_monitoring_system using proptest

use proptest::prelude::*;
use rust_monitoring_system::prelude::*;
use std::collections::HashMap;

// ============================================================================
// Counter Tests
// ============================================================================

proptest! {
    /// Test that Counter increments work correctly
    #[test]
    fn test_counter_inc(increments in prop::collection::vec(1u64..100, 1..50)) {
        let counter = Counter::new();
        let expected_total: u64 = increments.iter().sum();

        for &inc in &increments {
            counter.inc_by(inc);
        }

        assert_eq!(counter.get(), expected_total);
    }

    /// Test that Counter single increments accumulate
    #[test]
    fn test_counter_inc_single(count in 0usize..1000) {
        let counter = Counter::new();

        for _ in 0..count {
            counter.inc();
        }

        assert_eq!(counter.get(), count as u64);
    }

    /// Test that Counter reset works
    #[test]
    fn test_counter_reset(value in 1u64..10000) {
        let counter = Counter::new();
        counter.inc_by(value);
        assert_eq!(counter.get(), value);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    /// Test that Counter clone shares state
    #[test]
    fn test_counter_clone(value in 1u64..1000) {
        let counter1 = Counter::new();
        let counter2 = counter1.clone();

        counter1.inc_by(value);
        assert_eq!(counter2.get(), value,
                   "Cloned counter should share state");
    }
}

// ============================================================================
// Gauge Tests
// ============================================================================

proptest! {
    /// Test that Gauge set works correctly
    #[test]
    fn test_gauge_set(value in any::<i64>()) {
        let gauge = Gauge::new();
        gauge.set(value);
        assert_eq!(gauge.get(), value);
    }

    /// Test that Gauge inc/dec work correctly
    #[test]
    fn test_gauge_inc_dec(
        start in -1000i64..1000,
        increments in prop::collection::vec(-100i64..100, 1..50)
    ) {
        let gauge = Gauge::new();
        gauge.set(start);

        let mut expected = start;
        for &inc in &increments {
            if inc >= 0 {
                gauge.inc_by(inc);
                expected = expected.wrapping_add(inc);
            } else {
                gauge.dec_by(-inc);
                expected = expected.wrapping_sub(-inc);
            }
        }

        assert_eq!(gauge.get(), expected);
    }

    /// Test that Gauge clone shares state
    #[test]
    fn test_gauge_clone(value in any::<i64>()) {
        let gauge1 = Gauge::new();
        let gauge2 = gauge1.clone();

        gauge1.set(value);
        assert_eq!(gauge2.get(), value,
                   "Cloned gauge should share state");
    }

    /// Test that Gauge reset works
    #[test]
    fn test_gauge_reset(value in any::<i64>()) {
        let gauge = Gauge::new();
        gauge.set(value);
        gauge.reset();
        assert_eq!(gauge.get(), 0);
    }
}

// ============================================================================
// HistogramData Tests (Security Critical: NaN/Infinity Protection)
// ============================================================================

proptest! {
    /// Test that Histogram rejects NaN values (prevents metric corruption)
    #[test]
    fn test_histogram_rejects_nan(_dummy in 0..100u32) {
        let mut hist = HistogramData::new(vec![1.0, 5.0, 10.0]);
        let initial_count = hist.count;

        hist.observe(f64::NAN);

        // NaN should be rejected
        assert_eq!(hist.count, initial_count,
                   "Histogram should reject NaN values");
        assert_eq!(hist.sum, 0.0,
                   "Histogram sum should not be corrupted by NaN");
    }

    /// Test that Histogram rejects Infinity values (prevents metric corruption)
    #[test]
    fn test_histogram_rejects_infinity(_dummy in 0..100u32) {
        let mut hist = HistogramData::new(vec![1.0, 5.0, 10.0]);
        let initial_count = hist.count;

        hist.observe(f64::INFINITY);
        hist.observe(f64::NEG_INFINITY);

        // Infinity should be rejected
        assert_eq!(hist.count, initial_count,
                   "Histogram should reject Infinity values");
    }

    /// Test that Histogram handles finite values correctly
    #[test]
    fn test_histogram_finite_values(
        values in prop::collection::vec(
            any::<f64>().prop_filter("finite", |v| v.is_finite()),
            1..100
        )
    ) {
        let mut hist = HistogramData::new(vec![0.0, 1.0, 5.0, 10.0, 100.0]);

        for &value in &values {
            hist.observe(value);
        }

        assert_eq!(hist.count, values.len() as u64);
        let expected_sum: f64 = values.iter().sum();
        assert!((hist.sum - expected_sum).abs() < 1e-6,
                "Histogram sum mismatch: {} vs {}", hist.sum, expected_sum);
    }

    /// Test that Histogram buckets count correctly
    #[test]
    fn test_histogram_bucket_counts(
        values in prop::collection::vec(0.0f64..20.0, 10..50)
    ) {
        let buckets = vec![5.0, 10.0, 15.0];
        let mut hist = HistogramData::new(buckets.clone());

        for &value in &values {
            hist.observe(value);
        }

        // Verify total count matches
        let total_in_buckets: u64 = hist.counts.iter().sum();
        assert_eq!(total_in_buckets, values.len() as u64,
                   "Total bucket counts should match observations");
    }

    /// Test that Histogram clone preserves data
    #[test]
    fn test_histogram_clone(count in 1u64..100) {
        let mut hist = HistogramData::new(vec![1.0, 5.0, 10.0]);

        for i in 0..count {
            hist.observe(i as f64);
        }

        let cloned = hist.clone();
        assert_eq!(cloned.count, hist.count);
        assert_eq!(cloned.sum, hist.sum);
        assert_eq!(cloned.counts, hist.counts);
    }
}

// ============================================================================
// SummaryData Tests (Security Critical: NaN/Infinity Protection)
// ============================================================================

proptest! {
    /// Test that Summary rejects NaN values (prevents metric corruption)
    #[test]
    fn test_summary_rejects_nan(_dummy in 0..100u32) {
        let mut summary = SummaryData::new();
        let initial_count = summary.count;

        summary.observe(f64::NAN);

        // NaN should be rejected
        assert_eq!(summary.count, initial_count,
                   "Summary should reject NaN values");
        assert_eq!(summary.sum, 0.0,
                   "Summary sum should not be corrupted by NaN");
    }

    /// Test that Summary rejects Infinity values (prevents metric corruption)
    #[test]
    fn test_summary_rejects_infinity(_dummy in 0..100u32) {
        let mut summary = SummaryData::new();
        let initial_count = summary.count;

        summary.observe(f64::INFINITY);
        summary.observe(f64::NEG_INFINITY);

        // Infinity should be rejected
        assert_eq!(summary.count, initial_count,
                   "Summary should reject Infinity values");
    }

    /// Test that Summary calculates statistics correctly
    #[test]
    fn test_summary_statistics(
        values in prop::collection::vec(
            any::<f64>().prop_filter("finite", |v| v.is_finite() && v.abs() < 1e10),
            2..100
        )
    ) {
        let mut summary = SummaryData::new();

        for &value in &values {
            summary.observe(value);
        }

        assert_eq!(summary.count, values.len() as u64);

        let expected_sum: f64 = values.iter().sum();
        assert!((summary.sum - expected_sum).abs() < 1e-6);

        let expected_min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let expected_max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert_eq!(summary.min, expected_min);
        assert_eq!(summary.max, expected_max);

        let expected_mean = expected_sum / values.len() as f64;
        assert!((summary.mean - expected_mean).abs() < 1e-6,
                "Mean mismatch: {} vs {}", summary.mean, expected_mean);
    }

    /// Test that Summary handles single value correctly
    #[test]
    fn test_summary_single_value(value in any::<f64>().prop_filter("finite", |v| v.is_finite())) {
        let mut summary = SummaryData::new();
        summary.observe(value);

        assert_eq!(summary.count, 1);
        assert_eq!(summary.sum, value);
        assert_eq!(summary.min, value);
        assert_eq!(summary.max, value);
        assert_eq!(summary.mean, value);
    }
}

// ============================================================================
// Thread-Safe Histogram Tests
// ============================================================================

proptest! {
    /// Test that Histogram observe works correctly
    #[test]
    fn test_histogram_observe(
        values in prop::collection::vec(
            any::<f64>().prop_filter("finite", |v| v.is_finite() && *v >= 0.0 && *v < 100.0),
            1..50
        )
    ) {
        let hist = Histogram::new();

        for &value in &values {
            hist.observe(value);
        }

        let snapshot = hist.snapshot();
        assert_eq!(snapshot.count, values.len() as u64);
    }

    /// Test that Histogram reset works
    #[test]
    fn test_histogram_reset(count in 1usize..50) {
        let hist = Histogram::new();

        for i in 0..count {
            hist.observe(i as f64);
        }

        hist.reset();
        let snapshot = hist.snapshot();
        assert_eq!(snapshot.count, 0);
        assert_eq!(snapshot.sum, 0.0);
    }

    /// Test that Histogram clone shares state
    #[test]
    fn test_histogram_shared_clone(value in 0.0f64..100.0) {
        let hist1 = Histogram::new();
        let hist2 = hist1.clone();

        hist1.observe(value);

        let snapshot1 = hist1.snapshot();
        let snapshot2 = hist2.snapshot();
        assert_eq!(snapshot1.count, snapshot2.count,
                   "Cloned histogram should share state");
    }
}

// ============================================================================
// Thread-Safe Summary Tests
// ============================================================================

proptest! {
    /// Test that Summary observe works correctly
    #[test]
    fn test_summary_observe(
        values in prop::collection::vec(
            any::<f64>().prop_filter("finite", |v| v.is_finite() && v.abs() < 1000.0),
            1..50
        )
    ) {
        let summary = Summary::new();

        for &value in &values {
            summary.observe(value);
        }

        let snapshot = summary.snapshot();
        assert_eq!(snapshot.count, values.len() as u64);
    }

    /// Test that Summary reset works
    #[test]
    fn test_summary_reset(count in 1usize..50) {
        let summary = Summary::new();

        for i in 0..count {
            summary.observe(i as f64);
        }

        summary.reset();
        let snapshot = summary.snapshot();
        assert_eq!(snapshot.count, 0);
    }

    /// Test that Summary clone shares state
    #[test]
    fn test_summary_shared_clone(value in -100.0f64..100.0) {
        let summary1 = Summary::new();
        let summary2 = summary1.clone();

        summary1.observe(value);

        let snapshot1 = summary1.snapshot();
        let snapshot2 = summary2.snapshot();
        assert_eq!(snapshot1.count, snapshot2.count,
                   "Cloned summary should share state");
    }
}

// ============================================================================
// MetricType Tests
// ============================================================================

proptest! {
    /// Test that MetricType as_str is consistent
    #[test]
    fn test_metric_type_as_str(_dummy in 0..100u32) {
        assert_eq!(MetricType::Counter.as_str(), "counter");
        assert_eq!(MetricType::Gauge.as_str(), "gauge");
        assert_eq!(MetricType::Histogram.as_str(), "histogram");
        assert_eq!(MetricType::Summary.as_str(), "summary");
        assert_eq!(MetricType::Timer.as_str(), "timer");
    }
}

// ============================================================================
// Metric Creation Tests
// ============================================================================

proptest! {
    /// Test that Metric creation works for all types
    #[test]
    fn test_metric_creation(
        name in "[a-z]{3,20}",
        help in ".*"
    ) {
        let labels = HashMap::new();

        let counter = Metric::new(&name, MetricType::Counter, &help, labels.clone());
        assert_eq!(counter.name, name);
        assert_eq!(counter.metric_type, MetricType::Counter);

        let gauge = Metric::new(&name, MetricType::Gauge, &help, labels.clone());
        assert_eq!(gauge.metric_type, MetricType::Gauge);

        let histogram = Metric::new(&name, MetricType::Histogram, &help, labels.clone());
        assert_eq!(histogram.metric_type, MetricType::Histogram);

        let summary = Metric::new(&name, MetricType::Summary, &help, labels);
        assert_eq!(summary.metric_type, MetricType::Summary);
    }

    /// Test that Metric timestamp update works
    #[test]
    fn test_metric_timestamp_update(_dummy in 0..100u32) {
        let mut metric = Metric::new("test", MetricType::Counter, "help", HashMap::new());
        let original_ts = metric.timestamp;

        std::thread::sleep(std::time::Duration::from_millis(1));
        metric.update_timestamp();

        assert!(metric.timestamp > original_ts,
                "Timestamp should be updated");
    }
}

// ============================================================================
// Safety Tests (No Panics)
// ============================================================================

proptest! {
    /// Test that metrics never panic on extreme values
    #[test]
    fn test_counter_no_panic(value in any::<u64>()) {
        let counter = Counter::new();
        counter.inc_by(value);
        let _ = counter.get();
    }

    /// Test that gauge never panics on extreme values
    #[test]
    fn test_gauge_no_panic(value in any::<i64>()) {
        let gauge = Gauge::new();
        gauge.set(value);
        let _ = gauge.get();
    }

    /// Test that histogram never panics on any f64 value
    #[test]
    fn test_histogram_no_panic(value in any::<f64>()) {
        let mut hist = HistogramData::new(vec![1.0, 10.0]);
        hist.observe(value);  // Should not panic, even for NaN/Infinity
    }

    /// Test that summary never panics on any f64 value
    #[test]
    fn test_summary_no_panic(value in any::<f64>()) {
        let mut summary = SummaryData::new();
        summary.observe(value);  // Should not panic, even for NaN/Infinity
    }
}
