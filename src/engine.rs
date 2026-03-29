use crate::profile::AppProfileUpdate;
use crate::traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
use crate::types::{
    default_method_order, status_from_failure_kind, update_for_method_result, ActiveApp,
    CaptureFailure, CaptureFailureContext, CaptureMethod, CaptureOptions, CaptureOutcome,
    CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus, FailureKind, PlatformAttemptResult,
    TraceEvent, UserHint,
};
use std::thread;
use std::time::{Duration, Instant};

pub fn capture(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureOptions,
) -> CaptureOutcome {
    let start = Instant::now();
    let deadline = start + options.overall_timeout;

    let mut trace = if options.collect_trace {
        Some(CaptureTrace::default())
    } else {
        None
    };
    push_trace(&mut trace, TraceEvent::CaptureStarted);

    let active_app = platform.active_app();
    if let Some(app) = active_app.clone() {
        push_trace(&mut trace, TraceEvent::ActiveAppDetected(app));
    }

    let methods = resolve_methods(active_app.as_ref(), adapters, options);
    let mut methods_tried = Vec::new();
    let mut last_failure: Option<FailureKind> = None;

    for method in methods {
        let delays = method.retry_delays(&options.retry_policy);
        for (idx, delay) in delays.iter().enumerate() {
            if cancel.is_cancelled() {
                push_trace(&mut trace, TraceEvent::Cancelled);
                return finish_failure(
                    platform,
                    trace,
                    CaptureStatus::Cancelled,
                    None,
                    active_app.clone(),
                    methods_tried,
                    None,
                    false,
                );
            }

            let now = Instant::now();
            if now >= deadline {
                push_trace(&mut trace, TraceEvent::TimedOut);
                return finish_failure(
                    platform,
                    trace,
                    CaptureStatus::TimedOut,
                    None,
                    active_app.clone(),
                    methods_tried,
                    None,
                    false,
                );
            }

            if idx > 0 {
                let remaining = deadline.saturating_duration_since(now);
                if remaining < *delay {
                    push_trace(
                        &mut trace,
                        TraceEvent::RetryWaitSkipped {
                            method,
                            remaining_budget: remaining,
                            needed_delay: *delay,
                        },
                    );
                    break;
                }

                push_trace(
                    &mut trace,
                    TraceEvent::RetryWaitStarted {
                        method,
                        delay: *delay,
                    },
                );

                if wait_with_polling(*delay, deadline, cancel, options.retry_policy.poll_interval) {
                    push_trace(&mut trace, TraceEvent::Cancelled);
                    return finish_failure(
                        platform,
                        trace,
                        CaptureStatus::Cancelled,
                        None,
                        active_app.clone(),
                        methods_tried,
                        None,
                        false,
                    );
                }
            }

            methods_tried.push(method);
            push_trace(&mut trace, TraceEvent::MethodStarted(method));
            let result = platform.attempt(method, active_app.as_ref());
            store_profile_update(store, active_app.as_ref(), method, &result);

            match result {
                PlatformAttemptResult::Success(text) => {
                    push_trace(&mut trace, TraceEvent::MethodSucceeded(method));
                    return finish_success(platform, trace, text, method);
                }
                PlatformAttemptResult::EmptySelection => {
                    push_trace(&mut trace, TraceEvent::MethodReturnedEmpty(method));
                    last_failure = Some(FailureKind::EmptySelection);
                }
                PlatformAttemptResult::PermissionDenied => {
                    push_trace(
                        &mut trace,
                        TraceEvent::MethodFailed {
                            method,
                            kind: FailureKind::PermissionDenied,
                        },
                    );
                    last_failure = Some(FailureKind::PermissionDenied);
                }
                PlatformAttemptResult::AppBlocked => {
                    push_trace(
                        &mut trace,
                        TraceEvent::MethodFailed {
                            method,
                            kind: FailureKind::AppBlocked,
                        },
                    );
                    last_failure = Some(FailureKind::AppBlocked);
                }
                PlatformAttemptResult::ClipboardBorrowAmbiguous => {
                    push_trace(
                        &mut trace,
                        TraceEvent::MethodFailed {
                            method,
                            kind: FailureKind::ClipboardAmbiguous,
                        },
                    );
                    last_failure = Some(FailureKind::ClipboardAmbiguous);
                }
                PlatformAttemptResult::Unavailable => {}
            }
        }
    }

    let status = last_failure
        .map(status_from_failure_kind)
        .unwrap_or(CaptureStatus::StrategyExhausted);

    finish_failure(
        platform,
        trace,
        status,
        None,
        active_app,
        methods_tried,
        None,
        false,
    )
}

