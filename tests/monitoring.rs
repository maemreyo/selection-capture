use selection_capture::{
    CaptureFailure, CaptureFailureContext, CaptureMethod, CaptureMetrics, CaptureMonitor,
    CaptureOutcome, CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus, FailureKind,
    MonitorPlatform, TraceEvent,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
struct StubMonitorPlatform {
    events: Arc<Mutex<VecDeque<Option<String>>>>,
}

impl StubMonitorPlatform {
    fn new(events: Vec<Option<&str>>) -> Self {
        Self {
            events: Arc::new(Mutex::new(
                events
                    .into_iter()
                    .map(|event| event.map(str::to_owned))
                    .collect(),
            )),
        }
    }
}

impl MonitorPlatform for StubMonitorPlatform {
    fn next_selection_change(&self) -> Option<String> {
        self.events.lock().unwrap().pop_front().flatten()
    }
}

#[test]
fn monitor_emits_events_in_backend_order() {
    let platform = StubMonitorPlatform::new(vec![Some("first"), Some("second"), None]);
    let monitor = CaptureMonitor::new(platform);

    assert_eq!(monitor.next_event(), Some("first".to_string()));
    assert_eq!(monitor.next_event(), Some("second".to_string()));
    assert_eq!(monitor.next_event(), None);
}

#[test]
fn monitor_run_processes_until_backend_returns_none() {
    let platform = StubMonitorPlatform::new(vec![Some("first"), Some("second"), None]);
    let monitor = CaptureMonitor::new(platform);
    let mut observed = Vec::new();

    let processed = monitor.run(|event| observed.push(event));

    assert_eq!(processed, 2);
    assert_eq!(observed, vec!["first".to_string(), "second".to_string()]);
}

#[test]
fn monitor_run_with_limit_stops_at_max_events() {
    let platform = StubMonitorPlatform::new(vec![Some("first"), Some("second"), Some("third")]);
    let monitor = CaptureMonitor::new(platform);
    let mut observed = Vec::new();

    let processed = monitor.run_with_limit(2, |event| observed.push(event));

    assert_eq!(processed, 2);
    assert_eq!(observed, vec!["first".to_string(), "second".to_string()]);
    assert_eq!(monitor.next_event(), Some("third".to_string()));
}

#[test]
fn monitor_collect_events_returns_bounded_batch() {
    let platform = StubMonitorPlatform::new(vec![Some("a"), Some("b"), Some("c")]);
    let monitor = CaptureMonitor::new(platform);

    let batch = monitor.collect_events(2);

    assert_eq!(batch, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(monitor.next_event(), Some("c".to_string()));
}

#[test]
fn capture_metrics_aggregates_latency_and_status_by_method() {
    let mut metrics = CaptureMetrics::default();

    let success_outcome = CaptureOutcome::Success(CaptureSuccess {
        text: "hello".to_string(),
        method: CaptureMethod::AccessibilityPrimary,
        trace: Some(CaptureTrace {
            events: vec![
                TraceEvent::MethodStarted(CaptureMethod::AccessibilityPrimary),
                TraceEvent::MethodFinished {
                    method: CaptureMethod::AccessibilityPrimary,
                    elapsed: Duration::from_millis(25),
                },
                TraceEvent::MethodSucceeded(CaptureMethod::AccessibilityPrimary),
                TraceEvent::CleanupFinished(CleanupStatus::Clean),
            ],
            cleanup_status: CleanupStatus::Clean,
            total_elapsed: Duration::from_millis(40),
        }),
    });

    let failure_outcome = CaptureOutcome::Failure(CaptureFailure {
        status: CaptureStatus::TimedOut,
        hint: None,
        trace: Some(CaptureTrace {
            events: vec![
                TraceEvent::MethodStarted(CaptureMethod::ClipboardBorrow),
                TraceEvent::MethodFinished {
                    method: CaptureMethod::ClipboardBorrow,
                    elapsed: Duration::from_millis(80),
                },
                TraceEvent::MethodFailed {
                    method: CaptureMethod::ClipboardBorrow,
                    kind: FailureKind::TimedOut,
                },
                TraceEvent::TimedOut,
                TraceEvent::CleanupFinished(CleanupStatus::Clean),
            ],
            cleanup_status: CleanupStatus::Clean,
            total_elapsed: Duration::from_millis(100),
        }),
        cleanup_failed: false,
        context: CaptureFailureContext {
            status: CaptureStatus::TimedOut,
            active_app: None,
            methods_tried: vec![CaptureMethod::ClipboardBorrow],
            last_method: Some(CaptureMethod::ClipboardBorrow),
        },
    });

    metrics.record_outcome(&success_outcome);
    metrics.record_outcome(&failure_outcome);

    assert_eq!(metrics.total_captures, 2);
    assert_eq!(metrics.successes, 1);
    assert_eq!(metrics.failures, 1);
    assert_eq!(metrics.timed_out, 1);
    assert_eq!(metrics.cancelled, 0);
    assert_eq!(metrics.total_latency, Duration::from_millis(140));
    assert_eq!(metrics.average_latency(), Duration::from_millis(70));
    assert_eq!(metrics.overall_success_rate(), 0.5);

    let primary = metrics
        .method_metrics(CaptureMethod::AccessibilityPrimary)
        .expect("primary method metrics");
    assert_eq!(primary.attempts, 1);
    assert_eq!(primary.successes, 1);
    assert_eq!(primary.failures, 0);
    assert_eq!(primary.empty_results, 0);
    assert_eq!(primary.total_latency, Duration::from_millis(25));

    let clipboard = metrics
        .method_metrics(CaptureMethod::ClipboardBorrow)
        .expect("clipboard method metrics");
    assert_eq!(clipboard.attempts, 1);
    assert_eq!(clipboard.successes, 0);
    assert_eq!(clipboard.failures, 1);
    assert_eq!(clipboard.empty_results, 0);
    assert_eq!(clipboard.total_latency, Duration::from_millis(80));
}
