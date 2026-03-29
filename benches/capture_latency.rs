use criterion::{black_box, criterion_group, criterion_main, Criterion};
use selection_capture::{
    capture, ActiveApp, AppAdapter, AppProfile, AppProfileStore, AppProfileUpdate, CancelSignal,
    CaptureFailureContext, CaptureMethod, CaptureOptions, CapturePlatform, CleanupStatus,
    PlatformAttemptResult, UserHint,
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

struct NoAdapter;

impl AppAdapter for NoAdapter {
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

struct StubPlatform {
    app: Option<ActiveApp>,
    responses: Arc<Mutex<Vec<PlatformAttemptResult>>>,
}

impl StubPlatform {
    fn new(responses: Vec<PlatformAttemptResult>) -> Self {
        Self {
            app: Some(ActiveApp {
                bundle_id: "bench.app".to_string(),
                name: "BenchApp".to_string(),
            }),
            responses: Arc::new(Mutex::new(responses)),
        }
    }
}

impl CapturePlatform for StubPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        self.app.clone()
    }

    fn attempt(&self, _method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
        let mut guard = self.responses.lock().expect("responses lock");
        if guard.is_empty() {
            PlatformAttemptResult::Unavailable
        } else {
            guard.remove(0)
        }
    }

    fn cleanup(&self) -> CleanupStatus {
        CleanupStatus::Clean
    }
}

fn bench_capture_latency(c: &mut Criterion) {
    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapter;
    let adapters: [&dyn AppAdapter; 1] = [&adapter];

    let mut success_options = CaptureOptions::default();
    success_options.retry_policy.primary_accessibility = vec![Duration::ZERO];
    success_options.retry_policy.range_accessibility = vec![Duration::ZERO];
    success_options.retry_policy.clipboard = vec![Duration::ZERO];
    success_options.collect_trace = false;

    c.bench_function("capture_success_primary", |b| {
        b.iter(|| {
            let platform =
                StubPlatform::new(vec![PlatformAttemptResult::Success("ok".to_string())]);
            let outcome = capture(
                black_box(&platform),
                black_box(&store),
                black_box(&cancel),
                black_box(&adapters),
                black_box(&success_options),
            );
            black_box(outcome);
        });
    });

    let mut fallback_options = CaptureOptions::default();
    fallback_options.retry_policy.primary_accessibility = vec![Duration::ZERO];
    fallback_options.retry_policy.range_accessibility = vec![Duration::ZERO];
    fallback_options.retry_policy.clipboard = vec![Duration::ZERO];
    fallback_options.collect_trace = true;

    c.bench_function("capture_fallback_to_clipboard", |b| {
        b.iter(|| {
            let platform = StubPlatform::new(vec![
                PlatformAttemptResult::EmptySelection,
                PlatformAttemptResult::Unavailable,
                PlatformAttemptResult::Success("clipboard".to_string()),
            ]);
            let outcome = capture(
                black_box(&platform),
                black_box(&store),
                black_box(&cancel),
                black_box(&adapters),
                black_box(&fallback_options),
            );
            black_box(outcome);
        });
    });
}

criterion_group!(benches, bench_capture_latency);
criterion_main!(benches);