fn resolve_methods(
    active_app: Option<&ActiveApp>,
    adapters: &[&dyn AppAdapter],
    options: &CaptureOptions,
) -> Vec<CaptureMethod> {
    if let Some(methods) = &options.strategy_override {
        return methods.clone();
    }
    if let Some(app) = active_app {
        for adapter in adapters {
            if adapter.matches(app) {
                if let Some(methods) = adapter.strategy_override(app) {
                    return methods;
                }
            }
        }
    }
    default_method_order(options.allow_clipboard_borrow)
}

fn store_profile_update(
    store: &impl AppProfileStore,
    active_app: Option<&ActiveApp>,
    method: CaptureMethod,
    result: &PlatformAttemptResult,
) {
    if let Some(app) = active_app {
        let update: AppProfileUpdate = update_for_method_result(method, result);
        store.merge_update(app, update);
    }
}

fn finish_success(
    platform: &impl CapturePlatform,
    mut trace: Option<CaptureTrace>,
    text: String,
    method: CaptureMethod,
) -> CaptureOutcome {
    let cleanup_status = platform.cleanup();
    set_cleanup(&mut trace, cleanup_status);
    CaptureOutcome::Success(CaptureSuccess {
        text,
        method,
        trace,
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_failure(
    platform: &impl CapturePlatform,
    mut trace: Option<CaptureTrace>,
    status: CaptureStatus,
    hint: Option<UserHint>,
    active_app: Option<ActiveApp>,
    methods_tried: Vec<CaptureMethod>,
    last_method: Option<CaptureMethod>,
    cleanup_failed: bool,
) -> CaptureOutcome {
    let cleanup_status = platform.cleanup();
    let cleanup_failed = cleanup_failed || cleanup_status == CleanupStatus::ClipboardRestoreFailed;
    set_cleanup(&mut trace, cleanup_status);

    CaptureOutcome::Failure(CaptureFailure {
        status,
        hint,
        trace,
        cleanup_failed,
        context: CaptureFailureContext {
            status,
            active_app,
            methods_tried,
            last_method,
        },
    })
}

fn push_trace(trace: &mut Option<CaptureTrace>, event: TraceEvent) {
    if let Some(trace) = trace.as_mut() {
        trace.events.push(event);
    }
}

fn set_cleanup(trace: &mut Option<CaptureTrace>, status: CleanupStatus) {
    if let Some(trace) = trace.as_mut() {
        trace.cleanup_status = status;
        trace.events.push(TraceEvent::CleanupFinished(status));
    }
}

fn wait_with_polling(
    total: Duration,
    deadline: Instant,
    cancel: &impl CancelSignal,
    poll_interval: Duration,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < total {
        if cancel.is_cancelled() {
            return true;
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        let remaining_delay = total.saturating_sub(start.elapsed());
        let remaining_budget = deadline.saturating_duration_since(now);
        let step = min_duration(
            min_duration(remaining_delay, remaining_budget),
            poll_interval,
        );
        if step.is_zero() {
            return false;
        }
        thread::sleep(step);
    }
    cancel.is_cancelled()
}

fn min_duration(a: Duration, b: Duration) -> Duration {
    if a <= b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{AppProfile, AppProfileUpdate};
    use crate::traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
    use crate::types::{
        ActiveApp, CaptureOptions, CaptureStatus, CleanupStatus, PlatformAttemptResult,
    };
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

        fn attempt(
            &self,
            _method: CaptureMethod,
            _app: Option<&ActiveApp>,
        ) -> PlatformAttemptResult {
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

    #[test]
    fn collect_trace_true_always_returns_trace() {
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
        options.retry_policy.ax_text = vec![Duration::from_millis(0)];
        options.retry_policy.ax_range = vec![Duration::from_millis(0)];
        options.retry_policy.clipboard_borrow = vec![Duration::from_millis(0)];
        let out = capture(&platform, &store, &cancel, &[&adapter], &options);
        match out {
            CaptureOutcome::Success(success) => assert!(success.trace.is_some()),
            CaptureOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[test]
    fn skips_retry_when_budget_is_too_small() {
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
        options.retry_policy.ax_text = vec![Duration::from_millis(0)];
        options.retry_policy.ax_range = vec![Duration::from_millis(0)];
        options.retry_policy.clipboard_borrow = vec![Duration::from_millis(0)];
        let out = capture(&platform, &store, &cancel, &[&adapter], &options);
        match out {
            CaptureOutcome::Success(success) => {
                assert_eq!(success.text, "selected from clipboard");
                assert_eq!(success.method, CaptureMethod::ClipboardBorrowAppleScript);
                let trace = success.trace.expect("trace");
                assert!(trace.events.iter().any(|event| matches!(
                    event,
                    TraceEvent::MethodReturnedEmpty(CaptureMethod::AxSelectedText)
                )));
                assert!(trace.events.iter().any(|event| matches!(
                    event,
                    TraceEvent::MethodSucceeded(CaptureMethod::ClipboardBorrowAppleScript)
                )));
            }
            CaptureOutcome::Failure(_) => panic!("expected success"),
        }
    }
}
