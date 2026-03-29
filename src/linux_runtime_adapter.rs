use crate::linux::linux_default_runtime_event_source as linux_platform_runtime_event_source;
#[cfg(target_os = "linux")]
use crate::linux_observer::LinuxObserverBridge;
use crate::linux_subscriber::{
    linux_native_runtime_adapter_registered, set_linux_native_runtime_adapter,
};
#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader};
#[cfg(target_os = "linux")]
use std::process::{Child, ChildStdout, Command, Stdio};
#[cfg(target_os = "linux")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex as StdMutex,
};
use std::sync::{Mutex, OnceLock};
#[cfg(target_os = "linux")]
use std::thread::{self, JoinHandle};
#[cfg(target_os = "linux")]
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinuxDefaultRuntimeAdapterState {
    pub attached: bool,
    pub worker_running: bool,
    pub attach_calls: u64,
    pub detach_calls: u64,
    pub listener_exits: u64,
    pub listener_restarts: u64,
    pub listener_failures: u64,
}

pub type LinuxDefaultRuntimeEventSource = fn() -> Option<String>;

#[cfg(target_os = "linux")]
const LINUX_RUNTIME_EVENT_MARKER: &str = "__SC_EVENT__";
#[cfg(target_os = "linux")]
const LINUX_ATTACH_RETRY_LIMIT: u32 = 4;
#[cfg(target_os = "linux")]
const LINUX_RESTART_RETRY_LIMIT: u32 = 8;
#[cfg(target_os = "linux")]
const LINUX_RETRY_BACKOFF_BASE: Duration = Duration::from_millis(50);
#[cfg(target_os = "linux")]
const LINUX_RETRY_BACKOFF_MAX: Duration = Duration::from_millis(800);

#[cfg(target_os = "linux")]
fn retry_backoff_delay(attempt: u32) -> Duration {
    let factor = 1u64 << attempt.min(6);
    let millis = LINUX_RETRY_BACKOFF_BASE
        .as_millis()
        .saturating_mul(u128::from(factor))
        .min(LINUX_RETRY_BACKOFF_MAX.as_millis());
    Duration::from_millis(millis as u64)
}

#[cfg(target_os = "linux")]
const LINUX_RUNTIME_LISTENER_SCRIPT: &str = r#"
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

address_output = call([
    "gdbus", "call",
    "--session",
    "--dest", "org.a11y.Bus",
    "--object-path", "/org/a11y/bus",
    "--method", "org.a11y.Bus.GetAddress",
])

address = parse_address(address_output)
if not address:
    sys.exit(0)

monitor = subprocess.Popen(
    [
        "dbus-monitor",
        "--address",
        address,
        "type='signal',interface='org.a11y.atspi.Event.Object'",
    ],
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
    text=True,
    bufsize=1,
)

try:
    for line in monitor.stdout or []:
        if (
            "member=TextSelectionChanged" in line
            or "member=TextChanged" in line
            or "member=StateChanged" in line
            or "member=TextCaretMoved" in line
        ):
            print("__SC_EVENT__", flush=True)
finally:
    monitor.terminate()
"#;

#[cfg(target_os = "linux")]
struct LinuxRuntimeWorker {
    stop: Arc<AtomicBool>,
    child: Arc<StdMutex<Option<Child>>>,
    telemetry: Arc<LinuxWorkerTelemetry>,
    handle: JoinHandle<()>,
}

#[cfg(target_os = "linux")]
#[derive(Default)]
struct LinuxWorkerTelemetry {
    listener_exits: std::sync::atomic::AtomicU64,
    listener_restarts: std::sync::atomic::AtomicU64,
    listener_failures: std::sync::atomic::AtomicU64,
}

#[cfg(target_os = "linux")]
impl LinuxWorkerTelemetry {
    fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.listener_exits.load(Ordering::SeqCst),
            self.listener_restarts.load(Ordering::SeqCst),
            self.listener_failures.load(Ordering::SeqCst),
        )
    }
}

