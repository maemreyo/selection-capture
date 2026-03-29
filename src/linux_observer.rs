use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

const DEFAULT_OBSERVER_QUEUE_CAPACITY: usize = 1024;
const DEFAULT_DRAIN_BATCH_SIZE: usize = 64;

static OBSERVER_ACTIVE: AtomicBool = AtomicBool::new(false);
static OBSERVER_DROPPED_EVENTS: AtomicU64 = AtomicU64::new(0);
static OBSERVER_CLIENTS: AtomicU64 = AtomicU64::new(0);

pub type LinuxObserverLifecycleHook = fn(active: bool);

fn observer_queue() -> &'static Mutex<VecDeque<String>> {
    static QUEUE: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn lifecycle_hook() -> &'static Mutex<Option<LinuxObserverLifecycleHook>> {
    static HOOK: OnceLock<Mutex<Option<LinuxObserverLifecycleHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

pub struct LinuxObserverBridge;

impl LinuxObserverBridge {
    pub fn start() -> bool {
        let activated = !OBSERVER_ACTIVE.swap(true, Ordering::SeqCst);
        if activated {
            Self::invoke_lifecycle_hook(true);
        }
        activated
    }

    pub fn stop() -> bool {
        let was_active = OBSERVER_ACTIVE.swap(false, Ordering::SeqCst);
        OBSERVER_CLIENTS.store(0, Ordering::SeqCst);
        if let Ok(mut queue) = observer_queue().lock() {
            queue.clear();
        }
        if was_active {
            Self::invoke_lifecycle_hook(false);
        }
        was_active
    }

    pub fn acquire() -> bool {
        let previous = OBSERVER_CLIENTS.fetch_add(1, Ordering::SeqCst);
        if previous == 0 {
            let _ = Self::start();
        }
        Self::is_active()
    }

    pub fn release() -> bool {
        loop {
            let current = OBSERVER_CLIENTS.load(Ordering::SeqCst);
            if current == 0 {
                return Self::is_active();
            }
            if OBSERVER_CLIENTS
                .compare_exchange(current, current - 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                if current == 1 {
                    let _ = Self::stop();
                }
                return Self::is_active();
            }
        }
    }

    pub fn is_active() -> bool {
        OBSERVER_ACTIVE.load(Ordering::SeqCst)
    }

    pub fn push_event<T>(text: T) -> bool
    where
        T: Into<String>,
    {
        if !Self::is_active() {
            return false;
        }

        let text = text.into();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }

        let Ok(mut queue) = observer_queue().lock() else {
            return false;
        };

        if queue.back().map(|last| last == trimmed).unwrap_or(false) {
            return false;
        }

        if queue.len() >= DEFAULT_OBSERVER_QUEUE_CAPACITY {
            queue.pop_front();
            OBSERVER_DROPPED_EVENTS.fetch_add(1, Ordering::SeqCst);
        }

        queue.push_back(trimmed.to_string());
        true
    }

    pub fn drain_events(max_events: usize) -> Vec<String> {
        if max_events == 0 {
            return Vec::new();
        }

        let Ok(mut queue) = observer_queue().lock() else {
            return Vec::new();
        };

        let mut drained = Vec::new();
        while drained.len() < max_events {
            let Some(next) = queue.pop_front() else {
                break;
            };
            drained.push(next);
        }
        drained
    }

    pub fn dropped_events() -> u64 {
        OBSERVER_DROPPED_EVENTS.load(Ordering::SeqCst)
    }

    pub fn set_lifecycle_hook(hook: Option<LinuxObserverLifecycleHook>) {
        if let Ok(mut slot) = lifecycle_hook().lock() {
            *slot = hook;
        }
    }

    pub fn lifecycle_hook_registered() -> bool {
        lifecycle_hook()
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false)
    }

    fn invoke_lifecycle_hook(active: bool) {
        let callback = lifecycle_hook().lock().ok().and_then(|slot| *slot);
        if let Some(callback) = callback {
            callback(active);
        }
    }
}

pub fn drain_events_for_monitor() -> Vec<String> {
    LinuxObserverBridge::drain_events(DEFAULT_DRAIN_BATCH_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn reset_state() {
        OBSERVER_ACTIVE.store(false, Ordering::SeqCst);
        OBSERVER_DROPPED_EVENTS.store(0, Ordering::SeqCst);
        OBSERVER_CLIENTS.store(0, Ordering::SeqCst);
        if let Ok(mut queue) = observer_queue().lock() {
            queue.clear();
        }
        LinuxObserverBridge::set_lifecycle_hook(None);
    }

    #[test]
    fn observer_requires_active_state_to_accept_events() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        reset_state();
        assert!(!LinuxObserverBridge::push_event("hello"));
        assert!(LinuxObserverBridge::start());
        assert!(LinuxObserverBridge::push_event("hello"));
        assert_eq!(
            LinuxObserverBridge::drain_events(1),
            vec!["hello".to_string()]
        );
    }

    #[test]
    fn observer_dedups_tail_events() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        reset_state();
        assert!(LinuxObserverBridge::start());
        assert!(LinuxObserverBridge::push_event("a"));
        assert!(!LinuxObserverBridge::push_event("a"));
        assert!(LinuxObserverBridge::push_event("b"));
        assert_eq!(
            LinuxObserverBridge::drain_events(8),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn observer_stop_returns_previous_state() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        reset_state();
        assert!(!LinuxObserverBridge::stop());
        assert!(LinuxObserverBridge::start());
        assert!(LinuxObserverBridge::stop());
        assert!(!LinuxObserverBridge::is_active());
    }

    #[test]
    fn observer_acquire_release_tracks_lifecycle() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        reset_state();
        assert!(LinuxObserverBridge::acquire());
        assert!(LinuxObserverBridge::is_active());
        assert!(LinuxObserverBridge::acquire());
        assert!(LinuxObserverBridge::release());
        assert!(LinuxObserverBridge::is_active());
        assert!(!LinuxObserverBridge::release());
        assert!(!LinuxObserverBridge::is_active());
    }

    #[test]
    fn observer_stop_clears_queued_events() {
        let _guard = test_lock().lock().expect("test lock poisoned");
        reset_state();
        assert!(LinuxObserverBridge::start());
        assert!(LinuxObserverBridge::push_event("queued"));
        assert!(LinuxObserverBridge::stop());
        assert!(LinuxObserverBridge::drain_events(8).is_empty());
    }

    #[test]
    fn observer_invokes_lifecycle_hook_on_start_and_stop() {
        static STARTED: AtomicUsize = AtomicUsize::new(0);
        static STOPPED: AtomicUsize = AtomicUsize::new(0);

        fn hook(active: bool) {
            if active {
                STARTED.fetch_add(1, AtomicOrdering::SeqCst);
            } else {
                STOPPED.fetch_add(1, AtomicOrdering::SeqCst);
            }
        }

        let _guard = test_lock().lock().expect("test lock poisoned");
        reset_state();
        STARTED.store(0, AtomicOrdering::SeqCst);
        STOPPED.store(0, AtomicOrdering::SeqCst);
        LinuxObserverBridge::set_lifecycle_hook(Some(hook));

        let _ = LinuxObserverBridge::start();
        assert!(LinuxObserverBridge::is_active());
        let _ = LinuxObserverBridge::stop();
        assert!(!LinuxObserverBridge::is_active());

        assert_eq!(STARTED.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(STOPPED.load(AtomicOrdering::SeqCst), 1);
    }
}
