use super::*;
use crate::AxObserverBridge;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

fn native_pump_batches() -> &'static Mutex<VecDeque<Vec<String>>> {
    static BATCHES: OnceLock<Mutex<VecDeque<Vec<String>>>> = OnceLock::new();
    BATCHES.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn active_pid_batches() -> &'static Mutex<VecDeque<Option<u64>>> {
    static BATCHES: OnceLock<Mutex<VecDeque<Option<u64>>>> = OnceLock::new();
    BATCHES.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn monitor_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn reset_native_observer_state_for_tests() {
    force_native_observer_activation(Some(false));
    let _ = AxObserverBridge::stop();
    let _ = AxObserverBridge::drain_events(usize::MAX);
    if let Ok(mut pids) = active_pid_batches().lock() {
        pids.clear();
    }
}

fn push_native_pump_batch(batch: Vec<String>) {
    native_pump_batches().lock().unwrap().push_back(batch);
}

fn push_active_pid_batch(batch: Option<u64>) {
    active_pid_batches().lock().unwrap().push_back(batch);
}

fn test_native_event_pump() -> Vec<String> {
    native_pump_batches()
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_default()
}

fn test_active_pid_provider() -> Option<u64> {
    active_pid_batches().lock().unwrap().pop_front().flatten()
}

#[test]
fn bundle_root_uses_app_ancestor_when_present() {
    let path = PathBuf::from("/Applications/Test.app/Contents/MacOS/Test");
    let bundle = bundle_id_from_process_path(&path);
    assert_eq!(bundle, "/Applications/Test.app");
}

#[test]
fn bundle_root_falls_back_to_process_path() {
    let path = PathBuf::from("/usr/local/bin/code");
    let bundle = bundle_id_from_process_path(&path);
    assert_eq!(bundle, "/usr/local/bin/code");
}

#[test]
fn selection_monitor_default_poll_interval_is_stable() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    let monitor = MacOSSelectionMonitor::default();
    assert_eq!(monitor.poll_interval, Duration::from_millis(120));
    assert_eq!(monitor.backend(), MacOSMonitorBackend::Polling);
    assert!(!monitor.native_observer_active());
}

#[test]
fn selection_monitor_native_preferred_falls_back_to_polling_path() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
        poll_interval: Duration::from_millis(75),
        backend: MacOSMonitorBackend::NativeObserverPreferred,
        native_queue_capacity: 256,
        native_event_pump: None,
        active_pid_provider: None,
    });

    assert_eq!(monitor.poll_interval, Duration::from_millis(75));
    assert_eq!(
        monitor.backend(),
        MacOSMonitorBackend::NativeObserverPreferred
    );
    assert!(!monitor.native_observer_active());
}

#[test]
fn selection_monitor_native_queue_ignores_empty_events() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    let monitor = MacOSSelectionMonitor::default();

    assert!(!monitor.enqueue_native_selection_event(""));
    assert!(!monitor.enqueue_native_selection_event("   "));
}

#[test]
fn selection_monitor_native_queue_emits_in_order_and_dedups() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    let mut monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
        poll_interval: Duration::from_millis(75),
        backend: MacOSMonitorBackend::NativeObserverPreferred,
        native_queue_capacity: 256,
        native_event_pump: None,
        active_pid_provider: None,
    });
    monitor.native_observer_active = true;
    monitor.native_observer_attached = false;

    assert!(monitor.enqueue_native_selection_event("first"));
    assert!(!monitor.enqueue_native_selection_event("first"));
    assert!(monitor.enqueue_native_selection_event("second"));

    assert_eq!(monitor.next_selection_text(), Some("first".to_string()));
    assert_eq!(monitor.next_selection_text(), Some("second".to_string()));
    assert_eq!(monitor.next_selection_text(), None);
}

#[test]
fn selection_monitor_native_queue_applies_capacity_and_tracks_drops() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
        poll_interval: Duration::from_millis(50),
        backend: MacOSMonitorBackend::Polling,
        native_queue_capacity: 2,
        native_event_pump: None,
        active_pid_provider: None,
    });

    assert!(monitor.enqueue_native_selection_event("a"));
    assert!(monitor.enqueue_native_selection_event("b"));
    assert!(monitor.enqueue_native_selection_event("c"));

    assert_eq!(monitor.native_queue_depth(), 2);
    assert_eq!(monitor.native_events_dropped(), 1);
}

#[test]
fn selection_monitor_native_queue_batch_enqueue_counts_accepts() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
        poll_interval: Duration::from_millis(50),
        backend: MacOSMonitorBackend::Polling,
        native_queue_capacity: 4,
        native_event_pump: None,
        active_pid_provider: None,
    });

    let accepted =
        monitor.enqueue_native_selection_events(vec!["one", "one", " ", "two", "three"]);

    assert_eq!(accepted, 3);
    assert_eq!(monitor.native_queue_depth(), 3);
    assert_eq!(monitor.native_events_dropped(), 0);
}

