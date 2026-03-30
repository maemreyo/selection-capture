#[cfg(target_os = "macos")]
use crate::ax_observer_drain_events_for_monitor;
#[cfg(feature = "rich-content")]
pub(crate) use crate::macos_ax::try_selected_rtf_by_ax;
use crate::macos_ax::{
    get_selected_text_by_ax, run_clipboard_borrow_script, ClipboardBorrowResult,
};
use crate::traits::{CapturePlatform, MonitorPlatform};
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};
#[cfg(target_os = "macos")]
use crate::AxObserverBridge;
use accessibility_ng::{AXObserver, AXUIElement};
use accessibility_sys_ng::{
    kAXFocusedUIElementChangedNotification, kAXSelectedTextChangedNotification, pid_t,
    AXObserverRef, AXUIElementRef,
};
use active_win_pos_rs::get_active_window;
use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};
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
    poll_interval: Duration,
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
        *self
            .cleanup_status
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = CleanupStatus::Clean;
    }

    fn mark_cleanup_failed(&self) {
        *self
            .cleanup_status
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = CleanupStatus::ClipboardRestoreFailed;
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

    pub fn poll_interval(&self) -> Duration {
        self.poll_interval
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
        let mut guard = self
            .cleanup_status
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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

// SAFETY: This function is registered as a Core Foundation AXObserver callback via
// `AXObserver::add_notification`. Core Foundation guarantees it is dispatched on the
// run-loop thread associated with the observer — the same thread that calls
// `NativeObserverRuntime::poll_once`. `AxObserverBridge::push_event` is `Send`-safe
// (it acquires its own `Mutex` internally), so calling it from this C callback is sound.
// No mutable references to observer state are held across this call boundary.
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
        // SAFETY: `CFRunLoop::run_in_mode` is called with a zero timeout so it drains
        // pending callbacks without blocking. This is invoked only on the thread that
        // owns the run loop (the same thread that called `AXObserver::start` and
        // registered this callback), and `kCFRunLoopDefaultMode` is a valid, non-null
        // static constant provided by Core Foundation.
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

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
