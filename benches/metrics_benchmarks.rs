use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use rust_monitoring_system::prelude::*;
use std::sync::Arc;
use std::thread;

fn benchmark_counter_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("counter_operations");

    group.bench_function("counter_inc_single_thread", |b| {
        let counter = Counter::new();
        b.iter(|| {
            counter.inc();
        });
    });

    group.bench_function("counter_inc_by_single_thread", |b| {
        let counter = Counter::new();
        b.iter(|| {
            counter.inc_by(black_box(100));
        });
    });

    group.bench_function("counter_inc_multi_thread", |b| {
        b.iter_batched(
            Counter::new,
            |counter| {
                let counter = Arc::new(counter);
                let handles: Vec<_> = (0..4)
                    .map(|_| {
                        let counter = Arc::clone(&counter);
                        thread::spawn(move || {
                            for _ in 0..1000 {
                                counter.inc();
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().expect("Thread panicked");
                }

                assert_eq!(counter.get(), 4000);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn benchmark_gauge_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("gauge_operations");

    group.bench_function("gauge_set", |b| {
        let gauge = Gauge::new();
        let mut value = 0i64;
        b.iter(|| {
            value = value.wrapping_add(1);
            gauge.set(black_box(value));
        });
    });

    group.bench_function("gauge_inc_dec", |b| {
        let gauge = Gauge::new();
        b.iter(|| {
            gauge.inc();
            gauge.dec();
        });
    });

    group.finish();
}

fn benchmark_histogram_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_operations");

    group.bench_function("histogram_observe", |b| {
        let mut hist = HistogramData::new(vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]);
        let mut value = 0.0;
        b.iter(|| {
            value = (value + 0.01) % 10.0;
            hist.observe(black_box(value));
        });
    });

    group.bench_function("histogram_observe_with_validation", |b| {
        let mut hist = HistogramData::new(vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]);
        b.iter(|| {
            // Test with valid values
            hist.observe(black_box(0.5));
            // Test with edge cases (will be rejected)
            hist.observe(f64::NAN);
            hist.observe(f64::INFINITY);
        });
    });

    group.finish();
}

fn benchmark_summary_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("summary_operations");

    group.bench_function("summary_observe", |b| {
        let mut summary = SummaryData::new();
        let mut value = 0.0;
        b.iter(|| {
            value = (value + 1.0) % 100.0;
            summary.observe(black_box(value));
        });
    });

    group.finish();
}

fn benchmark_registry_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_operations");

    group.bench_function("get_or_create_counter_fast_path", |b| {
        let registry = MetricRegistry::new();
        let labels = std::collections::HashMap::new();

        // Warm up - create the counter first
        let _ = registry.get_or_create_counter("test_counter", labels.clone());

        b.iter(|| {
            let _ = registry.get_or_create_counter("test_counter", labels.clone());
        });
    });

    group.bench_function("get_or_create_counter_slow_path", |b| {
        b.iter_batched(
            MetricRegistry::new,
            |registry| {
                let labels = std::collections::HashMap::new();
                // Always creates new counter (slow path)
                let _ = registry.get_or_create_counter("test_counter", labels);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("registry_concurrent_access", |b| {
        b.iter_batched(
            || Arc::new(MetricRegistry::new()),
            |registry| {
                let handles: Vec<_> = (0..4)
                    .map(|thread_id| {
                        let registry = Arc::clone(&registry);
                        thread::spawn(move || {
                            for i in 0..100 {
                                let mut labels = std::collections::HashMap::new();
                                labels.insert("thread".to_string(), thread_id.to_string());
                                labels.insert("iteration".to_string(), i.to_string());

                                let counter =
                                    registry.get_or_create_counter("concurrent_test", labels);
                                counter.inc();
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().expect("Thread panicked");
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn benchmark_monitor_operations(c: &mut Criterion) {
    c.bench_function("monitor_counter_increment", |b| {
        let monitor = Monitor::new();
        let labels = std::collections::HashMap::new();
        let counter = monitor.counter("bench_counter", labels);

        b.iter(|| {
            counter.inc();
        });
    });
}

criterion_group!(
    benches,
    benchmark_counter_operations,
    benchmark_gauge_operations,
    benchmark_histogram_operations,
    benchmark_summary_operations,
    benchmark_registry_operations,
    benchmark_monitor_operations
);
criterion_main!(benches);
