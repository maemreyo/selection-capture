use crate::traits::{CapturePlatform, MonitorPlatform};
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};
use accessibility_ng::{AXAttribute, AXUIElement};
use accessibility_sys_ng::{kAXFocusedUIElementAttribute, kAXSelectedTextAttribute};
use active_win_pos_rs::get_active_window;
use core_foundation::string::CFString;
use macos_accessibility_client::accessibility::application_is_trusted;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

pub struct MacOSPlatform {
    cleanup_status: Mutex<CleanupStatus>,
}

pub struct MacOSSelectionMonitor {
    last_emitted: Mutex<Option<String>>,
    native_event_queue: Mutex<VecDeque<String>>,
    native_events_dropped: Mutex<u64>,
    native_queue_capacity: usize,
    pub poll_interval: Duration,
    backend: MacOSMonitorBackend,
    native_observer_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacOSNativeEventSource {
    AXObserverSelectionChanged,
    AXObserverFocusedElementChanged,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacOSMonitorBackend {
    Polling,
    NativeObserverPreferred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MacOSSelectionMonitorOptions {
    pub poll_interval: Duration,
    pub backend: MacOSMonitorBackend,
    pub native_queue_capacity: usize,
}

impl Default for MacOSPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MacOSSelectionMonitor {
    fn default() -> Self {
        Self::new_with_options(MacOSSelectionMonitorOptions::default())
    }
}

impl Default for MacOSSelectionMonitorOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(120),
            backend: MacOSMonitorBackend::Polling,
            native_queue_capacity: 256,
        }
    }
}

impl MacOSPlatform {
    pub fn new() -> Self {
        Self {
            cleanup_status: Mutex::new(CleanupStatus::Clean),
        }
    }

    fn reset_cleanup(&self) {
        *self.cleanup_status.lock().unwrap() = CleanupStatus::Clean;
    }

    fn mark_cleanup_failed(&self) {
        *self.cleanup_status.lock().unwrap() = CleanupStatus::ClipboardRestoreFailed;
    }

    fn active_app_inner(&self) -> Option<ActiveApp> {
        let window = get_active_window().ok()?;
        Some(ActiveApp {
            bundle_id: bundle_id_from_process_path(&window.process_path),
            name: window.app_name,
        })
    }

    fn attempt_ax_selected_text(&self) -> PlatformAttemptResult {
        if !application_is_trusted() {
            return PlatformAttemptResult::PermissionDenied;
        }

        match get_selected_text_by_ax() {
            Ok(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    PlatformAttemptResult::EmptySelection
                } else {
                    PlatformAttemptResult::Success(trimmed.to_string())
                }
            }
            Err(_) => PlatformAttemptResult::Unavailable,
        }
    }

    fn attempt_clipboard_borrow(&self) -> PlatformAttemptResult {
        if !application_is_trusted() {
            return PlatformAttemptResult::PermissionDenied;
        }

        match run_clipboard_borrow_script() {
            Ok(ClipboardBorrowResult::Success(text)) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    PlatformAttemptResult::EmptySelection
                } else {
                    PlatformAttemptResult::Success(trimmed.to_string())
                }
            }
            Ok(ClipboardBorrowResult::Empty) => PlatformAttemptResult::EmptySelection,
            Ok(ClipboardBorrowResult::RestoreFailed) => {
                self.mark_cleanup_failed();
                PlatformAttemptResult::ClipboardBorrowAmbiguous
            }
            Err(stderr) => {
                if is_permission_error(&stderr) {
                    PlatformAttemptResult::PermissionDenied
                } else {
                    PlatformAttemptResult::Unavailable
                }
            }
        }
    }
}

