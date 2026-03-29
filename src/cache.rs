use crate::profile::{AppProfile, TriState};
use crate::types::{CaptureMethod, PlatformAttemptResult};
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

const ADAPTIVE_HISTORY_WINDOW: usize = 32;
const ADAPTIVE_RECENT_WINDOW: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MethodAttemptSnapshot {
    method: CaptureMethod,
    success: bool,
}

fn adaptive_history() -> &'static Mutex<HashMap<String, VecDeque<MethodAttemptSnapshot>>> {
    static HISTORY: OnceLock<Mutex<HashMap<String, VecDeque<MethodAttemptSnapshot>>>> =
        OnceLock::new();
    HISTORY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn record_method_outcome(
    app_bundle_id: &str,
    method: CaptureMethod,
    result: &PlatformAttemptResult,
) {
    let success = matches!(result, PlatformAttemptResult::Success(_));
    let snapshot = MethodAttemptSnapshot { method, success };
    if let Ok(mut by_app) = adaptive_history().lock() {
        let history = by_app
            .entry(app_bundle_id.to_string())
            .or_insert_with(VecDeque::new);
        if history.len() >= ADAPTIVE_HISTORY_WINDOW {
            history.pop_front();
        }
        history.push_back(snapshot);
    }
}

pub(crate) fn prioritize_profile_method(
    mut methods: Vec<CaptureMethod>,
    profile: Option<&AppProfile>,
) -> Vec<CaptureMethod> {
    let Some(profile) = profile else {
        return methods;
    };

    let mut scored = methods
        .iter()
        .copied()
        .enumerate()
        .map(|(index, method)| (index, method, score_method_for_profile(method, profile)))
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    methods = scored.into_iter().map(|(_, method, _)| method).collect();
    methods
}

fn score_method_for_profile(method: CaptureMethod, profile: &AppProfile) -> i32 {
    let mut score = 0;
    if profile.last_success_method == Some(method) {
        score += 100;
    }

    match (method.is_ax(), profile.ax_supported) {
        (true, TriState::Yes) => score += 20,
        (true, TriState::No) => score -= 40,
        _ => {}
    }

    match (method.is_clipboard(), profile.clipboard_borrow_supported) {
        (true, TriState::Yes) => score += 15,
        (true, TriState::No) => score -= 35,
        _ => {}
    }

    if let Some(last_failure) = profile.last_failure_kind {
        use crate::types::FailureKind;
        match last_failure {
            FailureKind::PermissionDenied => {
                if method.is_ax() && profile.ax_supported != TriState::Yes {
                    score -= 10;
                }
                if method.is_clipboard() && profile.clipboard_borrow_supported != TriState::Yes {
                    score -= 8;
                }
            }
            FailureKind::ClipboardAmbiguous => {
                if method.is_clipboard() {
                    score -= 12;
                }
            }
            FailureKind::EmptySelection => {
                if profile.last_success_method == Some(method) {
                    score -= 5;
                }
            }
            FailureKind::AppBlocked | FailureKind::TimedOut | FailureKind::Cancelled => {}
        }
    }

    score += score_method_from_recent_history(method, &profile.bundle_id);

    score
}

fn score_method_from_recent_history(method: CaptureMethod, bundle_id: &str) -> i32 {
    let Ok(by_app) = adaptive_history().lock() else {
        return 0;
    };
    let Some(history) = by_app.get(bundle_id) else {
        return 0;
    };

    let mut method_successes = 0;
    let mut method_failures = 0;
    let mut recent_successes = 0;
    let mut recent_failures = 0;

    for (idx, attempt) in history
        .iter()
        .rev()
        .take(ADAPTIVE_RECENT_WINDOW)
        .enumerate()
    {
        if attempt.method != method {
            continue;
        }
        if attempt.success {
            method_successes += 1;
            if idx < 4 {
                recent_successes += 1;
            }
        } else {
            method_failures += 1;
            if idx < 4 {
                recent_failures += 1;
            }
        }
    }

    (method_successes * 6) - (method_failures * 4) + (recent_successes * 4) - (recent_failures * 3)
}

#[cfg(test)]
pub(crate) fn reset_adaptive_history_for_tests() {
    if let Ok(mut by_app) = adaptive_history().lock() {
        by_app.clear();
    }
}

