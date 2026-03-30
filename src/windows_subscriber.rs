use crate::native_subscriber::define_native_subscriber_core;
use crate::WindowsObserverBridge;

define_native_subscriber_core!(
    observer_bridge = WindowsObserverBridge,
    runtime_adapter_type = WindowsNativeRuntimeAdapter,
    stats_type = WindowsNativeSubscriberStats,
    ensure_hook_fn = ensure_windows_native_subscriber_hook_installed,
    stats_fn = windows_native_subscriber_stats,
    set_adapter_fn = set_windows_native_runtime_adapter,
    adapter_registered_fn = windows_native_runtime_adapter_registered,
    reset_fn = reset_windows_native_subscriber_for_tests,
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_observer::windows_observer_test_lock;

    #[test]
    fn lifecycle_hook_drives_subscriber_stats() {
        let _guard = windows_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
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

        let _guard = windows_observer_test_lock()
            .lock()
            .expect("test lock poisoned");
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