impl MacOSSelectionMonitor {
    pub fn new(poll_interval: Duration) -> Self {
        Self::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval,
            backend: MacOSMonitorBackend::Polling,
            native_queue_capacity: 256,
        })
    }

    pub fn new_with_options(options: MacOSSelectionMonitorOptions) -> Self {
        let native_observer_active = matches!(
            options.backend,
            MacOSMonitorBackend::NativeObserverPreferred
        ) && try_enable_native_observer();

        Self {
            last_emitted: Mutex::new(None),
            native_event_queue: Mutex::new(VecDeque::new()),
            native_events_dropped: Mutex::new(0),
            native_queue_capacity: options.native_queue_capacity.max(1),
            poll_interval: options.poll_interval,
            backend: options.backend,
            native_observer_active,
        }
    }

    pub fn backend(&self) -> MacOSMonitorBackend {
        self.backend
    }

    pub fn native_observer_active(&self) -> bool {
        self.native_observer_active
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

    pub fn ingest_native_observer_payload(
        &self,
        _source: MacOSNativeEventSource,
        payload: &str,
    ) -> bool {
        self.enqueue_native_selection_event(payload)
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
            .map(|value| *value)
            .unwrap_or(0)
    }

    fn next_selection_text(&self) -> Option<String> {
        if self.native_observer_active {
            if let Some(event) = self.next_selection_text_native() {
                return Some(event);
            }
        }
        self.next_selection_text_polling()
    }

    fn next_selection_text_polling(&self) -> Option<String> {
        if !application_is_trusted() {
            return None;
        }
        let text = get_selected_text_by_ax().ok()?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        self.emit_if_new(trimmed.to_string())
    }

    fn next_selection_text_native(&self) -> Option<String> {
        let next = self.native_event_queue.lock().ok()?.pop_front()?;
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
}

impl CapturePlatform for MacOSPlatform {
    fn active_app(&self) -> Option<ActiveApp> {
        self.active_app_inner()
    }

    fn attempt(&self, method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
        self.reset_cleanup();
        match method {
            CaptureMethod::AccessibilityPrimary => self.attempt_ax_selected_text(),
            CaptureMethod::AccessibilityRange => PlatformAttemptResult::Unavailable,
            CaptureMethod::ClipboardBorrow => self.attempt_clipboard_borrow(),
            CaptureMethod::SyntheticCopy => PlatformAttemptResult::Unavailable,
        }
    }

    fn cleanup(&self) -> CleanupStatus {
        let mut guard = self.cleanup_status.lock().unwrap();
        let status = *guard;
        *guard = CleanupStatus::Clean;
        status
    }
}

impl MonitorPlatform for MacOSSelectionMonitor {
    fn next_selection_change(&self) -> Option<String> {
        self.next_selection_text()
    }
}

fn try_enable_native_observer() -> bool {
    false
}

fn get_selected_text_by_ax() -> Result<String, String> {
    let system_element = AXUIElement::system_wide();
    let Some(selected_element) = system_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXFocusedUIElementAttribute,
        )))
        .map(|element| element.downcast_into::<AXUIElement>())
        .ok()
        .flatten()
    else {
        return Err("No focused UI element".to_string());
    };

    let Some(selected_text) = selected_element
        .attribute(&AXAttribute::new(&CFString::from_static_string(
            kAXSelectedTextAttribute,
        )))
        .map(|text| text.downcast_into::<CFString>())
        .ok()
        .flatten()
    else {
        return Err("No selected text".to_string());
    };

    Ok(selected_text.to_string())
}

#[derive(Debug, PartialEq, Eq)]
enum ClipboardBorrowResult {
    Success(String),
    Empty,
    RestoreFailed,
}

fn run_clipboard_borrow_script() -> Result<ClipboardBorrowResult, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(APPLE_SCRIPT)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let mut lines = stdout.lines();
    match lines.next().unwrap_or_default() {
        "STATUS:OK" => Ok(ClipboardBorrowResult::Success(
            lines.collect::<Vec<_>>().join("\n"),
        )),
        "STATUS:EMPTY" => Ok(ClipboardBorrowResult::Empty),
        "STATUS:RESTORE_FAILED" => Ok(ClipboardBorrowResult::RestoreFailed),
        _ => Ok(ClipboardBorrowResult::Empty),
    }
}

fn is_permission_error(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("not authorized")
        || lower.contains("not permitted")
        || lower.contains("assistive access")
        || lower.contains("(-1743)")
        || lower.contains("(-1719)")
}

fn bundle_id_from_process_path(path: &Path) -> String {
    if let Some(bundle_root) = app_bundle_root(path) {
        if let Some(bundle_id) = read_bundle_identifier(&bundle_root) {
            return bundle_id;
        }
        return bundle_root.to_string_lossy().to_string();
    }

    PathBuf::from(path).to_string_lossy().to_string()
}

fn app_bundle_root(path: &Path) -> Option<PathBuf> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if candidate
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.ends_with(".app"))
            .unwrap_or(false)
        {
            return Some(candidate.to_path_buf());
        }
        current = candidate.parent();
    }
    None
}

