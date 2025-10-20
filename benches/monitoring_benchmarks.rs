//! Criterion benchmarks for rust_monitoring_system

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rust_monitoring_system::prelude::*;
use std::collections::HashMap;

// ============================================================================
// Counter Benchmarks
// ============================================================================

fn bench_counter_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("counter");
    group.throughput(Throughput::Elements(1));

    group.bench_function("inc", |b| {
        let counter = Counter::new();
        b.iter(|| {
            counter.inc();
            black_box(&counter)
        });
    });

    group.bench_function("inc_by", |b| {
        let counter = Counter::new();
        b.iter(|| {
            counter.inc_by(black_box(100));
            black_box(&counter)
        });
    });

    group.bench_function("get", |b| {
        let counter = Counter::new();
        counter.inc_by(1000);
        b.iter(|| {
            let value = counter.get();
            black_box(value)
        });
    });

    group.bench_function("clone", |b| {
        let counter = Counter::new();
        b.iter(|| {
            let cloned = counter.clone();
            black_box(cloned)
        });
    });

    group.finish();
}

// ============================================================================
// Gauge Benchmarks
// ============================================================================

fn bench_gauge_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("gauge");
    group.throughput(Throughput::Elements(1));

    group.bench_function("set", |b| {
        let gauge = Gauge::new();
        b.iter(|| {
            gauge.set(black_box(42));
            black_box(&gauge)
        });
    });

    group.bench_function("inc", |b| {
        let gauge = Gauge::new();
        b.iter(|| {
            gauge.inc();
            black_box(&gauge)
        });
    });

    group.bench_function("dec", |b| {
        let gauge = Gauge::new();
        b.iter(|| {
            gauge.dec();
            black_box(&gauge)
        });
    });

    group.bench_function("inc_by", |b| {
        let gauge = Gauge::new();
        b.iter(|| {
            gauge.inc_by(black_box(100));
            black_box(&gauge)
        });
    });

    group.bench_function("get", |b| {
        let gauge = Gauge::new();
        gauge.set(1000);
        b.iter(|| {
            let value = gauge.get();
            black_box(value)
        });
    });

    group.finish();
}

// ============================================================================
// Histogram Benchmarks
// ============================================================================

fn bench_histogram_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram");

    group.bench_function("observe", |b| {
        let hist = Histogram::new();
        b.iter(|| {
            hist.observe(black_box(5.5));
        });
    });

    group.bench_function("observe_sequential", |b| {
        let hist = Histogram::new();
        let mut value = 0.0;
        b.iter(|| {
            hist.observe(value);
            value += 0.1;
            if value > 100.0 {
                value = 0.0;
            }
        });
    });

    group.bench_function("snapshot", |b| {
        let hist = Histogram::new();
        for i in 0..1000 {
            hist.observe(i as f64 / 100.0);
        }
        b.iter(|| {
            let snapshot = hist.snapshot();
            black_box(snapshot)
        });
    });

    group.bench_function("reset", |b| {
        let hist = Histogram::new();
        b.iter(|| {
            hist.reset();
            black_box(&hist)
        });
    });

    group.finish();
}

// ============================================================================
// Summary Benchmarks
// ============================================================================

fn bench_summary_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("summary");

    group.bench_function("observe", |b| {
        let summary = Summary::new();
        b.iter(|| {
            summary.observe(black_box(5.5));
        });
    });

    group.bench_function("observe_sequential", |b| {
        let summary = Summary::new();
        let mut value = 0.0;
        b.iter(|| {
            summary.observe(value);
            value += 0.1;
            if value > 100.0 {
                value = 0.0;
            }
        });
    });

    group.bench_function("snapshot", |b| {
        let summary = Summary::new();
        for i in 0..1000 {
            summary.observe(i as f64 / 100.0);
        }
        b.iter(|| {
            let snapshot = summary.snapshot();
            black_box(snapshot)
        });
    });

    group.bench_function("reset", |b| {
        let summary = Summary::new();
        b.iter(|| {
            summary.reset();
            black_box(&summary)
        });
    });

    group.finish();
}