#[test]
fn selection_monitor_native_observer_payload_uses_same_backpressure_path() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    let mut monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
        poll_interval: Duration::from_millis(50),
        backend: MacOSMonitorBackend::NativeObserverPreferred,
        native_queue_capacity: 2,
        native_event_pump: None,
        active_pid_provider: None,
    });
    monitor.native_observer_active = true;
    monitor.native_observer_attached = false;

    assert!(monitor.ingest_native_observer_payload(
        MacOSNativeEventSource::AXObserverSelectionChanged,
        "first"
    ));
    assert!(monitor.ingest_native_observer_payload(
        MacOSNativeEventSource::AXObserverSelectionChanged,
        "second"
    ));
    assert!(monitor.ingest_native_observer_payload(
        MacOSNativeEventSource::AXObserverSelectionChanged,
        "third"
    ));

    assert_eq!(monitor.native_events_dropped(), 1);
    assert_eq!(monitor.next_selection_text(), Some("second".to_string()));
    assert_eq!(monitor.next_selection_text(), Some("third".to_string()));
    assert_eq!(monitor.next_selection_text(), None);
}

#[test]
fn selection_monitor_native_event_pump_feeds_queue_with_backpressure_rules() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    push_native_pump_batch(vec!["a".into(), "a".into(), "b".into()]);
    push_native_pump_batch(vec!["c".into()]);

    let mut monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
        poll_interval: Duration::from_millis(50),
        backend: MacOSMonitorBackend::NativeObserverPreferred,
        native_queue_capacity: 3,
        native_event_pump: Some(test_native_event_pump),
        active_pid_provider: None,
    });
    monitor.native_observer_active = true;
    monitor.native_observer_attached = false;

    assert_eq!(monitor.next_selection_text(), Some("a".to_string()));
    assert_eq!(monitor.next_selection_text(), Some("b".to_string()));
    assert_eq!(monitor.next_selection_text(), Some("c".to_string()));
    assert_eq!(monitor.next_selection_text(), None);
    assert_eq!(monitor.native_events_dropped(), 0);
}

#[test]
fn selection_monitor_native_preferred_uses_ax_observer_bridge_pump_by_default() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    force_native_observer_activation(Some(true));
    let _ = AxObserverBridge::stop();
    let _ = AxObserverBridge::drain_events(usize::MAX);
    assert!(AxObserverBridge::start());

    let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
        poll_interval: Duration::from_millis(75),
        backend: MacOSMonitorBackend::NativeObserverPreferred,
        native_queue_capacity: 4,
        native_event_pump: None,
        active_pid_provider: None,
    });

    assert!(monitor.native_observer_active());
    assert!(AxObserverBridge::push_event("bridge-a"));
    assert!(AxObserverBridge::push_event("bridge-b"));
    assert_eq!(monitor.next_selection_text(), Some("bridge-a".to_string()));
    assert_eq!(monitor.next_selection_text(), Some("bridge-b".to_string()));
    assert_eq!(monitor.next_selection_text(), None);
}

#[test]
fn selection_monitor_drop_releases_ax_observer_bridge() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    force_native_observer_activation(Some(true));
    let _ = AxObserverBridge::stop();
    let _ = AxObserverBridge::drain_events(usize::MAX);

    {
        let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
            poll_interval: Duration::from_millis(75),
            backend: MacOSMonitorBackend::NativeObserverPreferred,
            native_queue_capacity: 4,
            native_event_pump: None,
            active_pid_provider: None,
        });
        assert!(monitor.native_observer_active());
        assert!(AxObserverBridge::is_active());
    }

    assert!(!AxObserverBridge::is_active());
}

#[test]
fn selection_monitor_native_runtime_rebuilds_on_focus_pid_change() {
    let _guard = monitor_test_lock()
        .lock()
        .expect("monitor test lock poisoned");
    reset_native_observer_state_for_tests();
    force_native_observer_activation(Some(true));
    push_active_pid_batch(Some(111));
    push_active_pid_batch(Some(222));
    push_active_pid_batch(Some(222));

    let monitor = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
        poll_interval: Duration::from_millis(75),
        backend: MacOSMonitorBackend::NativeObserverPreferred,
        native_queue_capacity: 4,
        native_event_pump: None,
        active_pid_provider: Some(test_active_pid_provider),
    });

    let stats = monitor.native_observer_stats();
    assert_eq!(stats.attach_attempts, 1);
    assert_eq!(stats.attach_successes, 0);
    assert_eq!(stats.attach_failures, 1);
    assert_eq!(stats.skipped_same_pid_retries, 0);

    let _ = monitor.poll_native_event_pump_once();
    let stats = monitor.native_observer_stats();
    assert_eq!(stats.attach_attempts, 2);
    assert_eq!(stats.attach_successes, 0);
    assert_eq!(stats.attach_failures, 2);
    assert_eq!(stats.skipped_same_pid_retries, 0);

    let _ = monitor.poll_native_event_pump_once();
    let stats = monitor.native_observer_stats();
    assert_eq!(stats.attach_attempts, 2);
    assert_eq!(stats.attach_successes, 0);
    assert_eq!(stats.attach_failures, 2);
    assert_eq!(stats.skipped_same_pid_retries, 1);
}
