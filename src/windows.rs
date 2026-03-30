#[cfg(all(feature = "rich-content", target_os = "windows"))]
use crate::rich_convert::plain_text_to_minimal_rtf;
use crate::traits::{CapturePlatform, MonitorPlatform};
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};
use crate::windows_observer::{
    drain_events_for_monitor as windows_observer_drain_events_for_monitor, WindowsObserverBridge,
};
use crate::windows_runtime_adapter::install_default_windows_runtime_adapter_if_absent;
use crate::windows_subscriber::ensure_windows_native_subscriber_hook_installed;
use std::collections::VecDeque;
#[cfg(target_os = "windows")]
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct WindowsPlatform;

pub struct WindowsSelectionMonitor {
    last_emitted: Mutex<Option<String>>,
    native_event_queue: Mutex<VecDeque<String>>,
    native_events_dropped: Mutex<u64>,
    native_queue_capacity: usize,
    pub poll_interval: Duration,
    backend: WindowsMonitorBackend,
    native_observer_attached: bool,
    native_event_pump: Option<WindowsNativeEventPump>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsMonitorBackend {
    Polling,
    NativeEventPreferred,
}

#[derive(Clone, Copy, Debug)]
pub struct WindowsSelectionMonitorOptions {
    pub poll_interval: Duration,
    pub backend: WindowsMonitorBackend,
    pub native_queue_capacity: usize,
    pub native_event_pump: Option<WindowsNativeEventPump>,
}

pub type WindowsNativeEventPump = fn() -> Vec<String>;

trait WindowsBackend {
    fn attempt_ui_automation(&self) -> PlatformAttemptResult;
    fn attempt_iaccessible(&self) -> PlatformAttemptResult;
    fn attempt_clipboard(&self) -> PlatformAttemptResult;
    fn attempt_synthetic_copy(&self) -> PlatformAttemptResult;
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

    fn attempt_synthetic_copy(&self) -> PlatformAttemptResult {
        #[cfg(target_os = "windows")]
        {
            match synthetic_copy_capture_text() {
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
            CaptureMethod::ClipboardBorrow => backend.attempt_clipboard(),
            CaptureMethod::SyntheticCopy => backend.attempt_synthetic_copy(),
        }
    }
}

impl Default for WindowsSelectionMonitor {
    fn default() -> Self {
        Self::new_with_options(WindowsSelectionMonitorOptions::default())
    }
}

impl Default for WindowsSelectionMonitorOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(120),
            backend: WindowsMonitorBackend::Polling,
            native_queue_capacity: 256,
            native_event_pump: None,
        }
    }
}

impl WindowsSelectionMonitor {
    pub fn new(poll_interval: Duration) -> Self {
        Self::new_with_options(WindowsSelectionMonitorOptions {
            poll_interval,
            backend: WindowsMonitorBackend::Polling,
            native_queue_capacity: 256,
            native_event_pump: None,
        })
    }

    pub fn new_with_options(options: WindowsSelectionMonitorOptions) -> Self {
        if matches!(options.backend, WindowsMonitorBackend::NativeEventPreferred) {
            install_default_windows_runtime_adapter_if_absent();
            ensure_windows_native_subscriber_hook_installed();
        }
        let native_observer_attached =
            matches!(options.backend, WindowsMonitorBackend::NativeEventPreferred)
                && WindowsObserverBridge::acquire();
        let native_event_pump = if native_observer_attached {
            options
                .native_event_pump
                .or(Some(windows_observer_drain_events_for_monitor))
        } else {
            options.native_event_pump
        };

        Self {
            last_emitted: Mutex::new(None),
            native_event_queue: Mutex::new(VecDeque::new()),
            native_events_dropped: Mutex::new(0),
            native_queue_capacity: options.native_queue_capacity.max(1),
            poll_interval: options.poll_interval,
            backend: options.backend,
            native_observer_attached,
            native_event_pump,
        }
    }

    pub fn backend(&self) -> WindowsMonitorBackend {
        self.backend
    }

    pub fn enqueue_native_selection_event<T>(&self, text: T) -> bool
    where
        T: Into<String>,
    {
        let text = text.into();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        if let Ok(mut queue) = self.native_event_queue.lock() {
            if queue.back().map(|s| s == trimmed).unwrap_or(false) {
                return false;
            }
            if queue.len() >= self.native_queue_capacity {
                queue.pop_front();
                if let Ok(mut dropped) = self.native_events_dropped.lock() {
                    *dropped += 1;
                }
            }
            queue.push_back(trimmed.to_string());
            return true;
        }
        false
    }