#[cfg(test)]
pub(crate) fn adaptive_history_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{AppProfile, TriState};

    #[test]
    fn moves_profile_method_to_front_when_present() {
        let _guard = adaptive_history_test_lock()
            .lock()
            .expect("test lock poisoned");
        reset_adaptive_history_for_tests();
        let profile = AppProfile {
            bundle_id: "com.example".into(),
            ax_supported: TriState::Unknown,
            clipboard_borrow_supported: TriState::Unknown,
            last_success_method: Some(CaptureMethod::ClipboardBorrow),
            last_failure_kind: None,
        };

        let methods = prioritize_profile_method(
            vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
                CaptureMethod::ClipboardBorrow,
            ],
            Some(&profile),
        );

        assert_eq!(
            methods,
            vec![
                CaptureMethod::ClipboardBorrow,
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
            ]
        );
    }

    #[test]
    fn deprioritizes_ax_when_profile_marks_ax_unsupported() {
        let _guard = adaptive_history_test_lock()
            .lock()
            .expect("test lock poisoned");
        reset_adaptive_history_for_tests();
        let profile = AppProfile {
            bundle_id: "com.example".into(),
            ax_supported: TriState::No,
            clipboard_borrow_supported: TriState::Yes,
            last_success_method: None,
            last_failure_kind: None,
        };

        let methods = prioritize_profile_method(
            vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
                CaptureMethod::ClipboardBorrow,
            ],
            Some(&profile),
        );

        assert_eq!(
            methods,
            vec![
                CaptureMethod::ClipboardBorrow,
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
            ]
        );
    }

    #[test]
    fn keeps_last_success_on_top_when_capabilities_are_ambiguous() {
        let _guard = adaptive_history_test_lock()
            .lock()
            .expect("test lock poisoned");
        reset_adaptive_history_for_tests();
        let profile = AppProfile {
            bundle_id: "com.example".into(),
            ax_supported: TriState::Unknown,
            clipboard_borrow_supported: TriState::Unknown,
            last_success_method: Some(CaptureMethod::AccessibilityRange),
            last_failure_kind: Some(crate::types::FailureKind::EmptySelection),
        };

        let methods = prioritize_profile_method(
            vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
                CaptureMethod::ClipboardBorrow,
            ],
            Some(&profile),
        );

        assert_eq!(methods[0], CaptureMethod::AccessibilityRange);
    }

    #[test]
    fn deprioritizes_clipboard_after_ambiguous_failures() {
        let _guard = adaptive_history_test_lock()
            .lock()
            .expect("test lock poisoned");
        reset_adaptive_history_for_tests();
        let profile = AppProfile {
            bundle_id: "com.example".into(),
            ax_supported: TriState::Yes,
            clipboard_borrow_supported: TriState::Unknown,
            last_success_method: None,
            last_failure_kind: Some(crate::types::FailureKind::ClipboardAmbiguous),
        };

        let methods = prioritize_profile_method(
            vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
                CaptureMethod::ClipboardBorrow,
            ],
            Some(&profile),
        );

        assert_eq!(methods.last(), Some(&CaptureMethod::ClipboardBorrow));
    }

    #[test]
    fn recent_history_promotes_consistent_successful_method() {
        let _guard = adaptive_history_test_lock()
            .lock()
            .expect("test lock poisoned");
        reset_adaptive_history_for_tests();
        let profile = AppProfile {
            bundle_id: "com.history".into(),
            ax_supported: TriState::Unknown,
            clipboard_borrow_supported: TriState::Unknown,
            last_success_method: None,
            last_failure_kind: None,
        };

        for _ in 0..4 {
            record_method_outcome(
                "com.history",
                CaptureMethod::ClipboardBorrow,
                &PlatformAttemptResult::Success("ok".to_string()),
            );
        }
        for _ in 0..2 {
            record_method_outcome(
                "com.history",
                CaptureMethod::AccessibilityPrimary,
                &PlatformAttemptResult::EmptySelection,
            );
        }

        let methods = prioritize_profile_method(
            vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
                CaptureMethod::ClipboardBorrow,
            ],
            Some(&profile),
        );

        assert_eq!(methods[0], CaptureMethod::ClipboardBorrow);
    }
}
