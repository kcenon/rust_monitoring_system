# Rust Monitoring System - 개선 계획

> **Languages**: [English](./IMPROVEMENTS.md) | 한국어

## 개요

이 문서는 Rust Monitoring System의 제안된 개선사항과 기능 향상을 설명합니다. 현재 구현은 Arc의 좋은 사용과 명확한 관심사 분리로 견고하지만, 기능, 성능 및 프로덕션 준비성을 향상시킬 기회가 있습니다.

## 기능 향상 기회

### 1. 메트릭 집계 및 다운샘플링

**기회**: 메트릭 집계 윈도우와 다운샘플링 지원을 추가하여 장기 메트릭의 스토리지를 줄이고 쿼리 성능을 개선합니다.

**현재 상태**:
```rust
// 메트릭이 전체 해상도로 수집됨
// 내장된 집계나 다운샘플링 없음
pub fn collect(&self) -> Vec<Metric> {
    // 모든 원시 메트릭 반환
}
```

**제안된 향상**:

```rust
// TODO: 메트릭 집계 및 다운샘플링 지원 추가

#[derive(Debug, Clone)]
pub enum AggregationWindow {
    Seconds(u64),
    Minutes(u64),
    Hours(u64),
    Days(u64),
}

#[derive(Debug, Clone)]
pub enum AggregationFunction {
    Sum,
    Average,
    Min,
    Max,
    Count,
    P50,    // 50th percentile
    P95,    // 95th percentile
    P99,    // 99th percentile
}

pub struct AggregationConfig {
    pub window: AggregationWindow,
    pub functions: Vec<AggregationFunction>,
    pub keep_raw: bool,  // 집계와 함께 원시 메트릭 유지 여부
}

pub struct MetricAggregator {
    config: AggregationConfig,
    buffers: HashMap<String, MetricBuffer>,
}

impl MetricAggregator {
    pub fn aggregate(&mut self, metrics: Vec<Metric>) -> Vec<AggregatedMetric> {
        let now = SystemTime::now();
        let window_start = self.calculate_window_start(now);

        for metric in metrics {
            let buffer = self.buffers
                .entry(metric.name.clone())
                .or_insert_with(|| MetricBuffer::new(window_start));

            buffer.push(metric);
        }

        // 완료된 윈도우 플러시
        self.flush_completed_windows(now)
    }

    fn apply_function(&self, func: &AggregationFunction, metrics: &[Metric]) -> f64 {
        match func {
            AggregationFunction::Sum => {
                metrics.iter().map(|m| m.value()).sum()
            }
            AggregationFunction::Average => {
                let sum: f64 = metrics.iter().map(|m| m.value()).sum();
                sum / metrics.len() as f64
            }
            AggregationFunction::P95 => {
                self.calculate_percentile(metrics, 0.95)
            }
            // ... 다른 함수들
        }
    }
}

// 사용법:
let aggregator = MetricAggregator::new(AggregationConfig {
    window: AggregationWindow::Minutes(5),
    functions: vec![
        AggregationFunction::Average,
        AggregationFunction::P95,
        AggregationFunction::Max,
    ],
    keep_raw: false,
});

let aggregated = aggregator.aggregate(monitor.collect());
```

**이점**:
- 장기 메트릭의 스토리지 요구사항 감소
- 과거 데이터에 대한 더 빠른 쿼리
- 백분위수 및 통계의 자동 계산
- 설정 가능한 보존 정책

**우선순위**: 중간
**예상 작업량**: 대 (2-3주)

### 2. 알림 및 임계값 모니터링

**기회**: 메트릭이 임계값을 초과하거나 비정상적인 동작을 보일 때 운영자에게 알리는 내장 알림 기능을 추가합니다.

**제안된 향상**:

