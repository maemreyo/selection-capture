use crate::cache::{prioritize_profile_method, record_method_outcome};
use crate::profile::AppProfileUpdate;
use crate::traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
use crate::types::{
    default_method_order, status_from_failure_kind, update_for_method_result, ActiveApp,
    CaptureFailure, CaptureFailureContext, CaptureMethod, CaptureOptions, CaptureOutcome,
    CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus, FailureKind, PlatformAttemptResult,
    TraceEvent, UserHint, WouldBlock,
};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct ScheduledAttempt {
    method: CaptureMethod,
    delays: Vec<Duration>,
    next_attempt_idx: usize,
    next_due: Instant,
    order: usize,
}

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

    let methods = resolve_methods(store, active_app.as_ref(), adapters, options);
    let mut methods_tried = Vec::new();
    let mut last_failure: Option<FailureKind> = None;

    let mut schedule = build_capture_schedule(&methods, options, start);
    while !schedule.is_empty() {
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
                start,
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
                start,
            );
        }

        let Some(next_index) =
            select_next_scheduled_attempt(&schedule, options.interleave_method_retries)
        else {
            break;
        };
        let next = &schedule[next_index];
        if now < next.next_due {
            let wait = next.next_due.saturating_duration_since(now);
            let remaining = deadline.saturating_duration_since(now);
            if remaining < wait {
                push_trace(
                    &mut trace,
                    TraceEvent::RetryWaitSkipped {
                        method: next.method,
                        remaining_budget: remaining,
                        needed_delay: wait,
                    },
                );
                break;
            }

            push_trace(
                &mut trace,
                TraceEvent::RetryWaitStarted {
                    method: next.method,
                    delay: wait,
                },
            );
            if wait_with_polling(wait, deadline, cancel, options.retry_policy.poll_interval) {
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
                    start,
                );
            }
            continue;
        }

        let method = schedule[next_index].method;
        methods_tried.push(method);
        push_trace(&mut trace, TraceEvent::MethodStarted(method));
        let attempt_started_at = Instant::now();
        let result = platform.attempt(method, active_app.as_ref());
        push_trace(
            &mut trace,
            TraceEvent::MethodFinished {
                method,
                elapsed: attempt_started_at.elapsed(),
            },
        );
        store_profile_update(store, active_app.as_ref(), method, &result);

        if let PlatformAttemptResult::Success(text) = result {
            push_trace(&mut trace, TraceEvent::MethodSucceeded(method));
            return finish_success(platform, trace, text, method, start);
        }
        if let Some(kind) = record_attempt_failure(&mut trace, method, &result) {
            last_failure = Some(kind);
        }

        schedule[next_index].next_attempt_idx += 1;
        let next_attempt_idx = schedule[next_index].next_attempt_idx;
        if next_attempt_idx >= schedule[next_index].delays.len() {
            schedule.remove(next_index);
            continue;
        }
        schedule[next_index].next_due =
            Instant::now() + schedule[next_index].delays[next_attempt_idx];
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
        start,
    )
}

pub fn try_capture(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureOptions,
) -> Result<CaptureOutcome, WouldBlock> {
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

    let methods = resolve_methods(store, active_app.as_ref(), adapters, options);
    let mut methods_tried = Vec::new();
    let mut last_failure: Option<FailureKind> = None;
    let mut would_block = false;

    for method in methods {
        if cancel.is_cancelled() {
            push_trace(&mut trace, TraceEvent::Cancelled);
            return Ok(finish_failure(
                platform,
                trace,
                CaptureStatus::Cancelled,
                None,
                active_app.clone(),
                methods_tried,
                None,
                false,
                start,
            ));
        }

        if Instant::now() >= deadline {
            push_trace(&mut trace, TraceEvent::TimedOut);
            return Ok(finish_failure(
                platform,
                trace,
                CaptureStatus::TimedOut,
                None,
                active_app.clone(),
                methods_tried,
                None,
                false,
                start,
            ));
        }

        let delays = method.retry_delays(&options.retry_policy);
        if delays.is_empty() {
            continue;
        }

        if delays[0] > Duration::ZERO {
            would_block = true;
            continue;
        }

        methods_tried.push(method);
        push_trace(&mut trace, TraceEvent::MethodStarted(method));
        let attempt_started_at = Instant::now();
        let result = platform.attempt(method, active_app.as_ref());
        push_trace(
            &mut trace,
            TraceEvent::MethodFinished {
                method,
                elapsed: attempt_started_at.elapsed(),
            },
        );
        store_profile_update(store, active_app.as_ref(), method, &result);

        if let PlatformAttemptResult::Success(text) = result {
            push_trace(&mut trace, TraceEvent::MethodSucceeded(method));
            return Ok(finish_success(platform, trace, text, method, start));
        }
        if let Some(kind) = record_attempt_failure(&mut trace, method, &result) {
            last_failure = Some(kind);
        }

        if delays.len() > 1 {
            would_block = true;
        }
    }

    if would_block {
        return Err(WouldBlock);
    }

    let status = last_failure
        .map(status_from_failure_kind)
        .unwrap_or(CaptureStatus::StrategyExhausted);
    Ok(finish_failure(
        platform,
        trace,
        status,
        None,
        active_app,
        methods_tried,
        None,
        false,
        start,
    ))
}

