use crate::traits::CapturePlatform;
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};

#[derive(Debug, Default)]
pub struct LinuxPlatform;

trait LinuxBackend {
    fn attempt_atspi(&self) -> PlatformAttemptResult;
    fn attempt_x11_selection(&self) -> PlatformAttemptResult;
    fn attempt_clipboard(&self) -> PlatformAttemptResult;
}

#[derive(Debug, Default)]
struct DefaultLinuxBackend;

impl LinuxBackend for DefaultLinuxBackend {
    fn attempt_atspi(&self) -> PlatformAttemptResult {
        PlatformAttemptResult::Unavailable
    }

    fn attempt_x11_selection(&self) -> PlatformAttemptResult {
        PlatformAttemptResult::Unavailable
    }

    fn attempt_clipboard(&self) -> PlatformAttemptResult {
        PlatformAttemptResult::Unavailable
    }
}

impl LinuxPlatform {
    pub fn new() -> Self {
        Self
    }

    pub fn attempt_atspi(&self) -> PlatformAttemptResult {
        self.backend().attempt_atspi()
    }

    pub fn attempt_x11_selection(&self) -> PlatformAttemptResult {
        self.backend().attempt_x11_selection()
    }

    pub fn attempt_clipboard(&self) -> PlatformAttemptResult {
        self.backend().attempt_clipboard()
    }

    fn backend(&self) -> DefaultLinuxBackend {
        DefaultLinuxBackend
    }

    fn dispatch_attempt<B: LinuxBackend>(
        backend: &B,
        method: CaptureMethod,
    ) -> PlatformAttemptResult {
        match method {
            CaptureMethod::AccessibilityPrimary => backend.attempt_atspi(),
            CaptureMethod::AccessibilityRange => backend.attempt_x11_selection(),
            CaptureMethod::ClipboardBorrow | CaptureMethod::SyntheticCopy => {
                backend.attempt_clipboard()
            }
        }
    }
}

impl CapturePlatform for LinuxPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        None
    }

    fn attempt(&self, method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
        Self::dispatch_attempt(&self.backend(), method)
    }

    fn cleanup(&self) -> CleanupStatus {
        CleanupStatus::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct StubBackend {
        atspi: PlatformAttemptResult,
        x11_selection: PlatformAttemptResult,
        clipboard: PlatformAttemptResult,
    }

    impl LinuxBackend for StubBackend {
        fn attempt_atspi(&self) -> PlatformAttemptResult {
            self.atspi.clone()
        }

        fn attempt_x11_selection(&self) -> PlatformAttemptResult {
            self.x11_selection.clone()
        }

        fn attempt_clipboard(&self) -> PlatformAttemptResult {
            self.clipboard.clone()
        }
    }

    #[test]
    fn constructor_builds_stub_platform() {
        let platform = LinuxPlatform::new();
        let _ = platform;
    }

    #[test]
    fn active_app_returns_none() {
        let platform = LinuxPlatform::new();
        assert!(platform.active_app().is_none());
    }

    #[test]
    fn dispatches_primary_accessibility_to_atspi() {
        let backend = StubBackend {
            atspi: PlatformAttemptResult::PermissionDenied,
            x11_selection: PlatformAttemptResult::Unavailable,
            clipboard: PlatformAttemptResult::Unavailable,
        };

        let result = LinuxPlatform::dispatch_attempt(&backend, CaptureMethod::AccessibilityPrimary);

        assert_eq!(result, PlatformAttemptResult::PermissionDenied);
    }

    #[test]
    fn dispatches_range_accessibility_to_x11_selection() {
        let backend = StubBackend {
            atspi: PlatformAttemptResult::Unavailable,
            x11_selection: PlatformAttemptResult::EmptySelection,
            clipboard: PlatformAttemptResult::Unavailable,
        };

        let result = LinuxPlatform::dispatch_attempt(&backend, CaptureMethod::AccessibilityRange);

        assert_eq!(result, PlatformAttemptResult::EmptySelection);
    }

    #[test]
    fn dispatches_clipboard_methods_to_clipboard_attempt() {
        let backend = StubBackend {
            atspi: PlatformAttemptResult::Unavailable,
            x11_selection: PlatformAttemptResult::Unavailable,
            clipboard: PlatformAttemptResult::Success("clipboard".into()),
        };

        assert_eq!(
            LinuxPlatform::dispatch_attempt(&backend, CaptureMethod::ClipboardBorrow),
            PlatformAttemptResult::Success("clipboard".into())
        );
        assert_eq!(
            LinuxPlatform::dispatch_attempt(&backend, CaptureMethod::SyntheticCopy),
            PlatformAttemptResult::Success("clipboard".into())
        );
    }
}