fn read_bundle_identifier(bundle_root: &Path) -> Option<String> {
    let output = Command::new("mdls")
        .arg("-name")
        .arg("kMDItemCFBundleIdentifier")
        .arg("-raw")
        .arg(bundle_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let value = stdout.trim();
    if value.is_empty() || value == "(null)" {
        None
    } else {
        Some(value.to_string())
    }
}

const APPLE_SCRIPT: &str = r#"
use AppleScript version "2.4"
use scripting additions
use framework "Foundation"
use framework "AppKit"

set savedAlertVolume to alert volume of (get volume settings)
set savedClipboard to the clipboard

set thePasteboard to current application's NSPasteboard's generalPasteboard()
set theCount to thePasteboard's changeCount()

tell application "System Events"
    set volume alert volume 0
end tell

tell application "System Events" to keystroke "c" using {command down}
delay 0.12

tell application "System Events"
    set volume alert volume savedAlertVolume
end tell

if thePasteboard's changeCount() is theCount then
    try
        set the clipboard to savedClipboard
        return "STATUS:EMPTY"
    on error
        return "STATUS:RESTORE_FAILED"
    end try
end if

set theSelectedText to the clipboard

try
    set the clipboard to savedClipboard
on error
    return "STATUS:RESTORE_FAILED"
end try

return "STATUS:OK" & linefeed & theSelectedText
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_root_uses_app_ancestor_when_present() {
        let path = PathBuf::from("/Applications/Test.app/Contents/MacOS/Test");
        let bundle = bundle_id_from_process_path(&path);
        assert_eq!(bundle, "/Applications/Test.app");
    }

    #[test]
    fn bundle_root_falls_back_to_process_path() {
        let path = PathBuf::from("/usr/local/bin/code");
        let bundle = bundle_id_from_process_path(&path);
        assert_eq!(bundle, "/usr/local/bin/code");
    }

    #[test]
    fn selection_monitor_default_poll_interval_is_stable() {
        let monitor = MacOSSelectionMonitor::default();
        assert_eq!(monitor.poll_interval, Duration::from_millis(120));
        assert_eq!(monitor.backend(), MacOSMonitorBackend::Polling);
        assert!(!monitor.native_observer_active());
    }

    #[test]
    fn selection_monitor_native_preferred_falls_back_to_polling_path() {
        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(75),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 256,
        });

        assert_eq!(monitor.poll_interval, Duration::from_millis(75));
        assert_eq!(
            monitor.backend(),
            MacOSMonitorBackend::NativeObserverPreferred
        );
        assert!(!monitor.native_observer_active());
    }

    #[test]
    fn selection_monitor_native_queue_ignores_empty_events() {
        let monitor = MacOSSelectionMonitor::default();

        assert!(!monitor.enqueue_native_selection_event(""));
        assert!(!monitor.enqueue_native_selection_event("   "));
    }

    #[test]
    fn selection_monitor_native_queue_emits_in_order_and_dedups() {
        let mut monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(75),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 256,
        });
        monitor.native_observer_active = true;

        assert!(monitor.enqueue_native_selection_event("first"));
        assert!(!monitor.enqueue_native_selection_event("first"));
        assert!(monitor.enqueue_native_selection_event("second"));

        assert_eq!(monitor.next_selection_text(), Some("first".to_string()));
        assert_eq!(monitor.next_selection_text(), Some("second".to_string()));
        assert_eq!(monitor.next_selection_text(), None);
    }

    #[test]
    fn selection_monitor_native_queue_applies_capacity_and_tracks_drops() {
        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(50),
            backend: MacOSMonitorBackend::Polling,
            native_queue_capacity: 2,
        });

        assert!(monitor.enqueue_native_selection_event("a"));
        assert!(monitor.enqueue_native_selection_event("b"));
        assert!(monitor.enqueue_native_selection_event("c"));

        assert_eq!(monitor.native_queue_depth(), 2);
        assert_eq!(monitor.native_events_dropped(), 1);
    }

    #[test]
    fn selection_monitor_native_queue_batch_enqueue_counts_accepts() {
        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(50),
            backend: MacOSMonitorBackend::Polling,
            native_queue_capacity: 4,
        });

        let accepted =
            monitor.enqueue_native_selection_events(vec!["one", "one", " ", "two", "three"]);

        assert_eq!(accepted, 3);
        assert_eq!(monitor.native_queue_depth(), 3);
        assert_eq!(monitor.native_events_dropped(), 0);
    }

    #[test]
    fn selection_monitor_native_observer_payload_uses_same_backpressure_path() {
        let mut monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(50),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 2,
        });
        monitor.native_observer_active = true;

        assert!(monitor.ingest_native_observer_payload(
            MacOSNativeEventSource::AXObserverSelectionChanged,
            "first"
        ));
        assert!(monitor.ingest_native_observer_payload(
            MacOSNativeEventSource::AXObserverSelectionChanged,
            "second"
        ));
        assert!(monitor.ingest_native_observer_payload(
            MacOSNativeEventSource::AXObserverSelectionChanged,
            "third"
        ));

        assert_eq!(monitor.native_events_dropped(), 1);
        assert_eq!(monitor.next_selection_text(), Some("second".to_string()));
        assert_eq!(monitor.next_selection_text(), Some("third".to_string()));
        assert_eq!(monitor.next_selection_text(), None);
    }
}