```rust
// TODO: 알림 및 임계값 모니터링 추가

#[derive(Debug, Clone)]
pub enum Threshold {
    Above(f64),
    Below(f64),
    Range { min: f64, max: f64 },
    RateOfChange { max_delta: f64, window: Duration },
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

pub struct AlertRule {
    pub name: String,
    pub metric_pattern: String,  // 메트릭 이름에 대한 Glob 패턴
    pub threshold: Threshold,
    pub severity: AlertSeverity,
    pub duration: Option<Duration>,  // 지속적인 위반 지속시간
    pub cooldown: Duration,          // 반복 알림 사이 시간
}

pub struct Alert {
    pub rule: AlertRule,
    pub metric_name: String,
    pub current_value: f64,
    pub timestamp: SystemTime,
    pub message: String,
}

pub struct AlertManager {
    rules: Vec<AlertRule>,
    alert_states: HashMap<String, AlertState>,
    handlers: Vec<Box<dyn AlertHandler>>,
}

pub trait AlertHandler: Send + Sync {
    fn handle(&self, alert: &Alert);
}

impl AlertManager {
    pub fn add_rule(&mut self, rule: AlertRule) {
        self.rules.push(rule);
    }

    pub fn add_handler(&mut self, handler: Box<dyn AlertHandler>) {
        self.handlers.push(handler);
    }

    pub fn check_metrics(&mut self, metrics: &[Metric]) {
        let now = SystemTime::now();

        for rule in &self.rules {
            for metric in metrics {
                if self.matches_pattern(&metric.name, &rule.metric_pattern) {
                    self.check_metric(rule, metric, now);
                }
            }
        }
    }
}

// 알림 핸들러 예제
pub struct LogAlertHandler;

impl AlertHandler for LogAlertHandler {
    fn handle(&self, alert: &Alert) {
        log::warn!(
            "[{:?}] {}",
            alert.rule.severity,
            alert.message
        );
    }
}

pub struct WebhookAlertHandler {
    url: String,
}

impl AlertHandler for WebhookAlertHandler {
    fn handle(&self, alert: &Alert) {
        // 웹훅으로 알림 POST
        self.post_webhook(alert);
    }
}

// 사용법:
let mut alert_mgr = AlertManager::new();

alert_mgr.add_rule(AlertRule {
    name: "high_cpu".to_string(),
    metric_pattern: "system_cpu_usage_percent".to_string(),
    threshold: Threshold::Above(80.0),
    severity: AlertSeverity::Warning,
    duration: Some(Duration::from_secs(60)),  // 1분간 지속
    cooldown: Duration::from_secs(300),       // 최대 5분마다 알림
});

alert_mgr.add_handler(Box::new(LogAlertHandler));
alert_mgr.add_handler(Box::new(WebhookAlertHandler {
    url: "https://alerts.example.com/webhook".to_string(),
}));

// 주기적으로 메트릭 확인
let metrics = monitor.collect();
alert_mgr.check_metrics(&metrics);
```

**이점**:
- 사전 문제 감지
- 자동화된 운영자 알림
- 코드 변경 없이 설정 가능한 알림 규칙
- 다중 알림 채널 (이메일, Slack, PagerDuty 등)

**우선순위**: 높음
**예상 작업량**: 대 (2-3주)

### 3. 메트릭 영속성 및 과거 쿼리

**기회**: 과거 분석 및 그래프를 위해 메트릭을 저장하는 영속성 레이어를 추가합니다.

**제안된 향상**:

