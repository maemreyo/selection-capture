use selection_capture::{
    CancelSignal, CaptureFailure, CaptureFailureContext, CaptureMethod, CaptureMetrics,
    CaptureMonitor, CaptureOutcome, CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus,
    FailureKind, MonitorPlatform, MonitorSpamGuard, TraceEvent,
};
#[cfg(feature = "linux-alpha")]
use selection_capture::{LinuxMonitorBackend, LinuxSelectionMonitor, LinuxSelectionMonitorOptions};
#[cfg(feature = "windows-beta")]
use selection_capture::{
    WindowsMonitorBackend, WindowsSelectionMonitor, WindowsSelectionMonitorOptions,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
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

struct LoopCancelSignal {
    checks: AtomicUsize,
    cancel_after: usize,
}

impl LoopCancelSignal {
    fn new(cancel_after: usize) -> Self {
        Self {
            checks: AtomicUsize::new(0),
            cancel_after,
        }
    }
}

impl CancelSignal for LoopCancelSignal {
    fn is_cancelled(&self) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst) >= self.cancel_after
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
fn monitor_poll_until_continues_across_empty_polls() {
    let platform = StubMonitorPlatform::new(vec![None, Some("first"), None, Some("second"), None]);
    let monitor = CaptureMonitor::new(platform);
    let mut observed = Vec::new();
    let mut loops = 0usize;

    let processed = monitor.poll_until(
        Duration::ZERO,
        || {
            loops += 1;
            loops <= 5
        },
        |event| observed.push(event),
    );

    assert_eq!(processed, 2);
    assert_eq!(observed, vec!["first".to_string(), "second".to_string()]);
}

#[test]
fn monitor_poll_until_cancelled_stops_via_cancel_signal() {
    let platform = StubMonitorPlatform::new(vec![None, Some("first"), None, Some("second"), None]);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(6);
    let mut observed = Vec::new();

    let processed =
        monitor.poll_until_cancelled(Duration::ZERO, &cancel, |event| observed.push(event));

    assert_eq!(processed, 2);
    assert_eq!(observed, vec!["first".to_string(), "second".to_string()]);
}

#[test]
fn monitor_poll_until_cancelled_coalesced_suppresses_burst_events() {
    let platform = StubMonitorPlatform::new(vec![Some("a"), Some("b"), Some("c"), None, None]);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(6);
    let mut observed = Vec::new();

    let processed = monitor.poll_until_cancelled_coalesced(
        Duration::ZERO,
        Duration::from_secs(60),
        &cancel,
        |event| observed.push(event),
    );

    assert_eq!(processed, 1);
    assert_eq!(observed, vec!["a".to_string()]);
}

#[test]
fn monitor_poll_until_cancelled_guarded_suppresses_identical_bursts() {
    let mut events = vec![Some("stable")];
    events.extend((0..99).map(|_| Some("stable")));
    events.push(None);
    let platform = StubMonitorPlatform::new(events);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(130);
    let mut observed = Vec::new();
    let guard = MonitorSpamGuard::default();

    let processed =
        monitor.poll_until_cancelled_guarded(Duration::ZERO, &cancel, &guard, |event| {
            observed.push(event)
        });

    assert_eq!(processed, 1);
    assert_eq!(observed, vec!["stable".to_string()]);
}

#[test]
fn monitor_poll_until_cancelled_guarded_keeps_distinct_updates() {
    let platform = StubMonitorPlatform::new(vec![Some("a"), Some("b"), Some("c"), None]);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(8);
    let mut observed = Vec::new();
    let guard = MonitorSpamGuard::default();

    let processed =
        monitor.poll_until_cancelled_guarded(Duration::ZERO, &cancel, &guard, |event| {
            observed.push(event)
        });

    assert_eq!(processed, 3);
    assert_eq!(
        observed,
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

#[test]
fn monitor_poll_until_cancelled_guarded_can_enforce_global_emit_interval() {
    let platform = StubMonitorPlatform::new(vec![Some("a"), Some("b"), Some("c"), None]);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(8);
    let mut observed = Vec::new();
    let guard = MonitorSpamGuard {
        suppress_identical: false,
        min_emit_interval: Duration::from_secs(60),
        min_emit_interval_same_text: Duration::ZERO,
        normalize_whitespace: false,
        stable_polls_required: 1,
    };

    let processed =
        monitor.poll_until_cancelled_guarded(Duration::ZERO, &cancel, &guard, |event| {
            observed.push(event)
        });

    assert_eq!(processed, 1);
    assert_eq!(observed, vec!["a".to_string()]);
}

#[test]
fn monitor_poll_until_cancelled_guarded_can_normalize_whitespace_for_dedup() {
    let platform = StubMonitorPlatform::new(vec![
        Some("hello   world"),
        Some("hello world"),
        Some("hello   world  again"),
        None,
    ]);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(8);
    let mut observed = Vec::new();
    let guard = MonitorSpamGuard {
        normalize_whitespace: true,
        ..MonitorSpamGuard::default()
    };

    let processed =
        monitor.poll_until_cancelled_guarded(Duration::ZERO, &cancel, &guard, |event| {
            observed.push(event)
        });

    assert_eq!(processed, 2);
    assert_eq!(
        observed,
        vec![
            "hello   world".to_string(),
            "hello   world  again".to_string()
        ]
    );
}

#[test]
fn monitor_poll_until_cancelled_guarded_requires_stable_polls_before_emit() {
    let platform = StubMonitorPlatform::new(vec![Some("temp"), Some("final"), Some("final"), None]);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(8);
    let mut observed = Vec::new();
    let guard = MonitorSpamGuard {
        stable_polls_required: 2,
        suppress_identical: true,
        min_emit_interval: Duration::ZERO,
        min_emit_interval_same_text: Duration::ZERO,
        normalize_whitespace: false,
    };

    let processed =
        monitor.poll_until_cancelled_guarded(Duration::ZERO, &cancel, &guard, |event| {
            observed.push(event)
        });

    assert_eq!(processed, 1);
    assert_eq!(observed, vec!["final".to_string()]);
}

#[test]
fn monitor_poll_until_cancelled_guarded_ignores_flicker_when_not_stable() {
    let platform = StubMonitorPlatform::new(vec![Some("a"), Some("b"), Some("a"), None]);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(8);
    let mut observed = Vec::new();
    let guard = MonitorSpamGuard {
        stable_polls_required: 2,
        ..MonitorSpamGuard::default()
    };

    let processed =
        monitor.poll_until_cancelled_guarded(Duration::ZERO, &cancel, &guard, |event| {
            observed.push(event)
        });

    assert_eq!(processed, 0);
    assert!(observed.is_empty());
}

#[test]
fn monitor_poll_until_cancelled_guarded_with_stats_tracks_drop_reasons() {
    let platform = StubMonitorPlatform::new(vec![
        Some("a"), // unstable (requires 2)
        Some("a"), // emitted
        Some("a"), // duplicate
        Some("b"), // unstable
        Some("c"), // unstable
        Some("c"), // global interval drop (forced below)
        None,
    ]);
    let monitor = CaptureMonitor::new(platform);
    let cancel = LoopCancelSignal::new(12);
    let guard = MonitorSpamGuard {
        stable_polls_required: 2,
        suppress_identical: true,
        min_emit_interval: Duration::from_secs(60),
        min_emit_interval_same_text: Duration::ZERO,
        normalize_whitespace: false,
    };

    let stats = monitor.poll_until_cancelled_guarded_with_stats(
        Duration::ZERO,
        &cancel,
        &guard,
        |_event| {},
    );

    assert_eq!(stats.emitted, 1);
    assert_eq!(stats.dropped_duplicate, 0);
    assert_eq!(stats.dropped_global_interval, 2);
    assert_eq!(stats.dropped_same_text_interval, 0);
    assert_eq!(stats.dropped_unstable, 3);
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

#[cfg(feature = "windows-beta")]
#[test]
fn windows_monitor_native_pump_matches_manual_native_queue_path() {
    fn pump() -> Vec<String> {
        vec!["win a".to_string(), "win b".to_string()]
    }

    let from_pump = WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
        poll_interval: Duration::ZERO,
        backend: WindowsMonitorBackend::NativeEventPreferred,
        native_queue_capacity: 8,
        native_event_pump: Some(pump),
    });
    let from_manual = WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
        poll_interval: Duration::ZERO,
        backend: WindowsMonitorBackend::NativeEventPreferred,
        native_queue_capacity: 8,
        native_event_pump: None,
    });
    let _ =
        from_manual.enqueue_native_selection_events(vec!["win a".to_string(), "win b".to_string()]);

    let pump_events = CaptureMonitor::new(from_pump).collect_events(2);
    let manual_events = CaptureMonitor::new(from_manual).collect_events(2);

    assert_eq!(pump_events, manual_events);
    assert_eq!(pump_events, vec!["win a".to_string(), "win b".to_string()]);
}

#[cfg(feature = "linux-alpha")]
#[test]
fn linux_monitor_native_pump_matches_manual_native_queue_path() {
    fn pump() -> Vec<String> {
        vec!["linux a".to_string(), "linux b".to_string()]
    }

    let from_pump = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
        poll_interval: Duration::ZERO,
        backend: LinuxMonitorBackend::NativeEventPreferred,
        native_queue_capacity: 8,
        native_event_pump: Some(pump),
    });
    let from_manual = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
        poll_interval: Duration::ZERO,
        backend: LinuxMonitorBackend::NativeEventPreferred,
        native_queue_capacity: 8,
        native_event_pump: None,
    });
    let _ = from_manual
        .enqueue_native_selection_events(vec!["linux a".to_string(), "linux b".to_string()]);

    let pump_events = CaptureMonitor::new(from_pump).collect_events(2);
    let manual_events = CaptureMonitor::new(from_manual).collect_events(2);

    assert_eq!(pump_events, manual_events);
    assert_eq!(
        pump_events,
        vec!["linux a".to_string(), "linux b".to_string()]
    );
}
