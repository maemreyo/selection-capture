use crate::traits::CapturePlatform;
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};
#[cfg(target_os = "linux")]
use std::process::Command;

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
        #[cfg(target_os = "linux")]
        {
            match read_primary_selection_text() {
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
        #[cfg(not(target_os = "linux"))]
        {
            PlatformAttemptResult::Unavailable
        }
    }

    fn attempt_clipboard(&self) -> PlatformAttemptResult {
        #[cfg(target_os = "linux")]
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
        #[cfg(not(target_os = "linux"))]
        {
            PlatformAttemptResult::Unavailable
        }
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
        #[cfg(target_os = "linux")]
        {
            return read_active_app().ok().flatten();
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    fn attempt(&self, method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
        Self::dispatch_attempt(&self.backend(), method)
    }

    fn cleanup(&self) -> CleanupStatus {
        CleanupStatus::Clean
    }
}

#[cfg(target_os = "linux")]
fn read_clipboard_text() -> Result<Option<String>, String> {
    try_linux_text_commands(&[
        ("wl-paste", &["--no-newline", "--type", "text"][..]),
        ("xclip", &["-o", "-selection", "clipboard"][..]),
        ("xsel", &["--clipboard", "--output"][..]),
    ])
}

#[cfg(target_os = "linux")]
fn read_primary_selection_text() -> Result<Option<String>, String> {
    try_linux_text_commands(&[
        (
            "wl-paste",
            &["--primary", "--no-newline", "--type", "text"][..],
        ),
        ("xclip", &["-o", "-selection", "primary"][..]),
        ("xsel", &["--primary", "--output"][..]),
    ])
}

#[cfg(target_os = "linux")]
fn try_linux_text_commands(commands: &[(&str, &[&str])]) -> Result<Option<String>, String> {
    let mut errors = Vec::new();

    for (program, args) in commands {
        let output = match Command::new(program).args(*args).output() {
            Ok(output) => output,
            Err(err) => {
                errors.push(format!("{program}: {err}"));
                continue;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            errors.push(format!("{program}: {stderr}"));
            continue;
        }

        let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
        return Ok(normalize_linux_text_stdout(&stdout));
    }

    Err(errors.join("; "))
}

#[cfg(target_os = "linux")]
fn normalize_linux_text_stdout(stdout: &str) -> Option<String> {
    let text = stdout.replace("\r\n", "\n");
    let normalized = text.trim_end_matches(['\r', '\n']);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

#[cfg(target_os = "linux")]
fn read_active_app() -> Result<Option<ActiveApp>, String> {
    let pid = read_active_window_pid()?;
    let name = read_process_name(pid)?;
    let bundle_id =
        read_process_exe_path(pid)?.unwrap_or_else(|| format!("process://{}", name.to_lowercase()));

    Ok(Some(ActiveApp { bundle_id, name }))
}

#[cfg(target_os = "linux")]
fn read_active_window_pid() -> Result<u32, String> {
    let output = Command::new("xdotool")
        .args(["getactivewindow", "getwindowpid"])
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    let pid = stdout
        .trim()
        .parse::<u32>()
        .map_err(|err| err.to_string())?;
    Ok(pid)
}

#[cfg(target_os = "linux")]
fn read_process_name(pid: u32) -> Result<String, String> {
    let pid_arg = pid.to_string();
    let output = Command::new("ps")
        .args(["-p", pid_arg.as_str(), "-o", "comm="])
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    let name = stdout.trim();
    if name.is_empty() {
        return Err("empty process name".to_string());
    }
    Ok(name.to_string())
}

#[cfg(target_os = "linux")]
fn read_process_exe_path(pid: u32) -> Result<Option<String>, String> {
    let exe_link = format!("/proc/{pid}/exe");
    let output = Command::new("readlink")
        .arg(exe_link)
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Ok(None);
        }
        return Err(stderr);
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    let path = stdout.trim();
    if path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(path.to_string()))
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
    fn active_app_probe_does_not_panic() {
        let platform = LinuxPlatform::new();
        let _ = platform.active_app();
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