```rust
// TODO: 메트릭 영속성 및 과거 쿼리 지원 추가

pub trait MetricStorage: Send + Sync {
    fn write(&mut self, metrics: &[Metric]) -> Result<(), StorageError>;
    fn query(&self, query: &MetricQuery) -> Result<Vec<Metric>, StorageError>;
}

#[derive(Debug, Clone)]
pub struct MetricQuery {
    pub metric_name: String,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub labels: HashMap<String, String>,  // 레이블로 필터
    pub aggregation: Option<AggregationFunction>,
}

// 시계열 데이터베이스 백엔드
pub struct TimeSeriesStorage {
    db: Connection,  // SQLite, PostgreSQL, TimescaleDB 등
}

impl MetricStorage for TimeSeriesStorage {
    fn write(&mut self, metrics: &[Metric]) -> Result<(), StorageError> {
        let tx = self.db.transaction()?;

        for metric in metrics {
            tx.execute(
                "INSERT INTO metrics (name, value, timestamp, labels) VALUES (?1, ?2, ?3, ?4)",
                params![
                    metric.name,
                    metric.value(),
                    metric.timestamp,
                    serde_json::to_string(&metric.labels)?,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn query(&self, query: &MetricQuery) -> Result<Vec<Metric>, StorageError> {
        // SQL 쿼리 실행
        // ...
    }
}

// Monitor 통합
impl Monitor {
    pub fn with_storage(storage: Box<dyn MetricStorage>) -> Self {
        // ... 초기화
    }

    pub fn collect_and_store(&mut self) -> Result<(), StorageError> {
        let metrics = self.collect();
        self.storage.write(&metrics)?;
        Ok(())
    }

    pub fn query_historical(&self, query: MetricQuery) -> Result<Vec<Metric>, StorageError> {
        self.storage.query(&query)
    }
}

// 사용법:
let storage = Box::new(TimeSeriesStorage::new("metrics.db")?);
let mut monitor = Monitor::with_storage(storage);

// 수집 및 저장
monitor.collect_and_store()?;

// 과거 데이터 쿼리
let query = MetricQuery {
    metric_name: "system_cpu_usage_percent".to_string(),
    start_time: SystemTime::now() - Duration::from_secs(3600),  // 지난 시간
    end_time: SystemTime::now(),
    labels: HashMap::new(),
    aggregation: Some(AggregationFunction::Average),
};

let historical_metrics = monitor.query_historical(query)?;
```

**이점**:
- 장기 메트릭 보존
- 과거 추세 분석
- 용량 계획 지원
- 사고 회고

**우선순위**: 중간
**예상 작업량**: 대 (2-3주)

### 4. 메트릭 카디널리티 보호

**기회**: 무제한 레이블 값으로 인한 메트릭 폭발을 방지합니다.

**제안된 향상**:

```rust
// TODO: 메트릭 폭발을 방지하기 위한 카디널리티 제한 추가

pub struct CardinalityConfig {
    pub max_metrics: usize,
    pub max_label_values: usize,
    pub warn_threshold: f64,  // 최대값의 %에서 경고
}

pub struct CardinalityTracker {
    config: CardinalityConfig,
    metric_counts: HashMap<String, HashSet<LabelSet>>,
}

impl CardinalityTracker {
    pub fn check_cardinality(
        &mut self,
        metric_name: &str,
        labels: &HashMap<String, String>,
    ) -> Result<(), CardinalityError> {
        let label_set = LabelSet::from(labels);

        let metric_variants = self.metric_counts
            .entry(metric_name.to_string())
            .or_insert_with(HashSet::new);

        // 이 변형 추가가 제한을 초과하는지 확인
        if !metric_variants.contains(&label_set) {
            if metric_variants.len() >= self.config.max_label_values {
                return Err(CardinalityError::TooManyVariants {
                    metric_name: metric_name.to_string(),
                    current: metric_variants.len(),
                    max: self.config.max_label_values,
                });
            }

            // 제한에 근접하면 경고
            let usage_percent = metric_variants.len() as f64
                / self.config.max_label_values as f64;

            if usage_percent >= self.config.warn_threshold {
                log::warn!(
                    "Metric {} cardinality at {:.1}% ({}/{})",
                    metric_name,
                    usage_percent * 100.0,
                    metric_variants.len(),
                    self.config.max_label_values
                );
            }

            metric_variants.insert(label_set);
        }

        Ok(())
    }

    pub fn report(&self) -> CardinalityReport {
        // 카디널리티 보고서 생성
        // ...
    }
}

// Monitor와 통합
impl Monitor {
    pub fn counter_checked(
        &self,
        name: &str,
        help: &str,
        labels: HashMap<String, String>,
    ) -> Result<Counter, CardinalityError> {
        self.cardinality_tracker.check_cardinality(name, &labels)?;
        Ok(self.counter(name, help, labels))
    }
}
```

