use super::*;
use crate::linux_observer::linux_observer_test_lock;
use crate::linux_subscriber::linux_native_subscriber_stats;
use crate::LinuxObserverBridge;

#[derive(Debug)]
struct StubBackend {
    atspi: PlatformAttemptResult,
    x11_selection: PlatformAttemptResult,
    clipboard: PlatformAttemptResult,
}

impl LinuxBackend for StubBackend {
    fn attempt_atspi(&self) -> PlatformAttemptResult {
        self.atspi.clone()
    }

    fn attempt_x11_selection(&self) -> PlatformAttemptResult {
        self.x11_selection.clone()
    }

    fn attempt_clipboard(&self) -> PlatformAttemptResult {
        self.clipboard.clone()
    }
}

#[test]
fn constructor_builds_stub_platform() {
    let platform = LinuxPlatform::new();
    let _ = platform;
}

#[test]
fn selection_monitor_default_poll_interval_is_stable() {
    let monitor = LinuxSelectionMonitor::default();
    assert_eq!(monitor.poll_interval, Duration::from_millis(120));
    assert_eq!(monitor.backend(), LinuxMonitorBackend::Polling);
}

#[test]
fn selection_monitor_emits_only_new_values() {
    let monitor = LinuxSelectionMonitor::new(Duration::from_millis(10));
    assert_eq!(
        monitor.emit_if_new("first".to_string()),
        Some("first".to_string())
    );
    assert_eq!(monitor.emit_if_new("first".to_string()), None);
    assert_eq!(
        monitor.emit_if_new("second".to_string()),
        Some("second".to_string())
    );
}

#[test]
fn selection_monitor_native_preferred_uses_event_pump_when_available() {
    let _guard = linux_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    fn pump() -> Vec<String> {
        vec![
            "  native a ".to_string(),
            "native a".to_string(),
            "native b".to_string(),
        ]
    }

    let monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
        poll_interval: Duration::from_millis(10),
        backend: LinuxMonitorBackend::NativeEventPreferred,
        native_queue_capacity: 8,
        native_event_pump: Some(pump),
    });

    assert_eq!(
        monitor.next_selection_change(),
        Some("native a".to_string())
    );
    assert_eq!(
        monitor.next_selection_change(),
        Some("native b".to_string())
    );
}

#[test]
fn selection_monitor_native_preferred_uses_bridge_drain_by_default() {
    let _guard = linux_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    let _ = LinuxObserverBridge::stop();
    let _ = LinuxObserverBridge::start();
    assert!(LinuxObserverBridge::is_active());
    assert!(LinuxObserverBridge::push_event("bridge one"));
    assert!(LinuxObserverBridge::push_event("bridge two"));

    let monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
        poll_interval: Duration::from_millis(10),
        backend: LinuxMonitorBackend::NativeEventPreferred,
        native_queue_capacity: 8,
        native_event_pump: None,
    });

    assert_eq!(
        monitor.next_selection_change(),
        Some("bridge one".to_string())
    );
    assert_eq!(
        monitor.next_selection_change(),
        Some("bridge two".to_string())
    );
    assert!(LinuxObserverBridge::is_active());
    let _ = LinuxObserverBridge::stop();
}

#[test]
fn selection_monitor_native_preferred_releases_bridge_on_drop() {
    let _guard = linux_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    let _ = LinuxObserverBridge::stop();

    {
        let _monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
            poll_interval: Duration::from_millis(10),
            backend: LinuxMonitorBackend::NativeEventPreferred,
            native_queue_capacity: 8,
            native_event_pump: None,
        });
        assert!(LinuxObserverBridge::is_active());
    }

    assert!(!LinuxObserverBridge::is_active());
}

#[test]
fn selection_monitor_native_preferred_transitions_subscriber_manager_lifecycle() {
    let _guard = linux_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    let _ = LinuxObserverBridge::stop();
    let before = linux_native_subscriber_stats();

    {
        let _monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
            poll_interval: Duration::from_millis(10),
            backend: LinuxMonitorBackend::NativeEventPreferred,
            native_queue_capacity: 8,
            native_event_pump: None,
        });
        let during = linux_native_subscriber_stats();
        assert!(during.active);
        assert_eq!(during.starts, before.starts + 1);
    }

    let after = linux_native_subscriber_stats();
    assert!(after.stops > before.stops);
}

