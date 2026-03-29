#![cfg(feature = "async")]

use selection_capture::{
    capture_async, ActiveApp, AppAdapter, AppProfile, AppProfileStore, AppProfileUpdate,
    CancelSignal, CaptureFailureContext, CaptureMethod, CaptureOptions, CaptureOutcome,
    CapturePlatform, CleanupStatus, PlatformAttemptResult, UserHint,
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
    attempts: Arc<Mutex<Vec<CaptureMethod>>>,
    responses: Arc<Mutex<Vec<PlatformAttemptResult>>>,
}

impl StubPlatform {
    fn new(responses: Vec<PlatformAttemptResult>) -> Self {
        Self {
            attempts: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(responses)),
        }
    }

    fn attempts(&self) -> Vec<CaptureMethod> {
        self.attempts.lock().unwrap().clone()
    }
}

impl CapturePlatform for StubPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        Some(ActiveApp {
            bundle_id: "org.example.async".into(),
            name: "Async Capture Test".into(),
        })
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
        CleanupStatus::Clean
    }
}

fn test_options() -> CaptureOptions {
    let mut options = CaptureOptions::default();
    options.retry_policy.primary_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.range_accessibility = vec![Duration::from_millis(0)];
    options.retry_policy.clipboard = vec![Duration::from_millis(0)];
    options.strategy_override = Some(vec![CaptureMethod::AccessibilityPrimary]);
    options
}

#[tokio::test(flavor = "current_thread")]
async fn capture_async_delegates_to_sync_engine() {
    let platform = StubPlatform::new(vec![PlatformAttemptResult::Success("delegated".into())]);
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;
    let options = test_options();

    let outcome = capture_async(&platform, &store, &cancel, &[&adapter], &options).await;

    match outcome {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.text, "delegated");
            assert_eq!(success.method, CaptureMethod::AccessibilityPrimary);
            assert_eq!(
                platform.attempts(),
                vec![CaptureMethod::AccessibilityPrimary]
            );
        }
        CaptureOutcome::Failure(failure) => {
            panic!("expected success, got {:?}", failure.status);
        }
    }
}
