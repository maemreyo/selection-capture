use super::*;
use crate::windows_observer::windows_observer_test_lock;
use crate::windows_subscriber::windows_native_subscriber_stats;
use crate::WindowsObserverBridge;

#[derive(Debug)]
struct StubBackend {
    ui_automation: PlatformAttemptResult,
    iaccessible: PlatformAttemptResult,
    clipboard: PlatformAttemptResult,
    synthetic_copy: PlatformAttemptResult,
}

impl WindowsBackend for StubBackend {
    fn attempt_ui_automation(&self) -> PlatformAttemptResult {
        self.ui_automation.clone()
    }

    fn attempt_iaccessible(&self) -> PlatformAttemptResult {
        self.iaccessible.clone()
    }

    fn attempt_clipboard(&self) -> PlatformAttemptResult {
        self.clipboard.clone()
    }

    fn attempt_synthetic_copy(&self) -> PlatformAttemptResult {
        self.synthetic_copy.clone()
    }
}

#[test]
fn constructor_builds_stub_platform() {
    let platform = WindowsPlatform::new();
    let _ = platform;
}

#[test]
fn selection_monitor_default_poll_interval_is_stable() {
    let monitor = WindowsSelectionMonitor::default();
    assert_eq!(monitor.poll_interval, Duration::from_millis(120));
    assert_eq!(monitor.backend(), WindowsMonitorBackend::Polling);
}

#[test]
fn selection_monitor_emits_only_new_values() {
    let monitor = WindowsSelectionMonitor::new(Duration::from_millis(10));
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
    let _guard = windows_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    fn pump() -> Vec<String> {
        vec![
            "  native a ".to_string(),
            "native a".to_string(),
            "native b".to_string(),
        ]
    }

    let monitor = WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
        poll_interval: Duration::from_millis(10),
        backend: WindowsMonitorBackend::NativeEventPreferred,
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
    let _guard = windows_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    let _ = WindowsObserverBridge::stop();
    let _ = WindowsObserverBridge::start();
    assert!(WindowsObserverBridge::push_event("bridge one"));
    assert!(WindowsObserverBridge::push_event("bridge two"));

    let monitor = WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
        poll_interval: Duration::from_millis(10),
        backend: WindowsMonitorBackend::NativeEventPreferred,
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
    assert!(WindowsObserverBridge::is_active());
    let _ = WindowsObserverBridge::stop();
}

#[test]
fn selection_monitor_native_preferred_releases_bridge_on_drop() {
    let _guard = windows_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    let _ = WindowsObserverBridge::stop();

    {
        let _monitor =
            WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
                poll_interval: Duration::from_millis(10),
                backend: WindowsMonitorBackend::NativeEventPreferred,
                native_queue_capacity: 8,
                native_event_pump: None,
            });
        assert!(WindowsObserverBridge::is_active());
    }

    assert!(!WindowsObserverBridge::is_active());
}

#[test]
fn selection_monitor_native_preferred_transitions_subscriber_manager_lifecycle() {
    let _guard = windows_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    let _ = WindowsObserverBridge::stop();
    let before = windows_native_subscriber_stats();

    {
        let _monitor =
            WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
                poll_interval: Duration::from_millis(10),
                backend: WindowsMonitorBackend::NativeEventPreferred,
                native_queue_capacity: 8,
                native_event_pump: None,
            });
        let during = windows_native_subscriber_stats();
        assert!(during.active);
        assert_eq!(during.starts, before.starts + 1);
    }

    let after = windows_native_subscriber_stats();
    assert!(after.stops > before.stops);
}

#[test]
fn selection_monitor_native_preferred_applies_queue_capacity() {
    let _guard = windows_observer_test_lock()
        .lock()
        .expect("test lock poisoned");
    let monitor = WindowsSelectionMonitor::new_with_options(WindowsSelectionMonitorOptions {
        poll_interval: Duration::from_millis(10),
        backend: WindowsMonitorBackend::NativeEventPreferred,
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
    let platform = WindowsPlatform::new();
    let _ = platform.active_app();
}

#[test]
fn dispatches_primary_accessibility_to_ui_automation() {
    let backend = StubBackend {
        ui_automation: PlatformAttemptResult::PermissionDenied,
        iaccessible: PlatformAttemptResult::Unavailable,
        clipboard: PlatformAttemptResult::Unavailable,
        synthetic_copy: PlatformAttemptResult::Unavailable,
    };

    let result =
        WindowsPlatform::dispatch_attempt(&backend, CaptureMethod::AccessibilityPrimary);

    assert_eq!(result, PlatformAttemptResult::PermissionDenied);
}

#[test]
fn dispatches_clipboard_methods_to_clipboard_attempt() {
    let backend = StubBackend {
        ui_automation: PlatformAttemptResult::Unavailable,
        iaccessible: PlatformAttemptResult::Unavailable,
        clipboard: PlatformAttemptResult::Success("clipboard".into()),
        synthetic_copy: PlatformAttemptResult::Success("synthetic".into()),
    };

    assert_eq!(
        WindowsPlatform::dispatch_attempt(&backend, CaptureMethod::ClipboardBorrow),
        PlatformAttemptResult::Success("clipboard".into())
    );
}

#[test]
fn dispatches_synthetic_copy_to_synthetic_copy_attempt() {
    let backend = StubBackend {
        ui_automation: PlatformAttemptResult::Unavailable,
        iaccessible: PlatformAttemptResult::Unavailable,
        clipboard: PlatformAttemptResult::Success("clipboard".into()),
        synthetic_copy: PlatformAttemptResult::Success("synthetic".into()),
    };

    assert_eq!(
        WindowsPlatform::dispatch_attempt(&backend, CaptureMethod::SyntheticCopy),
        PlatformAttemptResult::Success("synthetic".into())
    );
}

#[cfg(target_os = "windows")]
#[test]
fn normalizes_windows_text_stdout_and_strips_trailing_newline() {
    let raw = "line one\r\nline two\r\n";
    assert_eq!(
        normalize_windows_text_stdout(raw),
        Some("line one\nline two".to_string())
    );
}

#[cfg(target_os = "windows")]
#[test]
fn returns_none_when_windows_text_stdout_is_effectively_empty() {
    assert_eq!(normalize_windows_text_stdout("\r\n"), None);
    assert_eq!(normalize_windows_text_stdout(""), None);
}

#[cfg(target_os = "windows")]
#[test]
fn parses_active_app_stdout_with_path() {
    let raw = "NAME:Code\nPATH:C:\\Program Files\\Microsoft VS Code\\Code.exe\n";
    let parsed = parse_active_app_stdout(raw).expect("active app");

    assert_eq!(parsed.name, "Code");
    assert_eq!(
        parsed.bundle_id,
        "C:\\Program Files\\Microsoft VS Code\\Code.exe"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn parses_active_app_stdout_without_path_uses_process_fallback() {
    let raw = "NAME:Notepad\nPATH:\n";
    let parsed = parse_active_app_stdout(raw).expect("active app");

    assert_eq!(parsed.name, "Notepad");
    assert_eq!(parsed.bundle_id, "process://notepad");
}
