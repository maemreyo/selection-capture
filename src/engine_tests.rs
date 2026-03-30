use super::*;
use crate::cache::{adaptive_history_test_lock, reset_adaptive_history_for_tests};
use crate::profile::{AppProfile, AppProfileUpdate};
use crate::traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
use crate::types::{
    ActiveApp, CaptureOptions, CaptureStatus, CleanupStatus, PlatformAttemptResult, WouldBlock,
};
#[cfg(not(target_os = "macos"))]
use crate::types::{CGPoint, CGRect, CGSize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct NeverCancel;
impl CancelSignal for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct NoAdapters;
impl AppAdapter for NoAdapters {
    fn matches(&self, _app: &ActiveApp) -> bool {
        false
    }
    fn strategy_override(&self, _app: &ActiveApp) -> Option<Vec<CaptureMethod>> {
        None
    }
    fn hint_override(&self, _context: &CaptureFailureContext) -> Option<UserHint> {
        None
    }
}

struct StubStore;
impl AppProfileStore for StubStore {
    fn load(&self, app: &ActiveApp) -> AppProfile {
        AppProfile::unknown(app.bundle_id.clone())
    }
    fn merge_update(&self, _app: &ActiveApp, _update: AppProfileUpdate) {}
}

struct StubPlatform {
    app: Option<ActiveApp>,
    responses: Arc<Mutex<Vec<PlatformAttemptResult>>>,
    cleanup: CleanupStatus,
}

impl CapturePlatform for StubPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        self.app.clone()
    }

    fn attempt(&self, _method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
        let mut guard = self.responses.lock().unwrap();
        if guard.is_empty() {
            PlatformAttemptResult::Unavailable
        } else {
            guard.remove(0)
        }
    }

    fn cleanup(&self) -> CleanupStatus {
        self.cleanup
    }
}

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    let guard = adaptive_history_test_lock()
        .lock()
        .expect("test lock poisoned");
    reset_adaptive_history_for_tests();
    guard
}

#[cfg(target_os = "macos")]
fn test_rect() -> crate::types::CGRect {
    crate::types::CGRect::new(
        &crate::types::CGPoint::new(10.0, 20.0),
        &crate::types::CGSize::new(300.0, 200.0),
    )
}

#[cfg(not(target_os = "macos"))]
fn test_rect() -> crate::types::CGRect {
    CGRect {
        origin: CGPoint { x: 10.0, y: 20.0 },
        size: CGSize {
            width: 300.0,
            height: 200.0,
        },
    }
}

#[test]
fn collect_trace_true_always_returns_trace() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            "hello".into(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        collect_trace: true,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.range_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.clipboard = vec![Duration::from_millis(0)];
    let out = capture(&platform, &store, &cancel, &[&adapter], &options);
    match out {
        CaptureOutcome::Success(success) => assert!(success.trace.is_some()),
        CaptureOutcome::Failure(_) => panic!("expected success"),
    }
}

