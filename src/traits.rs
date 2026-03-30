use crate::profile::{AppProfile, AppProfileUpdate};
use crate::types::{
    ActiveApp, CaptureFailureContext, CaptureMethod, CleanupStatus, PlatformAttemptResult,
    UserHint, WindowFrame,
};

pub trait CancelSignal {
    fn is_cancelled(&self) -> bool;
}

pub trait AppProfileStore {
    fn load(&self, app: &ActiveApp) -> AppProfile;
    fn merge_update(&self, app: &ActiveApp, update: AppProfileUpdate);
}

pub trait AppAdapter: Send + Sync {
    fn matches(&self, app: &ActiveApp) -> bool;
    fn strategy_override(&self, app: &ActiveApp) -> Option<Vec<CaptureMethod>>;
    fn hint_override(&self, context: &CaptureFailureContext) -> Option<UserHint>;
}

pub trait CapturePlatform {
    fn active_app(&self) -> Option<ActiveApp>;
    fn focused_window_frame(&self) -> Option<WindowFrame> {
        None
    }
    fn attempt(&self, method: CaptureMethod, app: Option<&ActiveApp>) -> PlatformAttemptResult;
    fn cleanup(&self) -> CleanupStatus;
}

pub trait MonitorPlatform {
    fn next_selection_change(&self) -> Option<String>;
}
