use selection_capture::{
    capture, ActiveApp, AppAdapter, AppProfile, AppProfileStore, AppProfileUpdate, CancelSignal,
    CaptureFailureContext, CaptureMethod, CaptureOptions, CaptureOutcome, CapturePlatform,
    CaptureStatus, CleanupStatus, PlatformAttemptResult, TraceEvent, UserHint,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct NeverCancel;

impl CancelSignal for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct StubStore;

impl AppProfileStore for StubStore {
    fn load(&self, app: &ActiveApp) -> AppProfile {
        AppProfile::unknown(app.bundle_id.clone())
    }

    fn merge_update(&self, _app: &ActiveApp, _update: AppProfileUpdate) {}
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

#[derive(Clone)]
struct StubPlatform {
    app: Option<ActiveApp>,
    attempts: Arc<Mutex<Vec<CaptureMethod>>>,
    responses: Arc<Mutex<Vec<PlatformAttemptResult>>>,
    cleanup: CleanupStatus,
}

impl StubPlatform {
    fn new(responses: Vec<PlatformAttemptResult>) -> Self {
        Self {
            app: Some(ActiveApp {
                bundle_id: "org.example.shared".into(),
                name: "Shared Engine Test App".into(),
            }),
            attempts: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(responses)),
            cleanup: CleanupStatus::Clean,
        }
    }

    fn attempts(&self) -> Vec<CaptureMethod> {
        self.attempts.lock().unwrap().clone()
    }
}

impl CapturePlatform for StubPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        self.app.clone()
    }

    fn attempt(&self, method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
        self.attempts.lock().unwrap().push(method);

        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            PlatformAttemptResult::Unavailable
        } else {
            responses.remove(0)
        }
    }

    fn cleanup(&self) -> CleanupStatus {
        self.cleanup
    }
}

fn test_options() -> CaptureOptions {
    let mut options = CaptureOptions {
        collect_trace: true,
        ..CaptureOptions::default()
    };
    options.retry_policy.primary_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.range_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.clipboard = vec![Duration::from_millis(0)];
    options
}

#[test]
fn unsupported_methods_do_not_overwrite_prior_failure() {
    let platform = StubPlatform::new(vec![
        PlatformAttemptResult::PermissionDenied,
        PlatformAttemptResult::Unavailable,
    ]);
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = test_options();
    options.strategy_override = Some(vec![
        CaptureMethod::AccessibilityPrimary,
        CaptureMethod::SyntheticCopy,
    ]);

    let out = capture(&platform, &store, &cancel, &[&adapter], &options);

    match out {
        CaptureOutcome::Failure(failure) => {
            assert_eq!(failure.status, CaptureStatus::PermissionDenied);
            assert_eq!(
                failure.context.methods_tried,
                vec![
                    CaptureMethod::AccessibilityPrimary,
                    CaptureMethod::SyntheticCopy,
                ]
            );
            assert_eq!(
                platform.attempts(),
                vec![
                    CaptureMethod::AccessibilityPrimary,
                    CaptureMethod::SyntheticCopy,
                ]
            );

            let trace = failure.trace.expect("trace");
            assert!(trace.events.iter().any(|event| matches!(
                event,
                TraceEvent::MethodFailed {
                    method: CaptureMethod::AccessibilityPrimary,
                    kind: selection_capture::FailureKind::PermissionDenied,
                }
            )));
        }
        CaptureOutcome::Success(success) => panic!("expected failure, got {:?}", success.method),
    }
}

#[test]
fn fallback_order_follows_configured_method_sequence() {
    let platform = StubPlatform::new(vec![
        PlatformAttemptResult::Unavailable,
        PlatformAttemptResult::EmptySelection,
        PlatformAttemptResult::Success("clipboard fallback".into()),
    ]);
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = test_options();
    options.strategy_override = Some(vec![
        CaptureMethod::AccessibilityPrimary,
        CaptureMethod::AccessibilityRange,
        CaptureMethod::ClipboardBorrow,
    ]);

    let out = capture(&platform, &store, &cancel, &[&adapter], &options);

    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.method, CaptureMethod::ClipboardBorrow);
            assert_eq!(success.text, "clipboard fallback");
            assert_eq!(
                platform.attempts(),
                vec![
                    CaptureMethod::AccessibilityPrimary,
                    CaptureMethod::AccessibilityRange,
                    CaptureMethod::ClipboardBorrow,
                ]
            );

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
                    CaptureMethod::AccessibilityRange,
                    CaptureMethod::ClipboardBorrow,
                ]
            );
        }
        CaptureOutcome::Failure(failure) => panic!("expected success, got {:?}", failure.status),
    }
}
