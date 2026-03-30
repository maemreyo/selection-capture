use crate::linux_observer::{
    drain_events_for_monitor as linux_observer_drain_events_for_monitor, LinuxObserverBridge,
};
use crate::linux_runtime_adapter::install_default_linux_runtime_adapter_if_absent;
use crate::linux_subscriber::ensure_linux_native_subscriber_hook_installed;
#[cfg(all(feature = "rich-content", target_os = "linux"))]
use crate::rich_convert::plain_text_to_minimal_rtf;
use crate::traits::{CapturePlatform, MonitorPlatform};
use crate::types::{ActiveApp, CaptureMethod, CleanupStatus, PlatformAttemptResult};
use std::collections::VecDeque;
#[cfg(target_os = "linux")]
use std::env;
#[cfg(target_os = "linux")]
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct LinuxPlatform;

pub struct LinuxSelectionMonitor {
    last_emitted: Mutex<Option<String>>,
    native_event_queue: Mutex<VecDeque<String>>,
    native_events_dropped: Mutex<u64>,
    native_queue_capacity: usize,
    pub poll_interval: Duration,
    backend: LinuxMonitorBackend,
    native_observer_attached: bool,
    native_event_pump: Option<LinuxNativeEventPump>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinuxMonitorBackend {
    Polling,
    NativeEventPreferred,
}

#[derive(Clone, Copy, Debug)]
pub struct LinuxSelectionMonitorOptions {
    pub poll_interval: Duration,
    pub backend: LinuxMonitorBackend,
    pub native_queue_capacity: usize,
    pub native_event_pump: Option<LinuxNativeEventPump>,
}

pub type LinuxNativeEventPump = fn() -> Vec<String>;

trait LinuxBackend {
    fn attempt_atspi(&self) -> PlatformAttemptResult;
    fn attempt_x11_selection(&self) -> PlatformAttemptResult;
    fn attempt_clipboard(&self) -> PlatformAttemptResult;
}

#[derive(Debug, Default)]
struct DefaultLinuxBackend;

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LinuxSession {
    wayland: bool,
    x11: bool,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
struct LinuxCommandSpec {
    program: &'static str,
    args: &'static [&'static str],
}

#[cfg(any(target_os = "linux", test))]
fn detect_linux_session(wayland_display: Option<&str>, display: Option<&str>) -> LinuxSession {
    LinuxSession {
        wayland: wayland_display
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        x11: display
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
    }
}

#[cfg(any(target_os = "linux", test))]
fn clipboard_command_plan(session: LinuxSession) -> &'static [LinuxCommandSpec] {
    const WAYLAND_FIRST: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline"],
        },
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "clipboard"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--clipboard", "--output"],
        },
    ];
    const X11_FIRST: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "clipboard"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--clipboard", "--output"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline"],
        },
    ];
    const MIXED_DEFAULT: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "clipboard"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--clipboard", "--output"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--no-newline"],
        },
    ];

    if session.wayland && !session.x11 {
        &WAYLAND_FIRST
    } else if session.x11 && !session.wayland {
        &X11_FIRST
    } else {
        &MIXED_DEFAULT
    }
}

#[cfg(any(target_os = "linux", test))]
fn primary_selection_command_plan(session: LinuxSession) -> &'static [LinuxCommandSpec] {
    const WAYLAND_FIRST: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline"],
        },
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "primary"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--primary", "--output"],
        },
    ];
    const X11_FIRST: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "primary"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--primary", "--output"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline"],
        },
    ];
    const MIXED_DEFAULT: [LinuxCommandSpec; 4] = [
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline", "--type", "text"],
        },
        LinuxCommandSpec {
            program: "xclip",
            args: &["-o", "-selection", "primary"],
        },
        LinuxCommandSpec {
            program: "xsel",
            args: &["--primary", "--output"],
        },
        LinuxCommandSpec {
            program: "wl-paste",
            args: &["--primary", "--no-newline"],
        },
    ];

    if session.wayland && !session.x11 {
        &WAYLAND_FIRST
    } else if session.x11 && !session.wayland {
        &X11_FIRST
    } else {
        &MIXED_DEFAULT
    }
}