#[test]
fn success_keeps_initial_window_frame_snapshot_even_if_focus_changes() {
    struct SnapshotPlatform {
        reads: AtomicUsize,
    }

    impl CapturePlatform for SnapshotPlatform {
        fn active_app(&self) -> Option<ActiveApp> {
            None
        }

        fn attempt(
            &self,
            _method: CaptureMethod,
            _app: Option<&ActiveApp>,
        ) -> PlatformAttemptResult {
            PlatformAttemptResult::Success("hello".to_string())
        }

        fn focused_window_frame(&self) -> Option<crate::types::CGRect> {
            if self.reads.fetch_add(1, Ordering::SeqCst) == 0 {
                Some(test_rect())
            } else {
                None
            }
        }

        fn cleanup(&self) -> CleanupStatus {
            CleanupStatus::Clean
        }
    }

    let _guard = test_guard();
    let platform = SnapshotPlatform {
        reads: AtomicUsize::new(0),
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        collect_trace: false,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::ZERO];
    options.retry_policy.range_accessibility = vec![Duration::ZERO];
    options.retry_policy.clipboard = vec![Duration::ZERO];

    let out = capture(&platform, &store, &cancel, &[&adapter], &options);
    match out {
        CaptureOutcome::Success(success) => {
            assert!(success.focused_window_frame.is_some());
        }
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn capture_trace_records_method_timing_and_total_elapsed() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            "hello".into(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        collect_trace: true,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::ZERO];
    options.retry_policy.range_accessibility = vec![Duration::ZERO];
    options.retry_policy.clipboard = vec![Duration::ZERO];
    options.overall_timeout = Duration::from_secs(1);

    let out = capture(&platform, &store, &cancel, &[&adapter], &options);
    match out {
        CaptureOutcome::Success(success) => {
            let trace = success.trace.expect("trace");
            assert!(trace.events.iter().any(|event| matches!(
                event,
                TraceEvent::MethodFinished {
                    method: CaptureMethod::AccessibilityPrimary,
                    ..
                }
            )));
            assert!(trace
                .events
                .iter()
                .any(|event| matches!(event, TraceEvent::CleanupFinished(CleanupStatus::Clean))));
            assert!(trace.total_elapsed <= options.overall_timeout);
        }
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn skips_retry_when_budget_is_too_small() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::EmptySelection])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        collect_trace: true,
        ..CaptureOptions::default()
    };
    options.overall_timeout = Duration::from_millis(10);
    let out = capture(&platform, &store, &cancel, &[&adapter], &options);
    match out {
        CaptureOutcome::Failure(failure) => {
            assert_eq!(failure.status, CaptureStatus::EmptySelection);
            let trace = failure.trace.expect("trace");
            assert!(trace
                .events
                .iter()
                .any(|e| matches!(e, TraceEvent::RetryWaitSkipped { .. })));
        }
        CaptureOutcome::Success(_) => panic!("expected failure"),
    }
}

#[test]
fn falls_through_to_clipboard_after_ax_returns_empty_or_unavailable() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![
            PlatformAttemptResult::EmptySelection,
            PlatformAttemptResult::Unavailable,
            PlatformAttemptResult::Success("selected from clipboard".into()),
        ])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        collect_trace: true,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.range_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.clipboard = vec![Duration::from_millis(0)];
    let out = capture(&platform, &store, &cancel, &[&adapter], &options);
    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.text, "selected from clipboard");
            assert_eq!(success.method, CaptureMethod::ClipboardBorrow);
            let trace = success.trace.expect("trace");
            assert!(trace.events.iter().any(|event| matches!(
                event,
                TraceEvent::MethodReturnedEmpty(CaptureMethod::AccessibilityPrimary)
            )));
            assert!(trace.events.iter().any(|event| matches!(
                event,
                TraceEvent::MethodSucceeded(CaptureMethod::ClipboardBorrow)
            )));
        }
        CaptureOutcome::Failure(_) => panic!("expected success"),
    }
}