#[test]
fn selection_monitor_native_preferred_applies_queue_capacity() {
    let _guard = linux_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    let monitor = LinuxSelectionMonitor::new_with_options(LinuxSelectionMonitorOptions {
        poll_interval: Duration::from_millis(10),
        backend: LinuxMonitorBackend::NativeEventPreferred,
        native_queue_capacity: 2,
        native_event_pump: None,
    });
    let accepted = monitor.enqueue_native_selection_events(vec![
        "first".to_string(),
        "second".to_string(),
        "third".to_string(),
    ]);
    assert_eq!(accepted, 3);
    assert_eq!(monitor.native_queue_depth(), 2);
    assert_eq!(monitor.native_events_dropped(), 1);
    assert_eq!(monitor.next_selection_change(), Some("second".to_string()));
    assert_eq!(monitor.next_selection_change(), Some("third".to_string()));
}

#[test]
fn active_app_probe_does_not_panic() {
    let platform = LinuxPlatform::new();
    let _ = platform.active_app();
}

#[test]
fn dispatches_primary_accessibility_to_atspi() {
    let backend = StubBackend {
        atspi: PlatformAttemptResult::PermissionDenied,
        x11_selection: PlatformAttemptResult::Unavailable,
        clipboard: PlatformAttemptResult::Unavailable,
    };

    let result = LinuxPlatform::dispatch_attempt(&backend, CaptureMethod::AccessibilityPrimary);

    assert_eq!(result, PlatformAttemptResult::PermissionDenied);
}

#[test]
fn dispatches_range_accessibility_to_x11_selection() {
    let backend = StubBackend {
        atspi: PlatformAttemptResult::Unavailable,
        x11_selection: PlatformAttemptResult::EmptySelection,
        clipboard: PlatformAttemptResult::Unavailable,
    };

    let result = LinuxPlatform::dispatch_attempt(&backend, CaptureMethod::AccessibilityRange);

    assert_eq!(result, PlatformAttemptResult::EmptySelection);
}

#[test]
fn dispatches_clipboard_methods_to_clipboard_attempt() {
    let backend = StubBackend {
        atspi: PlatformAttemptResult::Unavailable,
        x11_selection: PlatformAttemptResult::Unavailable,
        clipboard: PlatformAttemptResult::Success("clipboard".into()),
    };

    assert_eq!(
        LinuxPlatform::dispatch_attempt(&backend, CaptureMethod::ClipboardBorrow),
        PlatformAttemptResult::Success("clipboard".into())
    );
    assert_eq!(
        LinuxPlatform::dispatch_attempt(&backend, CaptureMethod::SyntheticCopy),
        PlatformAttemptResult::Success("clipboard".into())
    );
}

#[test]
fn detects_linux_session_flags_from_env_presence() {
    assert_eq!(
        detect_linux_session(Some("wayland-0"), None),
        LinuxSession {
            wayland: true,
            x11: false,
        }
    );
    assert_eq!(
        detect_linux_session(None, Some(":0")),
        LinuxSession {
            wayland: false,
            x11: true,
        }
    );
    assert_eq!(
        detect_linux_session(Some(""), Some("")),
        LinuxSession {
            wayland: false,
            x11: false,
        }
    );
}

#[test]
fn clipboard_plan_prioritizes_wayland_only_session() {
    let plan = clipboard_command_plan(LinuxSession {
        wayland: true,
        x11: false,
    });
    assert_eq!(plan[0].program, "wl-paste");
    assert_eq!(plan[1].program, "wl-paste");
    assert_eq!(plan[0].args, &["--no-newline", "--type", "text"]);
}

#[test]
fn clipboard_plan_prioritizes_x11_only_session() {
    let plan = clipboard_command_plan(LinuxSession {
        wayland: false,
        x11: true,
    });
    assert_eq!(plan[0].program, "xclip");
    assert_eq!(plan[1].program, "xsel");
    assert_eq!(plan[0].args, &["-o", "-selection", "clipboard"]);
}

#[test]
fn primary_selection_plan_prioritizes_wayland_only_session() {
    let plan = primary_selection_command_plan(LinuxSession {
        wayland: true,
        x11: false,
    });
    assert_eq!(plan[0].program, "wl-paste");
    assert_eq!(plan[1].program, "wl-paste");
    assert_eq!(
        plan[0].args,
        &["--primary", "--no-newline", "--type", "text"]
    );
}
