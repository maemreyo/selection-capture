use crate::types::{CaptureMethod, FailureKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState {
    Unknown,
    Yes,
    No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppProfile {
    pub bundle_id: String,
    pub ax_supported: TriState,
    pub clipboard_borrow_supported: TriState,
    pub last_success_method: Option<CaptureMethod>,
    pub last_failure_kind: Option<FailureKind>,
}

impl AppProfile {
    pub fn unknown(bundle_id: impl Into<String>) -> Self {
        Self {
            bundle_id: bundle_id.into(),
            ax_supported: TriState::Unknown,
            clipboard_borrow_supported: TriState::Unknown,
            last_success_method: None,
            last_failure_kind: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppProfileUpdate {
    pub ax_supported: Option<TriState>,
    pub clipboard_borrow_supported: Option<TriState>,
    pub last_success_method: Option<CaptureMethod>,
    pub last_failure_kind: Option<FailureKind>,
}