#[test]
fn probes_other_methods_before_waiting_for_retry_delay() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test.interleave".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![
            PlatformAttemptResult::EmptySelection,
            PlatformAttemptResult::Success("range hit".into()),
        ])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        collect_trace: true,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::ZERO, Duration::from_millis(60)];
    options.retry_policy.range_accessibility = vec![Duration::ZERO];
    options.retry_policy.clipboard = vec![Duration::from_millis(120)];
    options.interleave_method_retries = true;

    let out = capture(&platform, &store, &cancel, &[&adapter], &options);
    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.method, CaptureMethod::AccessibilityRange);
            assert_eq!(success.text, "range hit");
            let trace = success.trace.expect("trace");
            let started_methods: Vec<_> = trace
                .events
                .iter()
                .filter_map(|event| match event {
                    TraceEvent::MethodStarted(method) => Some(*method),
                    _ => None,
                })
                .collect();
            assert_eq!(
                started_methods,
                vec![
                    CaptureMethod::AccessibilityPrimary,
                    CaptureMethod::AccessibilityRange
                ]
            );
        }
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn capture_can_disable_interleaving_and_keep_sequential_retry_order() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test.sequential".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![
            PlatformAttemptResult::EmptySelection,
            PlatformAttemptResult::Success("primary retry hit".into()),
            PlatformAttemptResult::Success("range hit".into()),
        ])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        collect_trace: true,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::ZERO, Duration::from_millis(60)];
    options.retry_policy.range_accessibility = vec![Duration::ZERO];
    options.retry_policy.clipboard = vec![Duration::from_millis(120)];
    options.interleave_method_retries = false;

    let out = capture(&platform, &store, &cancel, &[&adapter], &options);
    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.method, CaptureMethod::AccessibilityPrimary);
            assert_eq!(success.text, "primary retry hit");
            let trace = success.trace.expect("trace");
            let started_methods: Vec<_> = trace
                .events
                .iter()
                .filter_map(|event| match event {
                    TraceEvent::MethodStarted(method) => Some(*method),
                    _ => None,
                })
                .collect();
            assert_eq!(
                started_methods,
                vec![
                    CaptureMethod::AccessibilityPrimary,
                    CaptureMethod::AccessibilityPrimary
                ]
            );
        }
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn try_capture_returns_would_block_when_later_method_requires_delay() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![
            PlatformAttemptResult::Unavailable,
            PlatformAttemptResult::Unavailable,
            PlatformAttemptResult::Success("clipboard".into()),
        ])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions::default();
    options.retry_policy.primary_accessibility = vec![Duration::ZERO];
    options.retry_policy.range_accessibility = vec![Duration::ZERO];
    options.retry_policy.clipboard = vec![Duration::from_millis(120)];

    let out = try_capture(&platform, &store, &cancel, &[&adapter], &options);
    assert_eq!(out, Err(WouldBlock));
}

#[test]
fn try_capture_succeeds_immediately_when_primary_method_succeeds() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            "hello".into(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions::default();
    options.retry_policy.primary_accessibility = vec![Duration::ZERO];
    options.retry_policy.range_accessibility = vec![Duration::ZERO];
    options.retry_policy.clipboard = vec![Duration::from_millis(120)];

    let out =
        try_capture(&platform, &store, &cancel, &[&adapter], &options).expect("should not block");
    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.method, CaptureMethod::AccessibilityPrimary);
            assert_eq!(success.text, "hello");
        }
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn try_capture_trace_records_method_timing_and_total_elapsed() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            "hello".into(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        collect_trace: true,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::ZERO];
    options.retry_policy.range_accessibility = vec![Duration::ZERO];
    options.retry_policy.clipboard = vec![Duration::from_millis(120)];
    options.overall_timeout = Duration::from_secs(1);

    let out =
        try_capture(&platform, &store, &cancel, &[&adapter], &options).expect("should not block");
    match out {
        CaptureOutcome::Success(success) => {
            let trace = success.trace.expect("trace");
            assert!(trace.events.iter().any(|event| matches!(
                event,
                TraceEvent::MethodFinished {
                    method: CaptureMethod::AccessibilityPrimary,
                    ..
                }
            )));
            assert!(trace
                .events
                .iter()
                .any(|event| matches!(event, TraceEvent::CleanupFinished(CleanupStatus::Clean))));
            assert!(trace.total_elapsed <= options.overall_timeout);
        }
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn try_capture_returns_failure_when_all_immediate_methods_are_exhausted() {
    let _guard = test_guard();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.test".into(),
            name: "Test".into(),
        }),
        responses: Arc::new(Mutex::new(vec![
            PlatformAttemptResult::PermissionDenied,
            PlatformAttemptResult::Unavailable,
        ])),
        cleanup: CleanupStatus::Clean,
    };
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = CaptureOptions {
        allow_clipboard_borrow: false,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::ZERO];
    options.retry_policy.range_accessibility = vec![Duration::ZERO];

    let out = try_capture(&platform, &store, &cancel, &[&adapter], &options)
        .expect("all paths are immediate");
    match out {
        CaptureOutcome::Failure(failure) => {
            assert_eq!(failure.status, CaptureStatus::PermissionDenied);
        }
        other => panic!("expected failure, got {other:?}"),
    }
}
