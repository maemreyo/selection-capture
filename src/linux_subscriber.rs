use crate::native_subscriber::define_native_subscriber_core;
use crate::LinuxObserverBridge;

define_native_subscriber_core!(
    observer_bridge = LinuxObserverBridge,
    runtime_adapter_type = LinuxNativeRuntimeAdapter,
    stats_type = LinuxNativeSubscriberStats,
    ensure_hook_fn = ensure_linux_native_subscriber_hook_installed,
    stats_fn = linux_native_subscriber_stats,
    set_adapter_fn = set_linux_native_runtime_adapter,
    adapter_registered_fn = linux_native_runtime_adapter_registered,
    reset_fn = reset_linux_native_subscriber_for_tests,
);

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
