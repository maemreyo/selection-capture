#[cfg(target_os = "macos")]
use crate::ax_observer_drain_events_for_monitor;
use crate::traits::{CapturePlatform, MonitorPlatform};
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};
#[cfg(target_os = "macos")]
use crate::AxObserverBridge;
use accessibility_ng::{AXAttribute, AXObserver, AXUIElement};
#[cfg(feature = "rich-content")]
use accessibility_sys_ng::kAXRTFForRangeParameterizedAttribute;
use accessibility_sys_ng::{
    kAXFocusedUIElementAttribute, kAXFocusedUIElementChangedNotification, kAXSelectedTextAttribute,
    kAXSelectedTextChangedNotification, pid_t, AXObserverRef, AXUIElementRef,
};
use active_win_pos_rs::get_active_window;
#[cfg(feature = "rich-content")]
use core_foundation::base::CFType;
#[cfg(feature = "rich-content")]
use core_foundation::data::CFData;
use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};
use core_foundation::string::CFString;
use macos_accessibility_client::accessibility::application_is_trusted;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
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
    native_observer_attached: bool,
    native_observer_runtime: Mutex<Option<NativeObserverRuntime>>,
    native_observer_last_runtime_pid: Mutex<Option<u64>>,
    native_observer_stats: Mutex<MacOSNativeObserverStats>,
    active_pid_provider: Option<MacOSActivePidProvider>,
    native_event_pump: Option<MacOSNativeEventPump>,
}

