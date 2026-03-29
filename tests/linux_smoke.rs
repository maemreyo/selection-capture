#![cfg(feature = "linux-alpha")]

use selection_capture::{
    capture, ActiveApp, AppAdapter, AppProfile, AppProfileStore, AppProfileUpdate, CancelSignal,
    CaptureFailureContext, CaptureMethod, CaptureOptions, CaptureOutcome, CapturePlatform,
    CleanupStatus, PlatformAttemptResult, TraceEvent, UserHint,
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
    fn with_app_and_responses(app: ActiveApp, responses: Vec<PlatformAttemptResult>) -> Self {
        Self {
            app: Some(app),
            responses: Arc::new(Mutex::new(responses)),
            cleanup: CleanupStatus::Clean,
        }
    }

    fn new(responses: Vec<PlatformAttemptResult>) -> Self {
        Self::with_app_and_responses(
            ActiveApp {
                bundle_id: "org.example.linux".into(),
                name: "Linux Test App".into(),
            },
            responses,
        )
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

fn assert_capture_success(
    out: CaptureOutcome,
    expected_method: CaptureMethod,
    expected_text: &str,
    expected_started_methods: &[CaptureMethod],
) {
    match out {
        CaptureOutcome::Success(success) => {
            assert_eq!(success.method, expected_method);
            assert_eq!(success.text, expected_text);
            let trace = success.trace.expect("trace should be collected");
            let started_methods: Vec<_> = trace
                .events
                .iter()
                .filter_map(|event| match event {
                    TraceEvent::MethodStarted(method) => Some(*method),
                    _ => None,
                })
                .collect();
            assert_eq!(started_methods, expected_started_methods);
        }
        CaptureOutcome::Failure(failure) => {
            panic!("expected success, got {:?}", failure.status);
        }
    }
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

    assert_capture_success(
        out,
        CaptureMethod::ClipboardBorrow,
        "clipboard capture",
        &[
            CaptureMethod::AccessibilityPrimary,
            CaptureMethod::AccessibilityRange,
            CaptureMethod::ClipboardBorrow,
        ],
    );
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

    assert_capture_success(
        out,
        CaptureMethod::SyntheticCopy,
        "synthetic copy capture",
        &[
            CaptureMethod::AccessibilityPrimary,
            CaptureMethod::SyntheticCopy,
        ],
    );
}

#[test]
fn linux_alpha_desktop_matrix_paths() {
    struct Case {
        app: ActiveApp,
        strategy_override: Option<Vec<CaptureMethod>>,
        responses: Vec<PlatformAttemptResult>,
        expected_method: CaptureMethod,
        expected_text: &'static str,
        expected_started_methods: Vec<CaptureMethod>,
    }

    let cases = vec![
        Case {
            app: ActiveApp {
                bundle_id: "process://gnome-shell".into(),
                name: "GNOME Shell".into(),
            },
            strategy_override: None,
            responses: vec![PlatformAttemptResult::Success("gnome atspi text".into())],
            expected_method: CaptureMethod::AccessibilityPrimary,
            expected_text: "gnome atspi text",
            expected_started_methods: vec![CaptureMethod::AccessibilityPrimary],
        },
        Case {
            app: ActiveApp {
                bundle_id: "process://plasmashell".into(),
                name: "KDE Plasma".into(),
            },
            strategy_override: None,
            responses: vec![
                PlatformAttemptResult::Unavailable,
                PlatformAttemptResult::Success("kde primary selection text".into()),
            ],
            expected_method: CaptureMethod::AccessibilityRange,
            expected_text: "kde primary selection text",
            expected_started_methods: vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
            ],
        },
        Case {
            app: ActiveApp {
                bundle_id: "process://sway".into(),
                name: "Sway".into(),
            },
            strategy_override: None,
            responses: vec![
                PlatformAttemptResult::EmptySelection,
                PlatformAttemptResult::Unavailable,
                PlatformAttemptResult::Success("sway clipboard text".into()),
            ],
            expected_method: CaptureMethod::ClipboardBorrow,
            expected_text: "sway clipboard text",
            expected_started_methods: vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
                CaptureMethod::ClipboardBorrow,
            ],
        },
        Case {
            app: ActiveApp {
                bundle_id: "process://x11-session".into(),
                name: "X11 Session".into(),
            },
            strategy_override: Some(vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::SyntheticCopy,
            ]),
            responses: vec![
                PlatformAttemptResult::Unavailable,
                PlatformAttemptResult::Success("x11 synthetic copy text".into()),
            ],
            expected_method: CaptureMethod::SyntheticCopy,
            expected_text: "x11 synthetic copy text",
            expected_started_methods: vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::SyntheticCopy,
            ],
        },
    ];

    let store = StubStore;
    let cancel = NeverCancel;
    let adapter = NoAdapters;

    for case in cases {
        let platform = StubPlatform::with_app_and_responses(case.app, case.responses);
        let mut options = smoke_options();
        options.strategy_override = case.strategy_override;

        let out = capture(&platform, &store, &cancel, &[&adapter], &options);
        assert_capture_success(
            out,
            case.expected_method,
            case.expected_text,
            &case.expected_started_methods,
        );
    }
}
