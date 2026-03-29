use crate::WindowsObserverBridge;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub type WindowsNativeRuntimeAdapter = fn(active: bool) -> bool;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowsNativeSubscriberStats {
    pub active: bool,
    pub starts: u64,
    pub stops: u64,
    pub adapter_attempts: u64,
    pub adapter_failures: u64,
}

struct WindowsNativeSubscriberManager {
    active: AtomicBool,
    starts: AtomicU64,
    stops: AtomicU64,
    adapter_attempts: AtomicU64,
    adapter_failures: AtomicU64,
}

impl WindowsNativeSubscriberManager {
    fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            starts: AtomicU64::new(0),
            stops: AtomicU64::new(0),
            adapter_attempts: AtomicU64::new(0),
            adapter_failures: AtomicU64::new(0),
        }
    }

    fn transition(&self, active: bool) {
        if active {
            if !self.active.swap(true, Ordering::SeqCst) {
                self.starts.fetch_add(1, Ordering::SeqCst);
                self.apply_runtime_adapter(true);
            }
        } else if self.active.swap(false, Ordering::SeqCst) {
            self.stops.fetch_add(1, Ordering::SeqCst);
            self.apply_runtime_adapter(false);
        }
    }

    fn stats(&self) -> WindowsNativeSubscriberStats {
        WindowsNativeSubscriberStats {
            active: self.active.load(Ordering::SeqCst),
            starts: self.starts.load(Ordering::SeqCst),
            stops: self.stops.load(Ordering::SeqCst),
            adapter_attempts: self.adapter_attempts.load(Ordering::SeqCst),
            adapter_failures: self.adapter_failures.load(Ordering::SeqCst),
        }
    }

    fn apply_runtime_adapter(&self, active: bool) {
        let Some(adapter) = runtime_adapter().lock().ok().and_then(|slot| *slot) else {
            return;
        };
        self.adapter_attempts.fetch_add(1, Ordering::SeqCst);
        if !adapter(active) {
            self.adapter_failures.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[cfg(test)]
    fn reset(&self) {
        self.active.store(false, Ordering::SeqCst);
        self.starts.store(0, Ordering::SeqCst);
        self.stops.store(0, Ordering::SeqCst);
        self.adapter_attempts.store(0, Ordering::SeqCst);
        self.adapter_failures.store(0, Ordering::SeqCst);
    }
}

fn manager() -> &'static WindowsNativeSubscriberManager {
    static MANAGER: OnceLock<WindowsNativeSubscriberManager> = OnceLock::new();
    MANAGER.get_or_init(WindowsNativeSubscriberManager::new)
}

fn runtime_adapter() -> &'static Mutex<Option<WindowsNativeRuntimeAdapter>> {
    static ADAPTER: OnceLock<Mutex<Option<WindowsNativeRuntimeAdapter>>> = OnceLock::new();
    ADAPTER.get_or_init(|| Mutex::new(None))
}

fn lifecycle_transition(active: bool) {
    manager().transition(active);
}

pub fn ensure_windows_native_subscriber_hook_installed() {
    if !WindowsObserverBridge::lifecycle_hook_registered() {
        WindowsObserverBridge::set_lifecycle_hook(Some(lifecycle_transition));
    }
}

pub fn windows_native_subscriber_stats() -> WindowsNativeSubscriberStats {
    manager().stats()
}

pub fn set_windows_native_runtime_adapter(adapter: Option<WindowsNativeRuntimeAdapter>) {
    if let Ok(mut slot) = runtime_adapter().lock() {
        *slot = adapter;
    }
}

pub fn windows_native_runtime_adapter_registered() -> bool {
    runtime_adapter()
        .lock()
        .map(|slot| slot.is_some())
        .unwrap_or(false)
}

#[cfg(test)]
fn reset_windows_native_subscriber_for_tests() {
    manager().reset();
    set_windows_native_runtime_adapter(None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn lifecycle_hook_drives_subscriber_stats() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        let _ = WindowsObserverBridge::stop();
        WindowsObserverBridge::set_lifecycle_hook(None);
        reset_windows_native_subscriber_for_tests();

        ensure_windows_native_subscriber_hook_installed();
        assert_eq!(windows_native_subscriber_stats().starts, 0);

        assert!(WindowsObserverBridge::start());
        let started = windows_native_subscriber_stats();
        assert!(started.active);
        assert_eq!(started.starts, 1);
        assert_eq!(started.stops, 0);

        assert!(WindowsObserverBridge::stop());
        let stopped = windows_native_subscriber_stats();
        assert!(!stopped.active);
        assert_eq!(stopped.starts, 1);
        assert_eq!(stopped.stops, 1);
    }

    #[test]
    fn runtime_adapter_failures_are_recorded() {
        fn adapter(_active: bool) -> bool {
            false
        }

        let _guard = test_lock().lock().expect("test lock poisoned");
        let _ = WindowsObserverBridge::stop();
        WindowsObserverBridge::set_lifecycle_hook(None);
        reset_windows_native_subscriber_for_tests();
        set_windows_native_runtime_adapter(Some(adapter));
        ensure_windows_native_subscriber_hook_installed();

        assert!(WindowsObserverBridge::start());
        assert!(WindowsObserverBridge::stop());

        let stats = windows_native_subscriber_stats();
        assert_eq!(stats.adapter_attempts, 2);
        assert_eq!(stats.adapter_failures, 2);
    }
}
