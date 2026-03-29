use crate::traits::CapturePlatform;
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};
#[cfg(target_os = "windows")]
use std::process::Command;

#[derive(Debug, Default)]
pub struct WindowsPlatform;

trait WindowsBackend {
    fn attempt_ui_automation(&self) -> PlatformAttemptResult;
    fn attempt_iaccessible(&self) -> PlatformAttemptResult;
    fn attempt_clipboard(&self) -> PlatformAttemptResult;
}

#[derive(Debug, Default)]
struct DefaultWindowsBackend;

impl WindowsBackend for DefaultWindowsBackend {
    fn attempt_ui_automation(&self) -> PlatformAttemptResult {
        PlatformAttemptResult::Unavailable
    }

    fn attempt_iaccessible(&self) -> PlatformAttemptResult {
        PlatformAttemptResult::Unavailable
    }

    fn attempt_clipboard(&self) -> PlatformAttemptResult {
        #[cfg(target_os = "windows")]
        {
            match read_clipboard_text() {
                Ok(Some(text)) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        PlatformAttemptResult::EmptySelection
                    } else {
                        PlatformAttemptResult::Success(trimmed.to_string())
                    }
                }
                Ok(None) => PlatformAttemptResult::EmptySelection,
                Err(_) => PlatformAttemptResult::Unavailable,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            PlatformAttemptResult::Unavailable
        }
    }
}

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }

    pub fn attempt_ui_automation(&self) -> PlatformAttemptResult {
        self.backend().attempt_ui_automation()
    }

    pub fn attempt_iaccessible(&self) -> PlatformAttemptResult {
        self.backend().attempt_iaccessible()
    }

    pub fn attempt_clipboard(&self) -> PlatformAttemptResult {
        self.backend().attempt_clipboard()
    }

    fn backend(&self) -> DefaultWindowsBackend {
        DefaultWindowsBackend
    }

    fn dispatch_attempt<B: WindowsBackend>(
        backend: &B,
        method: CaptureMethod,
    ) -> PlatformAttemptResult {
        match method {
            CaptureMethod::AccessibilityPrimary => backend.attempt_ui_automation(),
            CaptureMethod::AccessibilityRange => backend.attempt_iaccessible(),
            CaptureMethod::ClipboardBorrow | CaptureMethod::SyntheticCopy => {
                backend.attempt_clipboard()
            }
        }
    }
}

impl CapturePlatform for WindowsPlatform {
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

#[cfg(target_os = "windows")]
fn read_clipboard_text() -> Result<Option<String>, String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$t = Get-Clipboard -Raw; if ($null -eq $t) { '' } else { $t }",
        ])
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    Ok(normalize_clipboard_stdout(&stdout))
}

#[cfg(target_os = "windows")]
fn normalize_clipboard_stdout(stdout: &str) -> Option<String> {
    let text = stdout.replace("\r\n", "\n");
    let normalized = text.trim_end_matches(['\r', '\n']);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct StubBackend {
        ui_automation: PlatformAttemptResult,
        iaccessible: PlatformAttemptResult,
        clipboard: PlatformAttemptResult,
    }

    impl WindowsBackend for StubBackend {
        fn attempt_ui_automation(&self) -> PlatformAttemptResult {
            self.ui_automation.clone()
        }

        fn attempt_iaccessible(&self) -> PlatformAttemptResult {
            self.iaccessible.clone()
        }

        fn attempt_clipboard(&self) -> PlatformAttemptResult {
            self.clipboard.clone()
        }
    }

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

    #[test]
    fn dispatches_primary_accessibility_to_ui_automation() {
        let backend = StubBackend {
            ui_automation: PlatformAttemptResult::PermissionDenied,
            iaccessible: PlatformAttemptResult::Unavailable,
            clipboard: PlatformAttemptResult::Unavailable,
        };

        let result =
            WindowsPlatform::dispatch_attempt(&backend, CaptureMethod::AccessibilityPrimary);

        assert_eq!(result, PlatformAttemptResult::PermissionDenied);
    }

    #[test]
    fn dispatches_clipboard_methods_to_clipboard_attempt() {
        let backend = StubBackend {
            ui_automation: PlatformAttemptResult::Unavailable,
            iaccessible: PlatformAttemptResult::Unavailable,
            clipboard: PlatformAttemptResult::Success("clipboard".into()),
        };

        assert_eq!(
            WindowsPlatform::dispatch_attempt(&backend, CaptureMethod::ClipboardBorrow),
            PlatformAttemptResult::Success("clipboard".into())
        );
        assert_eq!(
            WindowsPlatform::dispatch_attempt(&backend, CaptureMethod::SyntheticCopy),
            PlatformAttemptResult::Success("clipboard".into())
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn normalizes_clipboard_stdout_and_strips_trailing_newline() {
        let raw = "line one\r\nline two\r\n";
        assert_eq!(
            normalize_clipboard_stdout(raw),
            Some("line one\nline two".to_string())
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn returns_none_when_clipboard_stdout_is_effectively_empty() {
        assert_eq!(normalize_clipboard_stdout("\r\n"), None);
        assert_eq!(normalize_clipboard_stdout(""), None);
    }
}