**이점**:
- 메트릭 폭발로 인한 메모리 고갈 방지
- 카디널리티 문제의 조기 경고
- 메트릭 레이블링 best practice 강제
- 카디널리티 사용량 가시성

**우선순위**: 높음
**예상 작업량**: 중간 (1주)

## 추가 향상사항

자세한 내용은 [영문 버전](./IMPROVEMENTS.md)의 추가 향상사항 섹션을 참조하세요:

- Push Gateway 지원
- OpenTelemetry 통합

## 테스트 요구사항

### 필요한 새 테스트:

1. **집계 테스트**:
   ```rust
   #[test]
   fn test_metric_aggregation() {
       let mut aggregator = MetricAggregator::new(AggregationConfig {
           window: AggregationWindow::Seconds(60),
           functions: vec![AggregationFunction::Average, AggregationFunction::P95],
           keep_raw: false,
       });

       // 시간에 따른 메트릭 생성
       let metrics = generate_test_metrics(100);
       let aggregated = aggregator.aggregate(metrics);

       assert!(!aggregated.is_empty());
       assert!(aggregated.iter().any(|m| m.name.contains("Average")));
       assert!(aggregated.iter().any(|m| m.name.contains("P95")));
   }
   ```

2. **알림 테스트**:
   ```rust
   #[test]
   fn test_alert_threshold() {
       let mut alert_mgr = AlertManager::new();

       alert_mgr.add_rule(AlertRule {
           name: "test_alert".to_string(),
           metric_pattern: "test_metric".to_string(),
           threshold: Threshold::Above(100.0),
           severity: AlertSeverity::Warning,
           duration: None,
           cooldown: Duration::from_secs(0),
       });

       // 임계값 이하 메트릭 - 알림 없음
       alert_mgr.check_metrics(&[Metric::new("test_metric", 50.0)]);

       // 임계값 이상 메트릭 - 알림
       alert_mgr.check_metrics(&[Metric::new("test_metric", 150.0)]);
   }
   ```

3. **카디널리티 테스트**:
   ```rust
   #[test]
   fn test_cardinality_limits() {
       let mut tracker = CardinalityTracker::new(CardinalityConfig {
           max_metrics: 100,
           max_label_values: 10,
           warn_threshold: 0.8,
       });

       // 첫 10개 변형에 대해 성공해야 함
       for i in 0..10 {
           let mut labels = HashMap::new();
           labels.insert("id".to_string(), i.to_string());
           assert!(tracker.check_cardinality("test", &labels).is_ok());
       }

       // 11번째 변형에 대해 실패해야 함
       let mut labels = HashMap::new();
       labels.insert("id".to_string(), "11".to_string());
       assert!(tracker.check_cardinality("test", &labels).is_err());
   }
   ```

## 구현 로드맵

### 1단계: 핵심 향상 (스프린트 1-2)
- [ ] 카디널리티 보호 구현
- [ ] 알림 프레임워크 추가
- [ ] 알림 핸들러 구현 생성
- [ ] 포괄적인 테스트 추가

### 2단계: 집계 및 스토리지 (스프린트 3-4)
- [ ] 메트릭 집계 구현
- [ ] 영속성 레이어 추가
- [ ] 과거 쿼리 지원
- [ ] 다운샘플링 추가

### 3단계: 통합 및 내보내기 (스프린트 5)
- [ ] Push Gateway 지원 추가
- [ ] OpenTelemetry exporter 구현
- [ ] 통합 예제 생성
- [ ] 문서 업데이트

## 참고자료

- 코드 분석: Monitoring System Review 2025-10-16
- Prometheus best practices: https://prometheus.io/docs/practices/naming/
- OpenTelemetry specification: https://opentelemetry.io/docs/specs/otel/

---

*개선 계획 버전 1.0*
*최종 업데이트: 2025-10-17*