fn resolve_methods(
    store: &impl AppProfileStore,
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

        let profile = store.load(app);
        return prioritize_profile_method(
            default_method_order(options.allow_clipboard_borrow),
            Some(&profile),
        );
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
        record_method_outcome(&app.bundle_id, method, result);
        let update: AppProfileUpdate = update_for_method_result(method, result);
        store.merge_update(app, update);
    }
}

/// Records trace events for a non-success attempt result and returns the resulting
/// `FailureKind` if one applies. Returns `None` for `Unavailable` (no state change).
/// Does NOT handle the `Success` variant — callers must check that first.
fn record_attempt_failure(
    trace: &mut Option<CaptureTrace>,
    method: CaptureMethod,
    result: &PlatformAttemptResult,
) -> Option<FailureKind> {
    match result {
        PlatformAttemptResult::EmptySelection => {
            push_trace(trace, TraceEvent::MethodReturnedEmpty(method));
            Some(FailureKind::EmptySelection)
        }
        PlatformAttemptResult::PermissionDenied => {
            push_trace(
                trace,
                TraceEvent::MethodFailed {
                    method,
                    kind: FailureKind::PermissionDenied,
                },
            );
            Some(FailureKind::PermissionDenied)
        }
        PlatformAttemptResult::AppBlocked => {
            push_trace(
                trace,
                TraceEvent::MethodFailed {
                    method,
                    kind: FailureKind::AppBlocked,
                },
            );
            Some(FailureKind::AppBlocked)
        }
        PlatformAttemptResult::ClipboardBorrowAmbiguous => {
            push_trace(
                trace,
                TraceEvent::MethodFailed {
                    method,
                    kind: FailureKind::ClipboardAmbiguous,
                },
            );
            Some(FailureKind::ClipboardAmbiguous)
        }
        PlatformAttemptResult::Unavailable | PlatformAttemptResult::Success(_) => None,
    }
}

fn build_capture_schedule(
    methods: &[CaptureMethod],
    options: &CaptureOptions,
    start: Instant,
) -> Vec<ScheduledAttempt> {
    let mut schedule = Vec::new();
    for (order, method) in methods.iter().copied().enumerate() {
        let delays = method.retry_delays(&options.retry_policy);
        if delays.is_empty() {
            continue;
        }
        schedule.push(ScheduledAttempt {
            method,
            delays: delays.to_vec(),
            next_attempt_idx: 0,
            next_due: start,
            order,
        });
    }
    schedule
}

fn select_next_scheduled_attempt(
    schedule: &[ScheduledAttempt],
    interleave_method_retries: bool,
) -> Option<usize> {
    if !interleave_method_retries {
        return schedule
            .iter()
            .enumerate()
            .min_by_key(|(_, attempt)| attempt.order)
            .map(|(index, _)| index);
    }
    schedule
        .iter()
        .enumerate()
        .min_by_key(|(_, attempt)| (attempt.next_due, attempt.order))
        .map(|(index, _)| index)
}

fn finish_success(
    platform: &impl CapturePlatform,
    mut trace: Option<CaptureTrace>,
    text: String,
    method: CaptureMethod,
    started_at: Instant,
) -> CaptureOutcome {
    let cleanup_status = platform.cleanup();
    finalize_trace(&mut trace, cleanup_status, started_at.elapsed());
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
    started_at: Instant,
) -> CaptureOutcome {
    let cleanup_status = platform.cleanup();
    let cleanup_failed = cleanup_failed || cleanup_status == CleanupStatus::ClipboardRestoreFailed;
    finalize_trace(&mut trace, cleanup_status, started_at.elapsed());

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

fn finalize_trace(
    trace: &mut Option<CaptureTrace>,
    status: CleanupStatus,
    total_elapsed: Duration,
) {
    if let Some(trace) = trace.as_mut() {
        trace.cleanup_status = status;
        trace.total_elapsed = total_elapsed;
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
#[path = "engine_tests.rs"]
mod tests;
