# Rust Monitoring System

[English](README.md) | [한국어](README.ko.md)

시스템 관찰성 및 메트릭 수집을 위한 프로덕션 준비 완료된 고성능 Rust 모니터링 프레임워크입니다.

## 주요 기능

- **실시간 메트릭 수집**: 최소한의 오버헤드로 시스템 및 애플리케이션 메트릭 추적
- **다양한 메트릭 타입**: Counter, Gauge, Histogram, Summary, Timer
- **스레드 안전 작업**: 모든 메트릭은 안전한 동시 액세스를 위해 원자적 연산 사용
- **유연한 레이블링**: 키-값 레이블을 사용한 다차원 메트릭
- **Prometheus 내보내기**: 내장된 Prometheus 텍스트 형식 exporter
- **시스템 Collector**: CPU, 메모리, 시스템 가동시간을 위한 사전 구축된 collector
- **낮은 오버헤드**: 최소 성능 영향을 위해 최적화 (<1% 오버헤드)
- **타입 안전 API**: Result 타입을 사용한 포괄적인 에러 처리

## 빠른 시작

`Cargo.toml`에 추가:

```toml
[dependencies]
rust_monitoring_system = "0.1.0"
```

기본 사용법:

```rust
use rust_monitoring_system::prelude::*;
use std::collections::HashMap;

fn main() -> Result<()> {
    // Create and start monitor
    let monitor = Monitor::new();
    monitor.start()?;

    // Create a counter
    let counter = monitor.counter(
        "requests_total",
        "Total number of requests",
        HashMap::new()
    );

    // Increment counter
    counter.inc();
    println!("Requests: {}", counter.get());

    // Create a gauge
    let gauge = monitor.gauge(
        "active_connections",
        "Number of active connections",
        HashMap::new()
    );

    gauge.set(42);

    // Collect and export metrics
    let metrics = monitor.collect();
    let exporter = PrometheusExporter::new();
    let output = exporter.export(&metrics)?;

    println!("{}", output);

    Ok(())
}
```

## 아키텍처

### 핵심 구성요소

- **Monitor**: 메트릭 생명주기를 관리하는 메인 모니터링 시스템
- **MetricRegistry**: 메트릭 저장 및 관리를 위한 스레드 안전 레지스트리
- **Metric Types**: Counter, Gauge, Histogram, Summary, Timer
- **Collectors**: 시스템 및 커스텀 메트릭 collector
- **Exporters**: Prometheus 및 커스텀 형식 exporter

### 설계 원칙

1. **성능 우선**: 가능한 경우 원자적 연산 및 lock-free 데이터 구조 사용
2. **타입 안전성**: Result 패턴을 사용한 포괄적인 에러 처리
3. **유연성**: 커스텀 collector 및 exporter 지원
4. **프로덕션 준비**: 높은 처리량, 낮은 지연시간 시나리오를 위해 구축

## 사용 예제

### 레이블을 사용한 메트릭

```rust
use rust_monitoring_system::prelude::*;
use std::collections::HashMap;

let monitor = Monitor::new();

// Create labeled metrics
let mut labels = HashMap::new();
labels.insert("method".to_string(), "GET".to_string());
labels.insert("endpoint".to_string(), "/api/users".to_string());

let counter = monitor.counter(
    "http_requests_total",
    "Total HTTP requests",
    labels
);

counter.inc();
```

### 시스템 모니터링

```rust
use rust_monitoring_system::prelude::*;
use std::sync::Arc;

let monitor = Arc::new(Monitor::new());
monitor.start()?;

// Create system collector
let collector = SystemCollector::new(monitor.clone())?;

// Collect system metrics
collector.collect()?;

// View metrics
let metrics = monitor.collect();
for metric in metrics {
    println!("{}: {:?}", metric.name, metric.value);
}
```

### 커스텀 설정

```rust
use rust_monitoring_system::prelude::*;
use std::time::Duration;

let config = MonitorConfig::new("my_service")
    .with_interval(Duration::from_secs(30))
    .with_auto_collect(true);

let monitor = Monitor::with_config(config);
```

### Histogram 메트릭

```rust
use rust_monitoring_system::prelude::*;

let monitor = Monitor::new();

// Create histogram with custom buckets
let mut histogram_metric = Metric::new(
    "request_duration_seconds",
    MetricType::Histogram,
    "Request duration",
    HashMap::new()
);

// Observe values (requires manual observation for now)
// Future versions will include histogram metric helpers
```

## 예제

`examples/` 디렉토리에는 여러 완전한 예제가 포함되어 있습니다:

- **basic_usage.rs**: 간단한 counter 및 gauge 사용법
- **system_monitoring.rs**: 시스템 메트릭 수집
- **advanced_metrics.rs**: 다중 컴포넌트 모니터링 시뮬레이션

예제 실행:

```bash
cargo run --example basic_usage
cargo run --example system_monitoring
cargo run --example advanced_metrics
```

## 성능 특성