impl LinuxBackend for DefaultLinuxBackend {
    fn attempt_atspi(&self) -> PlatformAttemptResult {
        #[cfg(target_os = "linux")]
        {
            match read_atspi_text() {
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

impl Default for LinuxSelectionMonitor {
    fn default() -> Self {
        Self::new_with_options(LinuxSelectionMonitorOptions::default())
    }
}

impl Default for LinuxSelectionMonitorOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(120),
            backend: LinuxMonitorBackend::Polling,
            native_queue_capacity: 256,
            native_event_pump: None,
        }
    }
}

impl LinuxSelectionMonitor {
    pub fn new(poll_interval: Duration) -> Self {
        Self::new_with_options(LinuxSelectionMonitorOptions {
            poll_interval,
            backend: LinuxMonitorBackend::Polling,
            native_queue_capacity: 256,
            native_event_pump: None,
        })
    }

    pub fn new_with_options(options: LinuxSelectionMonitorOptions) -> Self {
        if matches!(options.backend, LinuxMonitorBackend::NativeEventPreferred) {
            install_default_linux_runtime_adapter_if_absent();
            ensure_linux_native_subscriber_hook_installed();
        }
        let native_observer_attached =
            matches!(options.backend, LinuxMonitorBackend::NativeEventPreferred)
                && LinuxObserverBridge::acquire();
        let native_event_pump = if native_observer_attached {
            options
                .native_event_pump
                .or(Some(linux_observer_drain_events_for_monitor))
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

    pub fn backend(&self) -> LinuxMonitorBackend {
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
        if matches!(self.backend, LinuxMonitorBackend::NativeEventPreferred) {
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
        #[cfg(target_os = "linux")]
        {
            let atspi = read_atspi_text().ok().flatten();
            if let Some(next) = atspi {
                let trimmed = next.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }

            let primary = read_primary_selection_text().ok().flatten();
            if let Some(next) = primary {
                let trimmed = next.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            None
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
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

impl MonitorPlatform for LinuxSelectionMonitor {
    fn next_selection_change(&self) -> Option<String> {
        self.next_selection_text()
    }
}

impl Drop for LinuxSelectionMonitor {
    fn drop(&mut self) {
        if self.native_observer_attached {
            let _ = LinuxObserverBridge::release();
        }
    }
}

#[cfg(target_os = "linux")]
fn read_clipboard_text() -> Result<Option<String>, String> {
    let session = detect_linux_session(
        env::var("WAYLAND_DISPLAY").ok().as_deref(),
        env::var("DISPLAY").ok().as_deref(),
    );
    try_linux_text_commands(clipboard_command_plan(session))
}

#[cfg(target_os = "linux")]
fn read_primary_selection_text() -> Result<Option<String>, String> {
    let session = detect_linux_session(
        env::var("WAYLAND_DISPLAY").ok().as_deref(),
        env::var("DISPLAY").ok().as_deref(),
    );
    try_linux_text_commands(primary_selection_command_plan(session))
}

#[cfg(target_os = "linux")]
fn try_linux_text_commands(commands: &[LinuxCommandSpec]) -> Result<Option<String>, String> {
    let mut errors = Vec::new();

    for command in commands {
        let output = match Command::new(command.program).args(command.args).output() {
            Ok(output) => output,
            Err(err) => {
                errors.push(format!("{}: {err}", command.program));
                continue;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            errors.push(format!("{}: {stderr}", command.program));
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
fn read_atspi_text() -> Result<Option<String>, String> {
    let script = r#"
import re
import subprocess
import sys

def call(cmd):
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError((proc.stderr or proc.stdout).strip())
    return proc.stdout.strip()

def parse_address(output):
    match = re.search(r"'([^']+)'", output)
    return match.group(1) if match else None

def parse_reference(output):
    match = re.search(r"\('([^']+)'\s*,\s*objectpath\s*'([^']+)'\)", output)
    if not match:
        match = re.search(r"\('([^']+)'\s*,\s*'([^']+)'\)", output)
    if not match:
        return None, None
    return match.group(1), match.group(2)

def parse_int(output):
    match = re.search(r"(-?\d+)", output)
    return int(match.group(1)) if match else None

def parse_text(output):
    match = re.search(r"\('((?:\\'|[^'])*)',\)", output)
    if not match:
        return None
    return match.group(1).replace("\\\\", "\\").replace("\\'", "'")

try:
    addr_out = call([
        "gdbus", "call",
        "--session",
        "--dest", "org.a11y.Bus",
        "--object-path", "/org/a11y/bus",
        "--method", "org.a11y.Bus.GetAddress",
    ])
    address = parse_address(addr_out)
    if not address:
        print("")
        sys.exit(0)

    active_out = call([
        "gdbus", "call",
        "--address", address,
        "--dest", "org.a11y.atspi.Registry",
        "--object-path", "/org/a11y/atspi/accessible/root",
        "--method", "org.a11y.atspi.Collection.GetActiveDescendant",
    ])
    bus, path = parse_reference(active_out)
    if not bus or not path or path == "/org/a11y/atspi/null":
        print("")
        sys.exit(0)

    nsel = 0
    try:
        nsel_out = call([
            "gdbus", "call",
            "--address", address,
            "--dest", bus,
            "--object-path", path,
            "--method", "org.a11y.atspi.Text.GetNSelections",
        ])
        nsel = parse_int(nsel_out) or 0
    except Exception:
        nsel = 0

    if nsel > 0:
        selection_out = call([
            "gdbus", "call",
            "--address", address,
            "--dest", bus,
            "--object-path", path,
            "--method", "org.a11y.atspi.Text.GetSelection",
            "0",
        ])
        bounds = re.findall(r"(-?\d+)", selection_out)
        if len(bounds) >= 2:
            start = int(bounds[0])
            end = int(bounds[1])
            if end > start:
                selected_out = call([
                    "gdbus", "call",
                    "--address", address,
                    "--dest", bus,
                    "--object-path", path,
                    "--method", "org.a11y.atspi.Text.GetText",
                    str(start),
                    str(end),
                ])
                selected_text = parse_text(selected_out)
                if selected_text and selected_text.strip():
                    print(selected_text)
                    sys.exit(0)

    try:
        all_text_out = call([
            "gdbus", "call",
            "--address", address,
            "--dest", bus,
            "--object-path", path,
            "--method", "org.a11y.atspi.Text.GetText",
            "0",
            "-1",
        ])
        all_text = parse_text(all_text_out)
        if all_text and all_text.strip():
            print(all_text)
            sys.exit(0)
    except Exception:
        pass

    try:
        name_out = call([
            "gdbus", "call",
            "--address", address,
            "--dest", bus,
            "--object-path", path,
            "--method", "org.freedesktop.DBus.Properties.Get",
            "org.a11y.atspi.Accessible",
            "Name",
        ])
        name = parse_text(name_out)
        if name and name.strip():
            print(name)
            sys.exit(0)
    except Exception:
        pass

    print("")
except Exception as err:
    sys.stderr.write(str(err))
    sys.exit(1)
"#;

    let output = Command::new("python3")
        .args(["-c", script])
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    Ok(normalize_linux_text_stdout(&stdout))
}

pub(crate) fn linux_default_runtime_event_source() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        return read_atspi_text().ok().flatten();
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(all(feature = "rich-content", target_os = "linux"))]
pub(crate) fn try_selected_rtf_by_atspi() -> Option<String> {
    let text = read_atspi_text().ok().flatten()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(plain_text_to_minimal_rtf(trimmed))
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
    use crate::linux_observer::linux_observer_test_lock;
    use crate::linux_subscriber::linux_native_subscriber_stats;
    use crate::LinuxObserverBridge;

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
    fn selection_monitor_default_poll_interval_is_stable() {
        let monitor = LinuxSelectionMonitor::default();
        assert_eq!(monitor.poll_interval, Duration::from_millis(120));
        assert_eq!(monitor.backend(), LinuxMonitorBackend::Polling);
    }

    #[test]
    fn selection_monitor_emits_only_new_values() {
        let monitor = LinuxSelectionMonitor::new(Duration::from_millis(10));
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
        let _guard = linux_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        fn pump() -> Vec<String> {
            vec![
                "  native a ".to_string(),
                "native a".to_string(),
                "native b".to_string(),
            ]
        }

        let monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
            poll_interval: Duration::from_millis(10),
            backend: LinuxMonitorBackend::NativeEventPreferred,
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
        let _guard = linux_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let _ = LinuxObserverBridge::stop();
        let _ = LinuxObserverBridge::start();
        assert!(LinuxObserverBridge::is_active());
        assert!(LinuxObserverBridge::push_event("bridge one"));
        assert!(LinuxObserverBridge::push_event("bridge two"));

        let monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
            poll_interval: Duration::from_millis(10),
            backend: LinuxMonitorBackend::NativeEventPreferred,
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
        assert!(LinuxObserverBridge::is_active());
        let _ = LinuxObserverBridge::stop();
    }

    #[test]
    fn selection_monitor_native_preferred_releases_bridge_on_drop() {
        let _guard = linux_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let _ = LinuxObserverBridge::stop();

        {
            let _monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
                poll_interval: Duration::from_millis(10),
                backend: LinuxMonitorBackend::NativeEventPreferred,
                native_queue_capacity: 8,
                native_event_pump: None,
            });
            assert!(LinuxObserverBridge::is_active());
        }

        assert!(!LinuxObserverBridge::is_active());
    }

    #[test]
    fn selection_monitor_native_preferred_transitions_subscriber_manager_lifecycle() {
        let _guard = linux_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let _ = LinuxObserverBridge::stop();
        let before = linux_native_subscriber_stats();

        {
            let _monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
                poll_interval: Duration::from_millis(10),
                backend: LinuxMonitorBackend::NativeEventPreferred,
                native_queue_capacity: 8,
                native_event_pump: None,
            });
            let during = linux_native_subscriber_stats();
            assert!(during.active);
            assert_eq!(during.starts, before.starts + 1);
        }

        let after = linux_native_subscriber_stats();
        assert!(after.stops > before.stops);
    }

    #[test]
    fn selection_monitor_native_preferred_applies_queue_capacity() {
        let _guard = linux_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
            poll_interval: Duration::from_millis(10),
            backend: LinuxMonitorBackend::NativeEventPreferred,
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

    #[test]
    fn detects_linux_session_flags_from_env_presence() {
        assert_eq!(
            detect_linux_session(Some("wayland-0"), None),
            LinuxSession {
                wayland: true,
                x11: false,
            }
        );
        assert_eq!(
            detect_linux_session(None, Some(":0")),
            LinuxSession {
                wayland: false,
                x11: true,
            }
        );
        assert_eq!(
            detect_linux_session(Some(""), Some("")),
            LinuxSession {
                wayland: false,
                x11: false,
            }
        );
    }

    #[test]
    fn clipboard_plan_prioritizes_wayland_only_session() {
        let plan = clipboard_command_plan(LinuxSession {
            wayland: true,
            x11: false,
        });
        assert_eq!(plan[0].program, "wl-paste");
        assert_eq!(plan[1].program, "wl-paste");
        assert_eq!(plan[0].args, &["--no-newline", "--type", "text"]);
    }

    #[test]
    fn clipboard_plan_prioritizes_x11_only_session() {
        let plan = clipboard_command_plan(LinuxSession {
            wayland: false,
            x11: true,
        });
        assert_eq!(plan[0].program, "xclip");
        assert_eq!(plan[1].program, "xsel");
        assert_eq!(plan[0].args, &["-o", "-selection", "clipboard"]);
    }

    #[test]
    fn primary_selection_plan_prioritizes_wayland_only_session() {
        let plan = primary_selection_command_plan(LinuxSession {
            wayland: true,
            x11: false,
        });
        assert_eq!(plan[0].program, "wl-paste");
        assert_eq!(plan[1].program, "wl-paste");
        assert_eq!(
            plan[0].args,
            &["--primary", "--no-newline", "--type", "text"]
        );
    }
}
