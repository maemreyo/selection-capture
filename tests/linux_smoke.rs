#![cfg(feature = "linux-alpha")]

use selection_capture::{
    capture, ActiveApp, AppAdapter, AppProfile, AppProfileStore, AppProfileUpdate, CancelSignal,
    CaptureFailureContext, CaptureMethod, CaptureOptions, CaptureOutcome, CapturePlatform,
    CleanupStatus, PlatformAttemptResult, UserHint,
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
    responses: Arc<Mutex<Vec<PlatformAttemptResult>>>,
    cleanup: CleanupStatus,
}

impl StubPlatform {
    fn new(responses: Vec<PlatformAttemptResult>) -> Self {
        Self {
            app: Some(ActiveApp {
                bundle_id: "org.example.linux".into(),
                name: "Linux Test App".into(),
            }),
            responses: Arc::new(Mutex::new(responses)),
            cleanup: CleanupStatus::Clean,
        }
    }
}

impl CapturePlatform for StubPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        self.app.clone()
    }

    fn attempt(&self, _method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
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

fn smoke_options() -> CaptureOptions {
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
fn linux_alpha_falls_back_from_accessibility_to_clipboard() {
    let platform = StubPlatform::new(vec![
        PlatformAttemptResult::Unavailable,
        PlatformAttemptResult::EmptySelection,
        PlatformAttemptResult::Success("clipboard capture".into()),
    ]);
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;

    let out = capture(&platform, &store, &cancel, &[&adapter], &smoke_options());

    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.method, CaptureMethod::ClipboardBorrow);
            assert_eq!(success.text, "clipboard capture");
        }
        CaptureOutcome::Failure(failure) => {
            panic!("expected success, got {:?}", failure.status);
        }
    }
}

#[test]
fn linux_alpha_respects_strategy_override_order() {
    let platform = StubPlatform::new(vec![
        PlatformAttemptResult::Unavailable,
        PlatformAttemptResult::Success("synthetic copy capture".into()),
    ]);
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let mut options = smoke_options();
    options.strategy_override = Some(vec![
        CaptureMethod::AccessibilityPrimary,
        CaptureMethod::SyntheticCopy,
    ]);

    let out = capture(&platform, &store, &cancel, &[&adapter], &options);

    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.method, CaptureMethod::SyntheticCopy);
            assert_eq!(success.text, "synthetic copy capture");
        }
        CaptureOutcome::Failure(failure) => {
            panic!("expected success, got {:?}", failure.status);
        }
    }
}