    pub fn enqueue_native_selection_events<I, T>(&self, events: I) -> usize
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let mut accepted = 0usize;
        for event in events {
            if self.enqueue_native_selection_event(event.into()) {
                accepted += 1;
            }
        }
        accepted
    }

    pub fn native_queue_depth(&self) -> usize {
        self.native_event_queue
            .lock()
            .map(|queue| queue.len())
            .unwrap_or(0)
    }

    pub fn native_events_dropped(&self) -> u64 {
        self.native_events_dropped
            .lock()
            .map(|dropped| *dropped)
            .unwrap_or(0)
    }

    pub fn poll_native_event_pump_once(&self) -> usize {
        let Some(pump) = self.native_event_pump else {
            return 0;
        };
        self.enqueue_native_selection_events(pump())
    }

    fn next_selection_text(&self) -> Option<String> {
        if matches!(self.backend, WindowsMonitorBackend::NativeEventPreferred) {
            let _ = self.poll_native_event_pump_once();
            if let Some(next) = self.native_event_queue.lock().ok()?.pop_front() {
                return self.emit_if_new(next);
            }
        }
        let next = self.read_selection_text()?;
        self.emit_if_new(next)
    }

    fn emit_if_new(&self, next: String) -> Option<String> {
        let mut last = self.last_emitted.lock().ok()?;
        if last.as_ref() == Some(&next) {
            return None;
        }
        *last = Some(next.clone());
        Some(next)
    }

    fn read_selection_text(&self) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            let atspi = read_uia_text().ok().flatten();
            if let Some(next) = atspi {
                let trimmed = next.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }

            let legacy = read_iaccessible_text().ok().flatten();
            if let Some(next) = legacy {
                let trimmed = next.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            None
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
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

impl MonitorPlatform for WindowsSelectionMonitor {
    fn next_selection_change(&self) -> Option<String> {
        self.next_selection_text()
    }
}

impl Drop for WindowsSelectionMonitor {
    fn drop(&mut self) {
        if self.native_observer_attached {
            let _ = WindowsObserverBridge::release();
        }
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

#[cfg(all(feature = "rich-content", target_os = "windows"))]
pub(crate) fn try_selected_rtf_by_uia() -> Option<String> {
    let text = read_uia_text().ok().flatten()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(plain_text_to_minimal_rtf(trimmed))
    }
}

pub(crate) fn windows_default_runtime_event_source() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        return read_uia_text().ok().flatten();
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn synthetic_copy_capture_text() -> Result<Option<String>, String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-STA",
            "-Command",
            r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class Win32 {
  [DllImport("user32.dll")]
  public static extern IntPtr GetForegroundWindow();
}
"@
$hwnd = [Win32]::GetForegroundWindow()
if ($hwnd -eq [IntPtr]::Zero) { return }

$original = $null
$hasOriginal = $false
try {
  $original = Get-Clipboard -Raw -ErrorAction Stop
  $hasOriginal = $true
} catch {}

[System.Windows.Forms.SendKeys]::SendWait("^c")
Start-Sleep -Milliseconds 90

$captured = $null
try {
  $captured = Get-Clipboard -Raw -ErrorAction Stop
} catch {}

if ($hasOriginal) {
  try {
    Set-Clipboard -Value $original
  } catch {}
}

if ($null -ne $captured) {
  Write-Output $captured
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
    use crate::windows_observer::windows_observer_test_lock;
    use crate::windows_subscriber::windows_native_subscriber_stats;
    use crate::WindowsObserverBridge;

    #[derive(Debug)]
    struct StubBackend {
        ui_automation: PlatformAttemptResult,
        iaccessible: PlatformAttemptResult,
        clipboard: PlatformAttemptResult,
        synthetic_copy: PlatformAttemptResult,
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

        fn attempt_synthetic_copy(&self) -> PlatformAttemptResult {
            self.synthetic_copy.clone()
        }
    }

    #[test]
    fn constructor_builds_stub_platform() {
        let platform = WindowsPlatform::new();
        let _ = platform;
    }

    #[test]
    fn selection_monitor_default_poll_interval_is_stable() {
        let monitor = WindowsSelectionMonitor::default();
        assert_eq!(monitor.poll_interval, Duration::from_millis(120));
        assert_eq!(monitor.backend(), WindowsMonitorBackend::Polling);
    }

    #[test]
    fn selection_monitor_emits_only_new_values() {
        let monitor = WindowsSelectionMonitor::new(Duration::from_millis(10));
        assert_eq!(
            monitor.emit_if_new("first".to_string()),
            Some("first".to_string())
        );
        assert_eq!(monitor.emit_if_new("first".to_string()), None);
        assert_eq!(
            monitor.emit_if_new("second".to_string()),
            Some("second".to_string())
        );
    }

    #[test]
    fn selection_monitor_native_preferred_uses_event_pump_when_available() {
        let _guard = windows_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        fn pump() -> Vec<String> {
            vec![
                "  native a ".to_string(),
                "native a".to_string(),
                "native b".to_string(),
            ]
        }

        let monitor = WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
            poll_interval: Duration::from_millis(10),
            backend: WindowsMonitorBackend::NativeEventPreferred,
            native_queue_capacity: 8,
            native_event_pump: Some(pump),
        });

        assert_eq!(
            monitor.next_selection_change(),
            Some("native a".to_string())
        );
        assert_eq!(
            monitor.next_selection_change(),
            Some("native b".to_string())
        );
    }

    #[test]
    fn selection_monitor_native_preferred_uses_bridge_drain_by_default() {
        let _guard = windows_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let _ = WindowsObserverBridge::stop();
        let _ = WindowsObserverBridge::start();
        assert!(WindowsObserverBridge::push_event("bridge one"));
        assert!(WindowsObserverBridge::push_event("bridge two"));

        let monitor = WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
            poll_interval: Duration::from_millis(10),
            backend: WindowsMonitorBackend::NativeEventPreferred,
            native_queue_capacity: 8,
            native_event_pump: None,
        });

        assert_eq!(
            monitor.next_selection_change(),
            Some("bridge one".to_string())
        );
        assert_eq!(
            monitor.next_selection_change(),
            Some("bridge two".to_string())
        );
        assert!(WindowsObserverBridge::is_active());
        let _ = WindowsObserverBridge::stop();
    }

    #[test]
    fn selection_monitor_native_preferred_releases_bridge_on_drop() {
        let _guard = windows_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let _ = WindowsObserverBridge::stop();

        {
            let _monitor =
                WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
                    poll_interval: Duration::from_millis(10),
                    backend: WindowsMonitorBackend::NativeEventPreferred,
                    native_queue_capacity: 8,
                    native_event_pump: None,
                });
            assert!(WindowsObserverBridge::is_active());
        }

        assert!(!WindowsObserverBridge::is_active());
    }

    #[test]
    fn selection_monitor_native_preferred_transitions_subscriber_manager_lifecycle() {
        let _guard = windows_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let _ = WindowsObserverBridge::stop();
        let before = windows_native_subscriber_stats();

        {
            let _monitor =
                WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
                    poll_interval: Duration::from_millis(10),
                    backend: WindowsMonitorBackend::NativeEventPreferred,
                    native_queue_capacity: 8,
                    native_event_pump: None,
                });
            let during = windows_native_subscriber_stats();
            assert!(during.active);
            assert_eq!(during.starts, before.starts + 1);
        }

        let after = windows_native_subscriber_stats();
        assert!(after.stops > before.stops);
    }

    #[test]
    fn selection_monitor_native_preferred_applies_queue_capacity() {
        let _guard = windows_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let monitor = WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
            poll_interval: Duration::from_millis(10),
            backend: WindowsMonitorBackend::NativeEventPreferred,
            native_queue_capacity: 2,
            native_event_pump: None,
        });
        let accepted = monitor.enqueue_native_selection_events(vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ]);
        assert_eq!(accepted, 3);
        assert_eq!(monitor.native_queue_depth(), 2);
        assert_eq!(monitor.native_events_dropped(), 1);
        assert_eq!(monitor.next_selection_change(), Some("second".to_string()));
        assert_eq!(monitor.next_selection_change(), Some("third".to_string()));
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
            synthetic_copy: PlatformAttemptResult::Unavailable,
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
            synthetic_copy: PlatformAttemptResult::Success("synthetic".into()),
        };

        assert_eq!(
            WindowsPlatform::dispatch_attempt(&backend, CaptureMethod::ClipboardBorrow),
            PlatformAttemptResult::Success("clipboard".into())
        );
    }

    #[test]
    fn dispatches_synthetic_copy_to_synthetic_copy_attempt() {
        let backend = StubBackend {
            ui_automation: PlatformAttemptResult::Unavailable,
            iaccessible: PlatformAttemptResult::Unavailable,
            clipboard: PlatformAttemptResult::Success("clipboard".into()),
            synthetic_copy: PlatformAttemptResult::Success("synthetic".into()),
        };

        assert_eq!(
            WindowsPlatform::dispatch_attempt(&backend, CaptureMethod::SyntheticCopy),
            PlatformAttemptResult::Success("synthetic".into())
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