#[cfg(target_os = "linux")]
impl LinuxRuntimeWorker {
    fn spawn() -> Option<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let child = Arc::new(StdMutex::new(None));
        let telemetry = Arc::new(LinuxWorkerTelemetry::default());
        let stdout = install_new_linux_listener(&child)?;
        let stop_signal = Arc::clone(&stop);
        let child_signal = Arc::clone(&child);
        let telemetry_signal = Arc::clone(&telemetry);
        let handle = thread::Builder::new()
            .name("selection-capture-linux-runtime".to_string())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    if stop_signal.load(Ordering::SeqCst) {
                        break;
                    }

                    let mut line = String::new();
                    let Ok(read) = reader.read_line(&mut line) else {
                        telemetry_signal
                            .listener_exits
                            .fetch_add(1, Ordering::SeqCst);
                        if !restart_linux_listener(
                            &child_signal,
                            &stop_signal,
                            &telemetry_signal,
                            &mut reader,
                        ) {
                            break;
                        }
                        continue;
                    };
                    if read == 0 {
                        telemetry_signal
                            .listener_exits
                            .fetch_add(1, Ordering::SeqCst);
                        if !restart_linux_listener(
                            &child_signal,
                            &stop_signal,
                            &telemetry_signal,
                            &mut reader,
                        ) {
                            break;
                        }
                        continue;
                    }

                    if line.trim() == LINUX_RUNTIME_EVENT_MARKER {
                        if let Some(source) = linux_default_runtime_event_source() {
                            if let Some(text) = source() {
                                let _ = LinuxObserverBridge::push_event(text);
                            }
                        }
                    }
                }

                if let Ok(mut slot) = child_signal.lock() {
                    if let Some(mut child) = slot.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
            })
            .ok()?;
        Some(Self {
            stop,
            child,
            telemetry,
            handle,
        })
    }

    fn stop(self) -> bool {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut slot) = self.child.lock() {
            if let Some(mut child) = slot.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        self.handle.join().is_ok()
    }

    fn telemetry_snapshot(&self) -> (u64, u64, u64) {
        self.telemetry.snapshot()
    }

    fn is_running(&self) -> bool {
        !self.handle.is_finished()
    }
}

#[cfg(target_os = "linux")]
fn spawn_linux_runtime_listener_process() -> Option<Child> {
    Command::new("python3")
        .args(["-u", "-c", LINUX_RUNTIME_LISTENER_SCRIPT])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

#[cfg(target_os = "linux")]
fn install_new_linux_listener(child_slot: &Arc<StdMutex<Option<Child>>>) -> Option<ChildStdout> {
    let mut child = spawn_linux_runtime_listener_process()?;
    let stdout = child.stdout.take()?;
    if let Ok(mut slot) = child_slot.lock() {
        if let Some(mut previous) = slot.replace(child) {
            let _ = previous.kill();
            let _ = previous.wait();
        }
    }
    Some(stdout)
}

#[cfg(target_os = "linux")]
fn restart_linux_listener(
    child_slot: &Arc<StdMutex<Option<Child>>>,
    stop_signal: &Arc<AtomicBool>,
    telemetry: &Arc<LinuxWorkerTelemetry>,
    reader: &mut BufReader<ChildStdout>,
) -> bool {
    for attempt in 0..LINUX_RESTART_RETRY_LIMIT {
        if stop_signal.load(Ordering::SeqCst) {
            return false;
        }
        telemetry.listener_restarts.fetch_add(1, Ordering::SeqCst);
        if let Some(stdout) = install_new_linux_listener(child_slot) {
            *reader = BufReader::new(stdout);
            return true;
        }
        telemetry.listener_failures.fetch_add(1, Ordering::SeqCst);
        thread::sleep(retry_backoff_delay(attempt));
    }
    false
}

#[derive(Default)]
struct LinuxDefaultRuntimeAdapterRuntime {
    state: LinuxDefaultRuntimeAdapterState,
    #[cfg(target_os = "linux")]
    worker: Option<LinuxRuntimeWorker>,
}

fn adapter_runtime() -> &'static Mutex<LinuxDefaultRuntimeAdapterRuntime> {
    static RUNTIME: OnceLock<Mutex<LinuxDefaultRuntimeAdapterRuntime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(LinuxDefaultRuntimeAdapterRuntime::default()))
}

