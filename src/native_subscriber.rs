/// Generates the production code for a native-event subscriber manager.
///
/// Each invocation creates independent static state in the calling module.
/// Test modules are NOT included here because Windows and Linux have slightly
/// different assertion semantics — each module provides its own `#[cfg(test)] mod tests`.
///
/// # Parameters
/// - `observer_bridge` — the observer bridge type whose lifecycle hook this subscribes to
/// - `runtime_adapter_type` — `fn(bool) -> bool` type alias name
/// - `stats_type` — public stats struct name
/// - `ensure_hook_fn` — `pub fn` that installs the lifecycle hook if absent
/// - `stats_fn` — `pub fn` that returns the current stats snapshot
/// - `set_adapter_fn` — `pub fn` that sets the runtime adapter
/// - `adapter_registered_fn` — `pub fn` that checks whether an adapter is installed
/// - `reset_fn` — `#[cfg(test)] fn` that resets all state for test isolation
#[cfg(any(feature = "windows-beta", feature = "linux-alpha"))]
macro_rules! define_native_subscriber_core {
    (
        observer_bridge = $ObserverBridge:ident,
        runtime_adapter_type = $AdapterType:ident,
        stats_type = $StatsType:ident,
        ensure_hook_fn = $ensure_hook_fn:ident,
        stats_fn = $stats_fn:ident,
        set_adapter_fn = $set_adapter_fn:ident,
        adapter_registered_fn = $adapter_registered_fn:ident,
        reset_fn = $reset_fn:ident $(,)?
    ) => {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        use std::sync::{Mutex, OnceLock};

        pub type $AdapterType = fn(active: bool) -> bool;

        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $StatsType {
            pub active: bool,
            pub starts: u64,
            pub stops: u64,
            pub adapter_attempts: u64,
            pub adapter_failures: u64,
        }

        struct NativeSubscriberManager {
            active: AtomicBool,
            starts: AtomicU64,
            stops: AtomicU64,
            adapter_attempts: AtomicU64,
            adapter_failures: AtomicU64,
        }

        impl NativeSubscriberManager {
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

            fn stats(&self) -> $StatsType {
                $StatsType {
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

        fn manager() -> &'static NativeSubscriberManager {
            static MANAGER: OnceLock<NativeSubscriberManager> = OnceLock::new();
            MANAGER.get_or_init(NativeSubscriberManager::new)
        }

        fn runtime_adapter() -> &'static Mutex<Option<$AdapterType>> {
            static ADAPTER: OnceLock<Mutex<Option<$AdapterType>>> = OnceLock::new();
            ADAPTER.get_or_init(|| Mutex::new(None))
        }

        fn lifecycle_transition(active: bool) {
            manager().transition(active);
        }

        pub fn $ensure_hook_fn() {
            if !$ObserverBridge::lifecycle_hook_registered() {
                $ObserverBridge::set_lifecycle_hook(Some(lifecycle_transition));
            }
        }

        pub fn $stats_fn() -> $StatsType {
            manager().stats()
        }

        pub fn $set_adapter_fn(adapter: Option<$AdapterType>) {
            if let Ok(mut slot) = runtime_adapter().lock() {
                *slot = adapter;
            }
        }

        pub fn $adapter_registered_fn() -> bool {
            runtime_adapter()
                .lock()
                .map(|slot| slot.is_some())
                .unwrap_or(false)
        }

        #[cfg(test)]
        fn $reset_fn() {
            manager().reset();
            $set_adapter_fn(None);
        }
    };
}

#[cfg(any(feature = "windows-beta", feature = "linux-alpha"))]
pub(crate) use define_native_subscriber_core;
