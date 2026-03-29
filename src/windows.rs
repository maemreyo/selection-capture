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
        #[cfg(target_os = "windows")]
        {
            match read_uia_text() {
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

    fn attempt_iaccessible(&self) -> PlatformAttemptResult {
        #[cfg(target_os = "windows")]
        {
            match read_iaccessible_text() {
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
        #[cfg(target_os = "windows")]
        {
            return read_active_app().ok().flatten();
        }
        #[cfg(not(target_os = "windows"))]
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
    Ok(normalize_windows_text_stdout(&stdout))
}

#[cfg(target_os = "windows")]
fn read_uia_text() -> Result<Option<String>, String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$focused = [System.Windows.Automation.AutomationElement]::FocusedElement
if ($null -eq $focused) { return }
try {
  $textPattern = $focused.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
} catch {
  $textPattern = $null
}
if ($null -ne $textPattern) {
  $selection = $textPattern.GetSelection()
  if ($null -ne $selection -and $selection.Length -gt 0) {
    $text = $selection[0].GetText(-1)
    if ($null -ne $text -and $text.Trim().Length -gt 0) {
      Write-Output $text
      return
    }
  }
}
try {
  $valuePattern = $focused.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
} catch {
  $valuePattern = $null
}
if ($null -ne $valuePattern) {
  $value = $valuePattern.Current.Value
  if ($null -ne $value -and $value.Trim().Length -gt 0) {
    Write-Output $value
    return
  }
}
"#,
        ])
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    Ok(normalize_windows_text_stdout(&stdout))
}

#[cfg(target_os = "windows")]
fn read_iaccessible_text() -> Result<Option<String>, String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$focused = [System.Windows.Automation.AutomationElement]::FocusedElement
if ($null -eq $focused) { return }
try {
  $legacy = $focused.GetCurrentPattern([System.Windows.Automation.LegacyIAccessiblePattern]::Pattern)
} catch {
  $legacy = $null
}
if ($null -eq $legacy) { return }
$value = $legacy.Current.Value
if ($null -ne $value -and $value.Trim().Length -gt 0) {
  Write-Output $value
  return
}
$name = $legacy.Current.Name
if ($null -ne $name -and $name.Trim().Length -gt 0) {
  Write-Output $name
  return
}
"#,
        ])
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    Ok(normalize_windows_text_stdout(&stdout))
}

#[cfg(target_os = "windows")]
fn read_active_app() -> Result<Option<ActiveApp>, String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class Win32 {
  [DllImport("user32.dll")]
  public static extern IntPtr GetForegroundWindow();

  [DllImport("user32.dll")]
  public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@
$hwnd = [Win32]::GetForegroundWindow()
if ($hwnd -eq [IntPtr]::Zero) { return }
$pid = 0
[Win32]::GetWindowThreadProcessId($hwnd, [ref]$pid) | Out-Null
if ($pid -eq 0) { return }
$process = Get-Process -Id $pid -ErrorAction SilentlyContinue
if ($null -eq $process) { return }
$name = $process.ProcessName
$path = $process.Path
Write-Output ("NAME:" + $name)
Write-Output ("PATH:" + $path)
"#,
        ])
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    Ok(parse_active_app_stdout(&stdout))
}

#[cfg(target_os = "windows")]
fn normalize_windows_text_stdout(stdout: &str) -> Option<String> {
    let text = stdout.replace("\r\n", "\n");
    let normalized = text.trim_end_matches(['\r', '\n']);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

#[cfg(target_os = "windows")]
fn parse_active_app_stdout(stdout: &str) -> Option<ActiveApp> {
    let mut name: Option<String> = None;
    let mut path: Option<String> = None;

    for line in stdout.lines() {
        if let Some(value) = line.strip_prefix("NAME:") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                name = Some(trimmed.to_string());
            }
        } else if let Some(value) = line.strip_prefix("PATH:") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                path = Some(trimmed.to_string());
            }
        }
    }

    let app_name = name?;
    let bundle_id = path.unwrap_or_else(|| format!("process://{}", app_name.to_lowercase()));
    Some(ActiveApp {
        bundle_id,
        name: app_name,
    })
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
    fn active_app_probe_does_not_panic() {
        let platform = WindowsPlatform::new();
        let _ = platform.active_app();
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
    fn normalizes_windows_text_stdout_and_strips_trailing_newline() {
        let raw = "line one\r\nline two\r\n";
        assert_eq!(
            normalize_windows_text_stdout(raw),
            Some("line one\nline two".to_string())
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn returns_none_when_windows_text_stdout_is_effectively_empty() {
        assert_eq!(normalize_windows_text_stdout("\r\n"), None);
        assert_eq!(normalize_windows_text_stdout(""), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_active_app_stdout_with_path() {
        let raw = "NAME:Code\nPATH:C:\\Program Files\\Microsoft VS Code\\Code.exe\n";
        let parsed = parse_active_app_stdout(raw).expect("active app");

        assert_eq!(parsed.name, "Code");
        assert_eq!(
            parsed.bundle_id,
            "C:\\Program Files\\Microsoft VS Code\\Code.exe"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_active_app_stdout_without_path_uses_process_fallback() {
        let raw = "NAME:Notepad\nPATH:\n";
        let parsed = parse_active_app_stdout(raw).expect("active app");

        assert_eq!(parsed.name, "Notepad");
        assert_eq!(parsed.bundle_id, "process://notepad");
    }
}