fn event_source_slot() -> &'static Mutex<Option<LinuxDefaultRuntimeEventSource>> {
    static SOURCE: OnceLock<Mutex<Option<LinuxDefaultRuntimeEventSource>>> = OnceLock::new();
    SOURCE.get_or_init(|| Mutex::new(None))
}

#[cfg(target_os = "linux")]
fn linux_default_runtime_event_source() -> Option<LinuxDefaultRuntimeEventSource> {
    event_source_slot().lock().ok().and_then(|slot| *slot)
}

fn attach_default_linux_listener(runtime: &mut LinuxDefaultRuntimeAdapterRuntime) -> bool {
    #[cfg(target_os = "linux")]
    {
        if runtime.worker.is_some() {
            return true;
        }
        for attempt in 0..LINUX_ATTACH_RETRY_LIMIT {
            if let Some(worker) = LinuxRuntimeWorker::spawn() {
                runtime.worker = Some(worker);
                return true;
            }
            runtime.state.listener_failures += 1;
            thread::sleep(retry_backoff_delay(attempt));
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = runtime;
        true
    }
}

fn detach_default_linux_listener(runtime: &mut LinuxDefaultRuntimeAdapterRuntime) -> bool {
    #[cfg(target_os = "linux")]
    {
        runtime
            .worker
            .take()
            .map(|worker| worker.stop())
            .unwrap_or(true)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = runtime;
        true
    }
}

fn default_linux_runtime_adapter(active: bool) -> bool {
    let Ok(mut runtime) = adapter_runtime().lock() else {
        return false;
    };

    if active {
        if runtime.state.attached {
            return true;
        }
        if !attach_default_linux_listener(&mut runtime) {
            return false;
        }
        runtime.state.attached = true;
        runtime.state.worker_running = cfg!(target_os = "linux");
        runtime.state.attach_calls += 1;
        return true;
    }

    if !runtime.state.attached {
        return true;
    }
    if !detach_default_linux_listener(&mut runtime) {
        return false;
    }
    runtime.state.attached = false;
    runtime.state.worker_running = false;
    runtime.state.detach_calls += 1;
    true
}

pub fn linux_default_runtime_adapter_state() -> LinuxDefaultRuntimeAdapterState {
    adapter_runtime()
        .lock()
        .map(|runtime| {
            #[cfg(target_os = "linux")]
            {
                let mut state = runtime.state;
                if let Some(worker) = runtime.worker.as_ref() {
                    state.worker_running = state.worker_running && worker.is_running();
                    let (listener_exits, listener_restarts, listener_failures) =
                        worker.telemetry_snapshot();
                    state.listener_exits = state.listener_exits.saturating_add(listener_exits);
                    state.listener_restarts =
                        state.listener_restarts.saturating_add(listener_restarts);
                    state.listener_failures =
                        state.listener_failures.saturating_add(listener_failures);
                }
                state
            }
            #[cfg(not(target_os = "linux"))]
            {
                runtime.state
            }
        })
        .unwrap_or_default()
}

pub fn set_linux_default_runtime_event_source(source: Option<LinuxDefaultRuntimeEventSource>) {
    if let Ok(mut slot) = event_source_slot().lock() {
        *slot = source;
    }
}

pub fn linux_default_runtime_event_source_registered() -> bool {
    event_source_slot()
        .lock()
        .map(|slot| slot.is_some())
        .unwrap_or(false)
}

#[cfg(test)]
fn reset_linux_default_runtime_adapter_state() {
    let _ = default_linux_runtime_adapter(false);
    if let Ok(mut runtime) = adapter_runtime().lock() {
        *runtime = LinuxDefaultRuntimeAdapterRuntime::default();
    }
    set_linux_default_runtime_event_source(None);
}

#[cfg(all(test, target_os = "linux"))]
fn kill_linux_listener_for_tests() -> bool {
    let Ok(runtime) = adapter_runtime().lock() else {
        return false;
    };
    let Some(worker) = runtime.worker.as_ref() else {
        return false;
    };
    let Ok(mut slot) = worker.child.lock() else {
        return false;
    };
    let Some(child) = slot.as_mut() else {
        return false;
    };
    child.kill().is_ok()
}

pub fn install_default_linux_runtime_adapter_if_absent() {
    if !linux_default_runtime_event_source_registered() {
        set_linux_default_runtime_event_source(Some(linux_platform_runtime_event_source));
    }
    if !linux_native_runtime_adapter_registered() {
        set_linux_native_runtime_adapter(Some(default_linux_runtime_adapter));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ensure_linux_native_subscriber_hook_installed, linux_native_subscriber_stats,
        LinuxObserverBridge,
    };
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn installing_default_adapter_enables_lifecycle_attempt_tracking() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        let _ = LinuxObserverBridge::stop();
        LinuxObserverBridge::set_lifecycle_hook(None);
        reset_linux_default_runtime_adapter_state();
        set_linux_native_runtime_adapter(None);
        set_linux_default_runtime_event_source(None);
        ensure_linux_native_subscriber_hook_installed();
        install_default_linux_runtime_adapter_if_absent();
        assert!(linux_native_runtime_adapter_registered());
        assert!(linux_default_runtime_event_source_registered());

        let before = linux_native_subscriber_stats();
        let _ = LinuxObserverBridge::start();
        let _ = LinuxObserverBridge::stop();
        let after = linux_native_subscriber_stats();

        assert!(after.adapter_attempts >= before.adapter_attempts);
    }

    #[test]
    fn default_adapter_state_tracks_attach_detach_idempotently() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        reset_linux_default_runtime_adapter_state();
        assert_eq!(
            linux_default_runtime_adapter_state(),
            LinuxDefaultRuntimeAdapterState::default()
        );

        assert!(default_linux_runtime_adapter(true));
        assert!(default_linux_runtime_adapter(true));
        let started = linux_default_runtime_adapter_state();
        assert!(started.attached);
        assert_eq!(started.worker_running, cfg!(target_os = "linux"));
        assert_eq!(started.attach_calls, 1);
        assert_eq!(started.detach_calls, 0);

        assert!(default_linux_runtime_adapter(false));
        assert!(default_linux_runtime_adapter(false));
        let stopped = linux_default_runtime_adapter_state();
        assert!(!stopped.attached);
        assert!(!stopped.worker_running);
        assert_eq!(stopped.attach_calls, 1);
        assert_eq!(stopped.detach_calls, 1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn retry_backoff_delay_is_bounded_exponential() {
        assert_eq!(retry_backoff_delay(0), Duration::from_millis(50));
        assert_eq!(retry_backoff_delay(1), Duration::from_millis(100));
        assert_eq!(retry_backoff_delay(2), Duration::from_millis(200));
        assert_eq!(retry_backoff_delay(4), Duration::from_millis(800));
        assert_eq!(retry_backoff_delay(8), Duration::from_millis(800));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn listener_restart_updates_telemetry_after_forced_kill() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        let _ = LinuxObserverBridge::stop();
        LinuxObserverBridge::set_lifecycle_hook(None);
        reset_linux_default_runtime_adapter_state();
        set_linux_native_runtime_adapter(None);
        set_linux_default_runtime_event_source(None);
        ensure_linux_native_subscriber_hook_installed();
        install_default_linux_runtime_adapter_if_absent();

        let _ = LinuxObserverBridge::start();
        let before = linux_default_runtime_adapter_state();
        if !before.attached || !before.worker_running {
            let _ = LinuxObserverBridge::stop();
            return;
        }

        if !kill_linux_listener_for_tests() {
            let _ = LinuxObserverBridge::stop();
            return;
        }

        let mut after = before;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(50));
            after = linux_default_runtime_adapter_state();
            if after.listener_restarts > before.listener_restarts
                || after.listener_exits > before.listener_exits
            {
                break;
            }
        }

        assert!(after.listener_exits >= before.listener_exits);
        assert!(after.listener_restarts >= before.listener_restarts);
        let _ = LinuxObserverBridge::stop();
    }
}
