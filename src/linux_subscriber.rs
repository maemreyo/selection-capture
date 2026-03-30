use crate::LinuxObserverBridge;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub type LinuxNativeRuntimeAdapter = fn(active: bool) -> bool;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinuxNativeSubscriberStats {
    pub active: bool,
    pub starts: u64,
    pub stops: u64,
    pub adapter_attempts: u64,
    pub adapter_failures: u64,
}

struct LinuxNativeSubscriberManager {
    active: AtomicBool,
    starts: AtomicU64,
    stops: AtomicU64,
    adapter_attempts: AtomicU64,
    adapter_failures: AtomicU64,
}

impl LinuxNativeSubscriberManager {
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

    fn stats(&self) -> LinuxNativeSubscriberStats {
        LinuxNativeSubscriberStats {
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

fn manager() -> &'static LinuxNativeSubscriberManager {
    static MANAGER: OnceLock<LinuxNativeSubscriberManager> = OnceLock::new();
    MANAGER.get_or_init(LinuxNativeSubscriberManager::new)
}

fn runtime_adapter() -> &'static Mutex<Option<LinuxNativeRuntimeAdapter>> {
    static ADAPTER: OnceLock<Mutex<Option<LinuxNativeRuntimeAdapter>>> = OnceLock::new();
    ADAPTER.get_or_init(|| Mutex::new(None))
}

fn lifecycle_transition(active: bool) {
    manager().transition(active);
}

pub fn ensure_linux_native_subscriber_hook_installed() {
    if !LinuxObserverBridge::lifecycle_hook_registered() {
        LinuxObserverBridge::set_lifecycle_hook(Some(lifecycle_transition));
    }
}

pub fn linux_native_subscriber_stats() -> LinuxNativeSubscriberStats {
    manager().stats()
}

pub fn set_linux_native_runtime_adapter(adapter: Option<LinuxNativeRuntimeAdapter>) {
    if let Ok(mut slot) = runtime_adapter().lock() {
        *slot = adapter;
    }
}

pub fn linux_native_runtime_adapter_registered() -> bool {
    runtime_adapter()
        .lock()
        .map(|slot| slot.is_some())
        .unwrap_or(false)
}

#[cfg(test)]
fn reset_linux_native_subscriber_for_tests() {
    manager().reset();
    set_linux_native_runtime_adapter(None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux_observer::linux_observer_test_lock;

    #[test]
    fn lifecycle_hook_drives_subscriber_stats() {
        let _guard = linux_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let _ = LinuxObserverBridge::stop();
        LinuxObserverBridge::set_lifecycle_hook(None);
        reset_linux_native_subscriber_for_tests();

        ensure_linux_native_subscriber_hook_installed();
        let before = linux_native_subscriber_stats();

        let _ = LinuxObserverBridge::start();
        let started = linux_native_subscriber_stats();
        assert!(started.starts >= before.starts);
        assert_eq!(started.stops, before.stops);
        assert!(started.active || LinuxObserverBridge::is_active());

        let _ = LinuxObserverBridge::stop();
        let stopped = linux_native_subscriber_stats();
        assert!(stopped.starts >= started.starts);
        assert!(stopped.stops >= started.stops);
        assert!(!stopped.active || LinuxObserverBridge::is_active());
    }

    #[test]
    fn runtime_adapter_failures_are_recorded() {
        fn adapter(_active: bool) -> bool {
            false
        }

        let _guard = linux_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
        let _ = LinuxObserverBridge::stop();
        LinuxObserverBridge::set_lifecycle_hook(None);
        reset_linux_native_subscriber_for_tests();
        set_linux_native_runtime_adapter(Some(adapter));
        ensure_linux_native_subscriber_hook_installed();

        let _ = LinuxObserverBridge::start();
        let _ = LinuxObserverBridge::stop();

        let stats = linux_native_subscriber_stats();
        assert!(stats.adapter_attempts >= 2);
        assert!(stats.adapter_failures >= 2);
    }
}
