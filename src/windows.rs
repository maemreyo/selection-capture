use crate::traits::CapturePlatform;
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};

#[derive(Debug, Default)]
pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl CapturePlatform for WindowsPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        None
    }

    fn attempt(&self, _method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
        PlatformAttemptResult::Unavailable
    }

    fn cleanup(&self) -> CleanupStatus {
        CleanupStatus::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_builds_stub_platform() {
        let platform = WindowsPlatform::new();
        let _ = platform;
    }

    #[test]
    fn active_app_returns_none() {
        let platform = WindowsPlatform::new();
        assert!(platform.active_app().is_none());
    }
}