- **메트릭 업데이트**: 작업당 ~10-50 나노초 (원자적 counter/gauge)
- **수집**: O(n), 여기서 n은 등록된 메트릭의 수
- **내보내기**: Prometheus 텍스트 형식의 경우 O(n)
- **메모리 오버헤드**: 메트릭당 ~200 바이트 (레이블 제외)
- **처리량**: 최신 하드웨어에서 초당 10M+ 작업

### 벤치마크

벤치마크 실행:

```bash
cargo bench
```

예상 성능 (최신 하드웨어 기준):
- Counter 증가: ~10ns
- Gauge 업데이트: ~15ns
- 메트릭 수집 (1000개 메트릭): ~50μs
- Prometheus 내보내기 (1000개 메트릭): ~200μs

## 메트릭 타입

### Counter

단조 증가 카운터:

```rust
let counter = monitor.counter("name", "help", labels);
counter.inc();         // Increment by 1
counter.inc_by(10);    // Increment by 10
let value = counter.get();
counter.reset();       // Reset to 0
```

### Gauge

증가 또는 감소할 수 있는 값:

```rust
let gauge = monitor.gauge("name", "help", labels);
gauge.set(42);         // Set to specific value
gauge.inc();           // Increment by 1
gauge.dec();           // Decrement by 1
gauge.inc_by(10);      // Increment by 10
gauge.dec_by(5);       // Decrement by 5
let value = gauge.get();
```

### Histogram

버킷 내 값의 분포:

```rust
let mut hist = HistogramData::new(vec![0.1, 1.0, 10.0, 100.0]);
hist.observe(0.5);     // Observe a value
hist.observe(5.0);
// Histogram includes: count, sum, buckets
```

### Summary

관측값의 통계 요약:

```rust
let mut summary = SummaryData::new();
summary.observe(1.0);
summary.observe(2.0);
summary.observe(3.0);
// Summary includes: count, sum, min, max, mean
```

## Prometheus 내보내기

Prometheus 텍스트 형식으로 메트릭 내보내기:

```rust
let exporter = PrometheusExporter::new();
let metrics = monitor.collect();
let output = exporter.export(&metrics)?;

// Output format:
// # HELP metric_name Help text
// # TYPE metric_name counter
// metric_name{label="value"} 42
```

## 스레드 안전성

모든 public API는 스레드 안전합니다:

- `Counter`와 `Gauge`는 원자적 연산 사용
- `MetricRegistry`는 동시 액세스를 위해 RwLock 사용
- `Monitor`는 `Arc`를 통해 안전하게 공유 가능

```rust
use std::sync::Arc;
use std::thread;

let monitor = Arc::new(Monitor::new());
let counter = monitor.counter("requests", "help", HashMap::new());

let handles: Vec<_> = (0..10)
    .map(|_| {
        let counter = counter.clone();
        thread::spawn(move || {
            for _ in 0..1000 {
                counter.inc();
            }
        })
    })
    .collect();

for handle in handles {
    handle.join().unwrap();
}

assert_eq!(counter.get(), 10000);
```

## 에러 처리

라이브러리는 포괄적인 에러 타입을 사용합니다:

```rust
pub enum MonitoringError {
    ConfigError(String),
    RegistrationError(String),
    MetricNotFound(String),
    InvalidMetricType { expected: String, found: String },
    InvalidValue(String),
    StorageError(String),
    ExportError(String),
    CollectionError(String),
    AlertError(String),
    AlreadyExists(String),
    NotInitialized,
    AlreadyInitialized,
    Other(String),
}
```

모든 에러는 `thiserror`를 통해 `std::error::Error`를 구현합니다.

## 대안과의 비교

| 기능 | rust_monitoring_system | prometheus | metrics |
|---------|----------------------|------------|---------|
| Atomic operations | ✅ | ✅ | ✅ |
| Multiple metric types | ✅ | ✅ | ✅ |
| Built-in collectors | ✅ | ❌ | ❌ |
| Prometheus export | ✅ | ✅ | ⚠️ |
| System monitoring | ✅ | ❌ | ❌ |
| Custom labels | ✅ | ✅ | ✅ |

## 의존성

- **thiserror**: 인체공학적 에러 처리
- **parking_lot**: 고성능 동기화 프리미티브
- **serde**: 직렬화 프레임워크
- **serde_json**: JSON 직렬화
- **chrono**: 날짜 및 시간 처리
- **crossbeam**: 동시성 프로그래밍 유틸리티

## 라이선스

이 프로젝트는 BSD 3-Clause License로 라이선스됩니다. 자세한 내용은 LICENSE 파일을 참조하세요.

## 기여

기여를 환영합니다! 자유롭게 이슈를 제출하거나 pull request를 보내주세요.

## 저자

Monitoring System Team

## 참고

- [C++ monitoring_system](https://github.com/kcenon/monitoring_system) - 원본 C++ 구현
- [rust_container_system](../rust_container_system) - Companion Rust container library
- [rust_database_system](../rust_database_system) - Companion Rust database library
- [rust_logger_system](../rust_logger_system) - Companion Rust logger library
- [rust_thread_system](../rust_thread_system) - Companion Rust thread pool library