struct NativeObserverRuntime {
    pid: u64,
    observer: AXObserver,
    app_element: AXUIElement,
    selected_text_registered: bool,
    focused_element_registered: bool,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MacOSNativeObserverStats {
    pub attach_attempts: u64,
    pub attach_successes: u64,
    pub attach_failures: u64,
    pub skipped_same_pid_retries: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct MacOSSelectionMonitorOptions {
    pub poll_interval: Duration,
    pub backend: MacOSMonitorBackend,
    pub native_queue_capacity: usize,
    pub native_event_pump: Option<MacOSNativeEventPump>,
    pub active_pid_provider: Option<MacOSActivePidProvider>,
}

pub type MacOSNativeEventPump = fn() -> Vec<String>;
pub type MacOSActivePidProvider = fn() -> Option<u64>;

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
            native_event_pump: None,
            active_pid_provider: None,
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
            native_event_pump: None,
            active_pid_provider: None,
        })
    }

    pub fn new_with_options(options: MacOSSelectionMonitorOptions) -> Self {
        let native_observer_active = matches!(
            options.backend,
            MacOSMonitorBackend::NativeObserverPreferred
        ) && try_enable_native_observer();
        let native_event_pump = if native_observer_active {
            options
                .native_event_pump
                .or(Some(ax_observer_drain_events_for_monitor))
        } else {
            options.native_event_pump
        };

        let initial_active_pid = if native_observer_active {
            current_active_pid(options.active_pid_provider)
        } else {
            None
        };
        let initial_runtime = initial_active_pid.and_then(NativeObserverRuntime::try_new_for_pid);
        let initial_last_runtime_pid = if initial_runtime.is_some() {
            initial_runtime.as_ref().map(|runtime| runtime.pid)
        } else {
            initial_active_pid
        };
        let mut initial_stats = MacOSNativeObserverStats::default();
        if initial_active_pid.is_some() {
            initial_stats.attach_attempts = 1;
            if initial_runtime.is_some() {
                initial_stats.attach_successes = 1;
            } else {
                initial_stats.attach_failures = 1;
            }
        }

        Self {
            last_emitted: Mutex::new(None),
            native_event_queue: Mutex::new(VecDeque::new()),
            native_events_dropped: Mutex::new(0),
            native_queue_capacity: options.native_queue_capacity.max(1),
            poll_interval: options.poll_interval,
            backend: options.backend,
            native_observer_active,
            native_observer_attached: native_observer_active,
            native_observer_runtime: Mutex::new(initial_runtime),
            native_observer_last_runtime_pid: Mutex::new(initial_last_runtime_pid),
            native_observer_stats: Mutex::new(initial_stats),
            active_pid_provider: options.active_pid_provider,
            native_event_pump,
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

    pub fn native_observer_stats(&self) -> MacOSNativeObserverStats {
        self.native_observer_stats
            .lock()
            .map(|stats| *stats)
            .unwrap_or_default()
    }

    pub fn poll_native_event_pump_once(&self) -> usize {
        self.refresh_native_observer_runtime();
        if let Ok(runtime) = self.native_observer_runtime.lock() {
            if let Some(runtime) = runtime.as_ref() {
                runtime.poll_once();
            }
        }
        let Some(pump) = self.native_event_pump else {
            return 0;
        };
        let events = pump();
        self.enqueue_native_selection_events(events)
    }

    fn refresh_native_observer_runtime(&self) {
        if !self.native_observer_active {
            return;
        }

        let Some(active_pid) = current_active_pid(self.active_pid_provider) else {
            return;
        };

        let Ok(mut runtime) = self.native_observer_runtime.lock() else {
            return;
        };
        let Ok(mut last_runtime_pid) = self.native_observer_last_runtime_pid.lock() else {
            return;
        };

        if runtime
            .as_ref()
            .map(|existing| existing.pid == active_pid)
            .unwrap_or(false)
        {
            *last_runtime_pid = Some(active_pid);
            return;
        }

        if *last_runtime_pid == Some(active_pid) {
            if let Ok(mut stats) = self.native_observer_stats.lock() {
                stats.skipped_same_pid_retries += 1;
            }
            return;
        }

        if let Ok(mut stats) = self.native_observer_stats.lock() {
            stats.attach_attempts += 1;
        }
        *runtime = None;
        *runtime = NativeObserverRuntime::try_new_for_pid(active_pid);
        if let Ok(mut stats) = self.native_observer_stats.lock() {
            if runtime.is_some() {
                stats.attach_successes += 1;
            } else {
                stats.attach_failures += 1;
            }
        }
        *last_runtime_pid = Some(active_pid);
    }

    fn next_selection_text(&self) -> Option<String> {
        if self.native_observer_active {
            let _ = self.poll_native_event_pump_once();
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

impl Drop for MacOSSelectionMonitor {
    fn drop(&mut self) {
        if let Ok(mut runtime) = self.native_observer_runtime.lock() {
            *runtime = None;
        }
        if let Ok(mut last_runtime_pid) = self.native_observer_last_runtime_pid.lock() {
            *last_runtime_pid = None;
        }
        if self.native_observer_attached {
            let _ = AxObserverBridge::release();
        }
    }
}

fn try_enable_native_observer() -> bool {
    #[cfg(test)]
    if let Some(forced) = forced_native_observer_activation() {
        return if forced {
            AxObserverBridge::acquire()
        } else {
            false
        };
    }

    if !application_is_trusted() {
        return false;
    }

    AxObserverBridge::acquire()
}

unsafe extern "C" fn native_observer_callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    _notification: core_foundation::string::CFStringRef,
    _refcon: *mut c_void,
) {
    if let Ok(text) = get_selected_text_by_ax() {
        let _ = AxObserverBridge::push_event(text);
    }
}

impl NativeObserverRuntime {
    fn try_new_for_pid(pid: u64) -> Option<Self> {
        let pid = i32::try_from(pid).ok()?;
        let mut observer = AXObserver::new(pid as pid_t, native_observer_callback).ok()?;
        let app_element = AXUIElement::application(pid as pid_t);

        let mut selected_text_registered = false;
        let mut focused_element_registered = false;

        if observer
            .add_notification(kAXSelectedTextChangedNotification, &app_element, 0usize)
            .is_ok()
        {
            selected_text_registered = true;
        }

        if observer
            .add_notification(kAXFocusedUIElementChangedNotification, &app_element, 0usize)
            .is_ok()
        {
            focused_element_registered = true;
        }

        if !selected_text_registered && !focused_element_registered {
            return None;
        }

        observer.start();

        Some(Self {
            pid: pid as u64,
            observer,
            app_element,
            selected_text_registered,
            focused_element_registered,
        })
    }

    fn poll_once(&self) {
        unsafe {
            let _ = CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, Duration::from_millis(0), true);
        }
    }
}

fn current_active_pid(provider: Option<MacOSActivePidProvider>) -> Option<u64> {
    if let Some(provider) = provider {
        return provider();
    }
    get_active_window().ok().map(|window| window.process_id)
}

impl Drop for NativeObserverRuntime {
    fn drop(&mut self) {
        if self.selected_text_registered {
            let _ = self
                .observer
                .remove_notification(kAXSelectedTextChangedNotification, &self.app_element);
        }
        if self.focused_element_registered {
            let _ = self
                .observer
                .remove_notification(kAXFocusedUIElementChangedNotification, &self.app_element);
        }
        self.observer.stop();
    }
}

#[cfg(test)]
static FORCED_NATIVE_OBSERVER_ACTIVATION: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static FORCED_NATIVE_OBSERVER_ACTIVATION_SET: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
fn force_native_observer_activation(value: Option<bool>) {
    match value {
        Some(enabled) => {
            FORCED_NATIVE_OBSERVER_ACTIVATION.store(enabled, Ordering::SeqCst);
            FORCED_NATIVE_OBSERVER_ACTIVATION_SET.store(true, Ordering::SeqCst);
        }
        None => {
            FORCED_NATIVE_OBSERVER_ACTIVATION_SET.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
fn forced_native_observer_activation() -> Option<bool> {
    if FORCED_NATIVE_OBSERVER_ACTIVATION_SET.load(Ordering::SeqCst) {
        Some(FORCED_NATIVE_OBSERVER_ACTIVATION.load(Ordering::SeqCst))
    } else {
        None
    }
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

#[cfg(feature = "rich-content")]
pub(crate) fn try_selected_rtf_by_ax() -> Option<String> {
    if !application_is_trusted() {
        return None;
    }
    get_selected_rtf_by_ax()
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(feature = "rich-content")]
fn get_selected_rtf_by_ax() -> Result<String, String> {
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

    let selected_range = selected_element
        .attribute(&AXAttribute::selected_text_range())
        .map_err(|_| "No selected text range".to_string())?;
    let range = selected_range
        .get_value::<core_foundation::base::CFRange>()
        .map_err(|_| "Invalid selected text range".to_string())?;
    if range.length <= 0 {
        return Err("No selected text range".to_string());
    }

    let attr = AXAttribute::<CFType>::new(&CFString::from_static_string(
        kAXRTFForRangeParameterizedAttribute,
    ));
    let rtf_data = selected_element
        .parameterized_attribute(&attr, &selected_range)
        .map(|value| value.downcast_into::<CFData>())
        .ok()
        .flatten()
        .ok_or_else(|| "No RTF for selected range".to_string())?;

    Ok(String::from_utf8_lossy(rtf_data.bytes()).into_owned())
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
    use crate::AxObserverBridge;
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    fn native_pump_batches() -> &'static Mutex<VecDeque<Vec<String>>> {
        static BATCHES: OnceLock<Mutex<VecDeque<Vec<String>>>> = OnceLock::new();
        BATCHES.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    fn active_pid_batches() -> &'static Mutex<VecDeque<Option<u64>>> {
        static BATCHES: OnceLock<Mutex<VecDeque<Option<u64>>>> = OnceLock::new();
        BATCHES.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    fn monitor_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn reset_native_observer_state_for_tests() {
        force_native_observer_activation(Some(false));
        let _ = AxObserverBridge::stop();
        let _ = AxObserverBridge::drain_events(usize::MAX);
        if let Ok(mut pids) = active_pid_batches().lock() {
            pids.clear();
        }
    }

    fn push_native_pump_batch(batch: Vec<String>) {
        native_pump_batches().lock().unwrap().push_back(batch);
    }

    fn push_active_pid_batch(batch: Option<u64>) {
        active_pid_batches().lock().unwrap().push_back(batch);
    }

    fn test_native_event_pump() -> Vec<String> {
        native_pump_batches()
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default()
    }

    fn test_active_pid_provider() -> Option<u64> {
        active_pid_batches().lock().unwrap().pop_front().flatten()
    }

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
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        let monitor = MacOSSelectionMonitor::default();
        assert_eq!(monitor.poll_interval, Duration::from_millis(120));
        assert_eq!(monitor.backend(), MacOSMonitorBackend::Polling);
        assert!(!monitor.native_observer_active());
    }

    #[test]
    fn selection_monitor_native_preferred_falls_back_to_polling_path() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(75),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 256,
            native_event_pump: None,
            active_pid_provider: None,
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
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        let monitor = MacOSSelectionMonitor::default();

        assert!(!monitor.enqueue_native_selection_event(""));
        assert!(!monitor.enqueue_native_selection_event("   "));
    }

    #[test]
    fn selection_monitor_native_queue_emits_in_order_and_dedups() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        let mut monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(75),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 256,
            native_event_pump: None,
            active_pid_provider: None,
        });
        monitor.native_observer_active = true;
        monitor.native_observer_attached = false;

        assert!(monitor.enqueue_native_selection_event("first"));
        assert!(!monitor.enqueue_native_selection_event("first"));
        assert!(monitor.enqueue_native_selection_event("second"));

        assert_eq!(monitor.next_selection_text(), Some("first".to_string()));
        assert_eq!(monitor.next_selection_text(), Some("second".to_string()));
        assert_eq!(monitor.next_selection_text(), None);
    }

    #[test]
    fn selection_monitor_native_queue_applies_capacity_and_tracks_drops() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(50),
            backend: MacOSMonitorBackend::Polling,
            native_queue_capacity: 2,
            native_event_pump: None,
            active_pid_provider: None,
        });

        assert!(monitor.enqueue_native_selection_event("a"));
        assert!(monitor.enqueue_native_selection_event("b"));
        assert!(monitor.enqueue_native_selection_event("c"));

        assert_eq!(monitor.native_queue_depth(), 2);
        assert_eq!(monitor.native_events_dropped(), 1);
    }

    #[test]
    fn selection_monitor_native_queue_batch_enqueue_counts_accepts() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(50),
            backend: MacOSMonitorBackend::Polling,
            native_queue_capacity: 4,
            native_event_pump: None,
            active_pid_provider: None,
        });

        let accepted =
            monitor.enqueue_native_selection_events(vec!["one", "one", " ", "two", "three"]);

        assert_eq!(accepted, 3);
        assert_eq!(monitor.native_queue_depth(), 3);
        assert_eq!(monitor.native_events_dropped(), 0);
    }

    #[test]
    fn selection_monitor_native_observer_payload_uses_same_backpressure_path() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        let mut monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(50),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 2,
            native_event_pump: None,
            active_pid_provider: None,
        });
        monitor.native_observer_active = true;
        monitor.native_observer_attached = false;

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

    #[test]
    fn selection_monitor_native_event_pump_feeds_queue_with_backpressure_rules() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        push_native_pump_batch(vec!["a".into(), "a".into(), "b".into()]);
        push_native_pump_batch(vec!["c".into()]);

        let mut monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(50),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 3,
            native_event_pump: Some(test_native_event_pump),
            active_pid_provider: None,
        });
        monitor.native_observer_active = true;
        monitor.native_observer_attached = false;

        assert_eq!(monitor.next_selection_text(), Some("a".to_string()));
        assert_eq!(monitor.next_selection_text(), Some("b".to_string()));
        assert_eq!(monitor.next_selection_text(), Some("c".to_string()));
        assert_eq!(monitor.next_selection_text(), None);
        assert_eq!(monitor.native_events_dropped(), 0);
    }

    #[test]
    fn selection_monitor_native_preferred_uses_ax_observer_bridge_pump_by_default() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        force_native_observer_activation(Some(true));
        let _ = AxObserverBridge::stop();
        let _ = AxObserverBridge::drain_events(usize::MAX);
        assert!(AxObserverBridge::start());

        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(75),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 4,
            native_event_pump: None,
            active_pid_provider: None,
        });

        assert!(monitor.native_observer_active());
        assert!(AxObserverBridge::push_event("bridge-a"));
        assert!(AxObserverBridge::push_event("bridge-b"));
        assert_eq!(monitor.next_selection_text(), Some("bridge-a".to_string()));
        assert_eq!(monitor.next_selection_text(), Some("bridge-b".to_string()));
        assert_eq!(monitor.next_selection_text(), None);
    }

    #[test]
    fn selection_monitor_drop_releases_ax_observer_bridge() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        force_native_observer_activation(Some(true));
        let _ = AxObserverBridge::stop();
        let _ = AxObserverBridge::drain_events(usize::MAX);

        {
            let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
                poll_interval: Duration::from_millis(75),
                backend: MacOSMonitorBackend::NativeObserverPreferred,
                native_queue_capacity: 4,
                native_event_pump: None,
                active_pid_provider: None,
            });
            assert!(monitor.native_observer_active());
            assert!(AxObserverBridge::is_active());
        }

        assert!(!AxObserverBridge::is_active());
    }

    #[test]
    fn selection_monitor_native_runtime_rebuilds_on_focus_pid_change() {
        let _guard = monitor_test_lock()
            .lock()
            .expect("monitor test lock poisoned");
        reset_native_observer_state_for_tests();
        force_native_observer_activation(Some(true));
        push_active_pid_batch(Some(111));
        push_active_pid_batch(Some(222));
        push_active_pid_batch(Some(222));

        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(75),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 4,
            native_event_pump: None,
            active_pid_provider: Some(test_active_pid_provider),
        });

        let stats = monitor.native_observer_stats();
        assert_eq!(stats.attach_attempts, 1);
        assert_eq!(stats.attach_successes, 0);
        assert_eq!(stats.attach_failures, 1);
        assert_eq!(stats.skipped_same_pid_retries, 0);

        let _ = monitor.poll_native_event_pump_once();
        let stats = monitor.native_observer_stats();
        assert_eq!(stats.attach_attempts, 2);
        assert_eq!(stats.attach_successes, 0);
        assert_eq!(stats.attach_failures, 2);
        assert_eq!(stats.skipped_same_pid_retries, 0);

        let _ = monitor.poll_native_event_pump_once();
        let stats = monitor.native_observer_stats();
        assert_eq!(stats.attach_attempts, 2);
        assert_eq!(stats.attach_successes, 0);
        assert_eq!(stats.attach_failures, 2);
        assert_eq!(stats.skipped_same_pid_retries, 1);
    }
}