// ============================================================================
// HistogramData (Direct) Benchmarks
// ============================================================================

fn bench_histogram_data_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_data");

    group.bench_function("observe_single", |b| {
        let mut hist = HistogramData::new(vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]);
        b.iter(|| {
            hist.observe(black_box(0.5));
        });
    });

    group.bench_function("observe_batch_10", |b| {
        let mut hist = HistogramData::new(vec![1.0, 5.0, 10.0]);
        b.iter(|| {
            for i in 0..10 {
                hist.observe((i as f64) * 0.5);
            }
        });
    });

    group.bench_function("clone", |b| {
        let mut hist = HistogramData::new(vec![1.0, 5.0, 10.0, 50.0, 100.0]);
        for i in 0..100 {
            hist.observe(i as f64);
        }
        b.iter(|| {
            let cloned = hist.clone();
            black_box(cloned)
        });
    });

    group.finish();
}

// ============================================================================
// SummaryData (Direct) Benchmarks
// ============================================================================

fn bench_summary_data_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("summary_data");

    group.bench_function("observe_single", |b| {
        let mut summary = SummaryData::new();
        b.iter(|| {
            summary.observe(black_box(0.5));
        });
    });

    group.bench_function("observe_batch_10", |b| {
        let mut summary = SummaryData::new();
        b.iter(|| {
            for i in 0..10 {
                summary.observe((i as f64) * 0.5);
            }
        });
    });

    group.bench_function("clone", |b| {
        let mut summary = SummaryData::new();
        for i in 0..100 {
            summary.observe(i as f64);
        }
        b.iter(|| {
            let cloned = summary.clone();
            black_box(cloned)
        });
    });

    group.finish();
}

// ============================================================================
// Metric Creation Benchmarks
// ============================================================================

fn bench_metric_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("metric_creation");
    let labels = HashMap::new();

    group.bench_function("counter", |b| {
        b.iter(|| {
            let metric = Metric::new(
                black_box("test_counter"),
                MetricType::Counter,
                black_box("Test counter metric"),
                labels.clone(),
            );
            black_box(metric)
        });
    });

    group.bench_function("gauge", |b| {
        b.iter(|| {
            let metric = Metric::new(
                black_box("test_gauge"),
                MetricType::Gauge,
                black_box("Test gauge metric"),
                labels.clone(),
            );
            black_box(metric)
        });
    });

    group.bench_function("histogram", |b| {
        b.iter(|| {
            let metric = Metric::new(
                black_box("test_histogram"),
                MetricType::Histogram,
                black_box("Test histogram metric"),
                labels.clone(),
            );
            black_box(metric)
        });
    });

    group.bench_function("summary", |b| {
        b.iter(|| {
            let metric = Metric::new(
                black_box("test_summary"),
                MetricType::Summary,
                black_box("Test summary metric"),
                labels.clone(),
            );
            black_box(metric)
        });
    });

    group.finish();
}

// ============================================================================
// Workload Simulation Benchmarks
// ============================================================================

fn bench_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workload");

    group.bench_function("http_request_tracking", |b| {
        // Simulate HTTP request tracking
        let request_count = Counter::new();
        let request_duration = Histogram::new();
        let active_connections = Gauge::new();

        b.iter(|| {
            // Simulate a request
            active_connections.inc();
            request_count.inc();
            request_duration.observe(black_box(0.125)); // 125ms
            active_connections.dec();
        });
    });

    group.bench_function("mixed_metrics_update", |b| {
        let counter = Counter::new();
        let gauge = Gauge::new();
        let histogram = Histogram::new();
        let summary = Summary::new();

        b.iter(|| {
            counter.inc();
            gauge.set(black_box(100));
            histogram.observe(black_box(1.5));
            summary.observe(black_box(2.5));
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_counter_operations,
    bench_gauge_operations,
    bench_histogram_operations,
    bench_summary_operations,
    bench_histogram_data_operations,
    bench_summary_data_operations,
    bench_metric_creation,
    bench_realistic_workload
);

criterion_main!(benches);
