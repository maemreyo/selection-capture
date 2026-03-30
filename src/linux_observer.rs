use crate::observer_bridge::define_observer_bridge;

define_observer_bridge!(
    bridge = LinuxObserverBridge,
    lifecycle_hook_type = LinuxObserverLifecycleHook,
    test_lock_fn = linux_observer_test_lock,
);
