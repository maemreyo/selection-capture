use crate::profile::TriState;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveApp {
    pub bundle_id: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMethod {
    AccessibilityPrimary,
    AccessibilityRange,
    ClipboardBorrow,
    SyntheticCopy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureStatus {
    EmptySelection,
    PermissionDenied,
    AppBlocked,
    ClipboardBorrowAmbiguous,
    StrategyExhausted,
    TimedOut,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    PermissionDenied,
    AppBlocked,
    EmptySelection,
    ClipboardAmbiguous,
    TimedOut,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupStatus {
    Clean,
    ClipboardRestoreFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserHint {
    GrantAccessibilityPermission,
    GrantAutomationPermission,
    TryManualCopy,
    AppBlocksDirectCapture,
    RetryInFocusedApp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub primary_accessibility: Vec<Duration>,
    pub range_accessibility: Vec<Duration>,
    pub clipboard: Vec<Duration>,
    pub poll_interval: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            primary_accessibility: vec![Duration::from_millis(0), Duration::from_millis(60)],
            range_accessibility: vec![Duration::from_millis(0)],
            clipboard: vec![Duration::from_millis(120), Duration::from_millis(220)],
            poll_interval: Duration::from_millis(20),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureOptions {
    pub allow_clipboard_borrow: bool,
    pub retry_policy: RetryPolicy,
    pub collect_trace: bool,
    pub overall_timeout: Duration,
    pub strategy_override: Option<Vec<CaptureMethod>>,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            allow_clipboard_borrow: true,
            retry_policy: RetryPolicy::default(),
            collect_trace: false,
            overall_timeout: Duration::from_millis(500),
            strategy_override: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceEvent {
    CaptureStarted,
    ActiveAppDetected(ActiveApp),
    MethodStarted(CaptureMethod),
    MethodSucceeded(CaptureMethod),
    MethodReturnedEmpty(CaptureMethod),
    MethodFailed {
        method: CaptureMethod,
        kind: FailureKind,
    },
    RetryWaitStarted {
        method: CaptureMethod,
        delay: Duration,
    },
    RetryWaitSkipped {
        method: CaptureMethod,
        remaining_budget: Duration,
        needed_delay: Duration,
    },
    Cancelled,
    TimedOut,
    CleanupFinished(CleanupStatus),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureTrace {
    pub events: Vec<TraceEvent>,
    pub cleanup_status: CleanupStatus,
}

impl Default for CaptureTrace {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            cleanup_status: CleanupStatus::Clean,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureSuccess {
    pub text: String,
    pub method: CaptureMethod,
    pub trace: Option<CaptureTrace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureFailureContext {
    pub status: CaptureStatus,
    pub active_app: Option<ActiveApp>,
    pub methods_tried: Vec<CaptureMethod>,
    pub last_method: Option<CaptureMethod>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureFailure {
    pub status: CaptureStatus,
    pub hint: Option<UserHint>,
    pub trace: Option<CaptureTrace>,
    pub cleanup_failed: bool,
    pub context: CaptureFailureContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureOutcome {
    Success(CaptureSuccess),
    Failure(CaptureFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformAttemptResult {
    Success(String),
    EmptySelection,
    PermissionDenied,
    AppBlocked,
    ClipboardBorrowAmbiguous,
    Unavailable,
}

impl PlatformAttemptResult {
    pub fn failure_kind(self) -> Option<FailureKind> {
        match self {
            Self::EmptySelection => Some(FailureKind::EmptySelection),
            Self::PermissionDenied => Some(FailureKind::PermissionDenied),
            Self::AppBlocked => Some(FailureKind::AppBlocked),
            Self::ClipboardBorrowAmbiguous => Some(FailureKind::ClipboardAmbiguous),
            Self::Unavailable | Self::Success(_) => None,
        }
    }
}

impl CaptureMethod {
    pub fn is_ax(self) -> bool {
        matches!(self, Self::AccessibilityPrimary | Self::AccessibilityRange)
    }

    pub fn is_clipboard(self) -> bool {
        matches!(self, Self::ClipboardBorrow | Self::SyntheticCopy)
    }

    pub fn retry_delays(self, policy: &RetryPolicy) -> &[Duration] {
        match self {
            Self::AccessibilityPrimary => &policy.primary_accessibility,
            Self::AccessibilityRange => &policy.range_accessibility,
            Self::ClipboardBorrow | Self::SyntheticCopy => &policy.clipboard,
        }
    }
}

pub fn default_method_order(allow_clipboard_borrow: bool) -> Vec<CaptureMethod> {
    let mut methods = vec![
        CaptureMethod::AccessibilityPrimary,
        CaptureMethod::AccessibilityRange,
    ];
    if allow_clipboard_borrow {
        methods.push(CaptureMethod::ClipboardBorrow);
    }
    methods
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_method_order_includes_clipboard_when_allowed() {
        assert_eq!(
            default_method_order(true),
            vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
                CaptureMethod::ClipboardBorrow,
            ]
        );
    }

    #[test]
    fn default_method_order_excludes_clipboard_when_disallowed() {
        assert_eq!(
            default_method_order(false),
            vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
            ]
        );
    }

    #[test]
    fn retry_delays_use_platform_neutral_policy_fields() {
        let policy = RetryPolicy {
            primary_accessibility: vec![Duration::from_millis(1)],
            range_accessibility: vec![Duration::from_millis(2)],
            clipboard: vec![Duration::from_millis(3)],
            poll_interval: Duration::from_millis(4),
        };

        assert_eq!(
            CaptureMethod::AccessibilityPrimary.retry_delays(&policy),
            &[Duration::from_millis(1)]
        );
        assert_eq!(
            CaptureMethod::AccessibilityRange.retry_delays(&policy),
            &[Duration::from_millis(2)]
        );
        assert_eq!(
            CaptureMethod::ClipboardBorrow.retry_delays(&policy),
            &[Duration::from_millis(3)]
        );
        assert_eq!(
            CaptureMethod::SyntheticCopy.retry_delays(&policy),
            &[Duration::from_millis(3)]
        );
    }
}

pub fn status_from_failure_kind(kind: FailureKind) -> CaptureStatus {
    match kind {
        FailureKind::PermissionDenied => CaptureStatus::PermissionDenied,
        FailureKind::AppBlocked => CaptureStatus::AppBlocked,
        FailureKind::EmptySelection => CaptureStatus::EmptySelection,
        FailureKind::ClipboardAmbiguous => CaptureStatus::ClipboardBorrowAmbiguous,
        FailureKind::TimedOut => CaptureStatus::TimedOut,
        FailureKind::Cancelled => CaptureStatus::Cancelled,
    }
}

pub fn update_for_method_result(
    method: CaptureMethod,
    result: &PlatformAttemptResult,
) -> crate::profile::AppProfileUpdate {
    let mut update = crate::profile::AppProfileUpdate::default();
    if method.is_ax() {
        update.ax_supported = match result {
            PlatformAttemptResult::Success(_) => Some(TriState::Yes),
            PlatformAttemptResult::PermissionDenied | PlatformAttemptResult::AppBlocked => {
                Some(TriState::No)
            }
            _ => None,
        };
    }
    if method.is_clipboard() {
        update.clipboard_borrow_supported = match result {
            PlatformAttemptResult::Success(_) => Some(TriState::Yes),
            PlatformAttemptResult::PermissionDenied | PlatformAttemptResult::AppBlocked => {
                Some(TriState::No)
            }
            _ => None,
        };
    }
    if let PlatformAttemptResult::Success(_) = result {
        update.last_success_method = Some(method);
    } else if let Some(kind) = result.clone().failure_kind() {
        update.last_failure_kind = Some(kind);
    }
    update
}
