use crate::observer_bridge::define_observer_bridge;

define_observer_bridge!(
    bridge = WindowsObserverBridge,
    lifecycle_hook_type = WindowsObserverLifecycleHook,
    test_lock_fn = windows_observer_test_lock,
);
