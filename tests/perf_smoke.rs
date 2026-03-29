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

struct ProfileStore {
    profile: AppProfile,
}

impl AppProfileStore for ProfileStore {
    fn load(&self, _app: &ActiveApp) -> AppProfile {
        self.profile.clone()
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
}

impl StubPlatform {
    fn new() -> Self {
        Self {
            app: Some(ActiveApp {
                bundle_id: "com.example.profiled".into(),
                name: "Profiled App".into(),
            }),
            attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CapturePlatform for StubPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        self.app.clone()
    }

    fn attempt(&self, method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
        self.attempts.lock().unwrap().push(method);
        PlatformAttemptResult::Success("captured".into())
    }

    fn cleanup(&self) -> CleanupStatus {
        CleanupStatus::Clean
    }
}

fn smoke_options() -> CaptureOptions {
    let mut options = CaptureOptions::default();
    options.retry_policy.primary_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.range_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.clipboard = vec![Duration::from_millis(0)];
    options
}

#[test]
fn profile_last_success_method_is_tried_first_when_in_default_order() {
    let platform = StubPlatform::new();
    let store = ProfileStore {
        profile: AppProfile {
            bundle_id: "com.example.profiled".into(),
            ax_supported: selection_capture::TriState::Unknown,
            clipboard_borrow_supported: selection_capture::TriState::Unknown,
            last_success_method: Some(CaptureMethod::ClipboardBorrow),
            last_failure_kind: None,
        },
    };
    let cancel = NeverCancel;
    let adapter = NoAdapters;

    let out = capture(&platform, &store, &cancel, &[&adapter], &smoke_options());

    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.method, CaptureMethod::ClipboardBorrow);
        }
        CaptureOutcome::Failure(failure) => {
            panic!("expected success, got {:?}", failure.status);
        }
    }

    let attempts = platform.attempts.lock().unwrap().clone();
    assert_eq!(attempts, vec![CaptureMethod::ClipboardBorrow]);
}
