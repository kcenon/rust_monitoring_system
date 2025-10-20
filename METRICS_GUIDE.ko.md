# Rust Monitoring System - 메트릭 가이드

> **Languages**: [English](./METRICS_GUIDE.md) | 한국어

## 개요

이 가이드는 Rust Monitoring System의 모든 측면을 다룹니다. 메트릭 타입, 사용 패턴, best practice, 모니터링 플랫폼과의 통합을 포함합니다.

## 목차

1. [메트릭 타입](#메트릭-타입)
2. [메트릭 생성](#메트릭-생성)
3. [레이블과 차원](#레이블과-차원)
4. [수집 전략](#수집-전략)
5. [내보내기 형식](#내보내기-형식)
6. [시스템 메트릭](#시스템-메트릭)
7. [Best Practices](#best-practices)
8. [통합 예제](#통합-예제)

## 메트릭 타입

### Counter (카운터)

누적 메트릭으로 증가만 합니다 (또는 0으로 리셋).

**사용 사례:** 요청 수, 에러 수, 처리된 바이트, 트리거된 이벤트

**예제:**
```rust
let counter = monitor.counter(
    "http_requests_total",
    "수신한 총 HTTP 요청",
    HashMap::new()
);

counter.inc();           // 1 증가
counter.inc_by(5);       // 5 증가
let value = counter.get(); // 현재 값 읽기
```

### Gauge (게이지)

증가 또는 감소할 수 있는 메트릭.

**사용 사례:** 현재 온도, 메모리 사용량, 활성 연결, 큐 크기

**예제:**
```rust
let gauge = monitor.gauge(
    "active_connections",
    "현재 활성 연결 수",
    HashMap::new()
);

gauge.set(42);    // 특정 값으로 설정
gauge.inc();      // 1 증가
gauge.dec();      // 1 감소
```

### Histogram (히스토그램)

관찰값을 샘플링하고 설정 가능한 버킷에 카운트.

**사용 사례:** 요청 지속시간, 응답 크기, 쿼리 실행 시간

**예제:**
```rust
let histogram = monitor.histogram(
    "http_request_duration_seconds",
    "HTTP 요청 지속시간 (초)",
    vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0], // 버킷
    HashMap::new()
);

histogram.observe(0.234);  // 관찰값 기록
```

### Summary (요약)

관찰값을 샘플링하고 슬라이딩 시간 창에서 quantile 계산.

**사용 사례:** 요청 지연시간 (버킷 알 수 없을 때), 메시지 크기

### Timer (타이머)

지속시간 측정을 위한 특수 histogram.

**예제:**
```rust
let timer = monitor.timer(
    "operation_duration",
    "작업 지속시간 (초)",
    HashMap::new()
);

let start = std::time::Instant::now();
// ... 작업 수행 ...
timer.observe_duration(start.elapsed());
```

## 메트릭 생성

### 기본 메트릭 생성

```rust
use rust_monitoring_system::prelude::*;
use std::collections::HashMap;

let monitor = Monitor::new();
monitor.start()?;

// 레이블 없는 카운터
let requests = monitor.counter(
    "http_requests_total",
    "총 HTTP 요청",
    HashMap::new()
);

// 레이블 없는 게이지
let memory = monitor.gauge(
    "memory_usage_bytes",
    "현재 메모리 사용량",
    HashMap::new()
);
```

### 레이블이 있는 메트릭

```rust
// 레이블로 메트릭 생성
let mut labels = HashMap::new();
labels.insert("method".to_string(), "GET".to_string());
labels.insert("endpoint".to_string(), "/api/users".to_string());
labels.insert("status".to_string(), "200".to_string());

let requests = monitor.counter(
    "http_requests_total",
    "총 HTTP 요청",
    labels
);
```

### 동적 레이블 생성

```rust
fn record_request(monitor: &Monitor, method: &str, endpoint: &str, status: u16) {
    let mut labels = HashMap::new();
    labels.insert("method".to_string(), method.to_string());
    labels.insert("endpoint".to_string(), endpoint.to_string());
    labels.insert("status".to_string(), status.to_string());

    let counter = monitor.counter(
        "http_requests_total",
        "총 HTTP 요청",
        labels
    );

    counter.inc();
}
```

## 레이블과 차원

### 레이블 Best Practices

**좋은 레이블 이름:**
```rust
// ✓ 좋음: 설명적, 소문자, 밑줄로 구분
"method"
"endpoint"
"status_code"
"error_type"
"instance_id"
```

**나쁜 레이블 이름:**
```rust
// ✗ 나쁨: 불명확, 일관성 없는 대소문자
"m"
"Method"
"status-code"
"errorType"
```

### 레이블 카디널리티

**낮은 카디널리티 (좋음):**
```rust
// HTTP 메서드: ~9개 값 (GET, POST, PUT, DELETE 등)
labels.insert("method".to_string(), "GET".to_string());

// 상태 코드: ~60개 값 (200, 404, 500 등)
labels.insert("status".to_string(), "200".to_string());
```

**높은 카디널리티 (피하기):**
```rust
// ✗ 사용자 ID: 수백만 개 가능한 값
labels.insert("user_id".to_string(), user_id.to_string());

// ✗ 요청 ID: 무한한 가능한 값
labels.insert("request_id".to_string(), request_id.to_string());
```

**높은 카디널리티의 영향:**
- 메모리 사용량 증가
- 쿼리 성능 저하
- 스토리지 비용 증가
- 메트릭 폭발 가능성

## 수집 전략

### Pull 기반 수집

```rust
// 메트릭 엔드포인트 노출
let monitor = Arc::new(Monitor::new());
monitor.start()?;

// 주기적으로 수집 및 노출
loop {
    let metrics = monitor.collect();

    // Prometheus 형식으로 변환
    let exporter = PrometheusExporter::new();
    let output = exporter.export(&metrics)?;

    // HTTP 엔드포인트를 통해 제공
    // (예: /metrics에서)

    std::thread::sleep(Duration::from_secs(15));
}
```

### Push 기반 수집

```rust
use std::time::Duration;

let config = MonitorConfig::new("myapp")
    .with_interval(Duration::from_secs(10))
    .with_auto_collect(true);

let monitor = Monitor::with_config(config);
monitor.start()?;

// 10초마다 자동 수집
```

## 내보내기 형식

### Prometheus 텍스트 형식

```rust
use rust_monitoring_system::prelude::*;

let monitor = Monitor::new();
// ... 메트릭 생성 및 업데이트 ...

let metrics = monitor.collect();
let exporter = PrometheusExporter::new();
let output = exporter.export(&metrics)?;

println!("{}", output);
```

**출력 예제:**
```
# HELP http_requests_total 총 HTTP 요청
# TYPE http_requests_total counter
http_requests_total{method="GET",endpoint="/api/users"} 1543
http_requests_total{method="POST",endpoint="/api/users"} 234

# HELP active_connections 현재 활성 연결 수
# TYPE active_connections gauge
active_connections 42
```

## 시스템 메트릭

### SystemCollector 사용

```rust
use rust_monitoring_system::prelude::*;
use std::sync::Arc;

let monitor = Arc::new(Monitor::new());
monitor.start()?;

// 시스템 수집기 생성
let collector = SystemCollector::new(monitor.clone())?;

// 시스템 메트릭 수집
collector.collect()?;

// 이제 메트릭을 사용할 수 있습니다
let metrics = monitor.collect();
```

### 사용 가능한 시스템 메트릭

```rust
// CPU 메트릭
system_cpu_usage_percent        // 현재 CPU 사용률 (0-100)
system_cpu_cores                // CPU 코어 수

// 메모리 메트릭
system_memory_total_bytes       // 총 시스템 메모리
system_memory_usage_bytes       // 현재 메모리 사용량
system_memory_available_bytes   // 사용 가능한 메모리
system_memory_usage_percent     // 메모리 사용률

// 시스템 메트릭
system_uptime_seconds           // 모니터 가동 시간 (초)
```

## Best Practices

### 명명 규칙

```rust
// ✓ 좋은 메트릭 이름
"http_requests_total"           // Counter: _total 접미사
"active_connections"            // Gauge: 현재 상태
"request_duration_seconds"      // Histogram: _seconds 접미사
"response_size_bytes"           // 크기에 _bytes 접미사

// ✗ 나쁜 메트릭 이름
"requests"                      // 총계인지 현재인지 불명확
"latency"                       // 단위 누락
"RequestDuration"               // 잘못된 대소문자
"req_dur_s"                     // 너무 축약됨
```

### 단위 접미사

```rust
// 시간 지속시간
"_seconds"
"_milliseconds"
"_microseconds"

// 크기
"_bytes"
"_kilobytes"
"_megabytes"

// 백분율
"_percent"
"_ratio"      // 0-1 스케일

// 카운트
"_total"      // 누적 카운트 (counter)
// (접미사 없음)  // 현재 카운트 (gauge)
```

### 메트릭 생명주기

```rust
// 1. 애플리케이션 시작 시 모니터 생성
let monitor = Arc::new(Monitor::new());
monitor.start()?;

// 2. 메트릭을 일찍 생성 (캐시)
struct AppMetrics {
    requests: Counter,
    errors: Counter,
    latency: Histogram,
}

impl AppMetrics {
    fn new(monitor: &Monitor) -> Self {
        Self {
            requests: monitor.counter(
                "app_requests_total",
                "총 요청",
                HashMap::new()
            ),
            errors: monitor.counter(
                "app_errors_total",
                "총 에러",
                HashMap::new()
            ),
            latency: monitor.histogram(
                "app_request_duration_seconds",
                "요청 지속시간",
                vec![0.01, 0.1, 0.5, 1.0],
                HashMap::new()
            ),
        }
    }
}

// 3. 애플리케이션 생명주기 동안 메트릭 사용
let metrics = AppMetrics::new(&monitor);

// 4. 종료 시 모니터 중지
monitor.stop()?;
```

### 성능 고려사항

```rust
// ✓ 메트릭 인스턴스 캐시
struct Handler {
    requests_metric: Counter,  // 캐시됨
}

impl Handler {
    fn handle(&self) {
        self.requests_metric.inc();  // 빠름
    }
}

// ✗ 메트릭 재생성하지 않기
fn handle_request(monitor: &Monitor) {
    let counter = monitor.counter(  // 느림!
        "requests",
        "요청",
        HashMap::new()
    );
    counter.inc();
}
```

## 통합 예제

### 웹 서버 통합 (Axum)

```rust
use axum::{
    routing::get,
    Router,
    extract::State,
};
use rust_monitoring_system::prelude::*;
use std::sync::Arc;

async fn metrics_handler(
    State(monitor): State<Arc<Monitor>>
) -> String {
    let metrics = monitor.collect();
    let exporter = PrometheusExporter::new();
    exporter.export(&metrics).unwrap_or_default()
}

#[tokio::main]
async fn main() {
    let monitor = Arc::new(Monitor::new());
    monitor.start().unwrap();

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(monitor);

    axum::Server::bind(&"0.0.0.0:9090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

### 미들웨어 통합

```rust
async fn metrics_middleware(
    State(monitor): State<Arc<Monitor>>,
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status().as_u16();

    // 메트릭 기록
    let mut labels = HashMap::new();
    labels.insert("method".to_string(), method);
    labels.insert("endpoint".to_string(), path);
    labels.insert("status".to_string(), status.to_string());

    let counter = monitor.counter(
        "http_requests_total",
        "HTTP 요청",
        labels.clone()
    );
    counter.inc();

    let histogram = monitor.histogram(
        "http_request_duration_seconds",
        "HTTP 요청 지속시간",
        vec![0.001, 0.01, 0.1, 1.0],
        labels
    );
    histogram.observe(duration.as_secs_f64());

    response
}
```

---

*메트릭 가이드 버전 1.0*
*최종 업데이트: 2025-10-16*

더 자세한 내용은 [영문 버전](./METRICS_GUIDE.md)을 참조하세요.
