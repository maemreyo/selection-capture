#[cfg(feature = "async")]
mod async_api;
#[cfg(target_os = "macos")]
mod ax_observer;
mod cache;
mod engine;
#[cfg(feature = "linux-alpha")]
mod linux;
#[cfg(feature = "linux-alpha")]
mod linux_observer;
#[cfg(feature = "linux-alpha")]
mod linux_runtime_adapter;
#[cfg(feature = "linux-alpha")]
mod linux_shell;
#[cfg(feature = "linux-alpha")]
mod linux_subscriber;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_ax;
mod monitor;
mod native_subscriber;
mod observer_bridge;
pub mod platform;
mod profile;
#[cfg(feature = "rich-content")]
mod rich_clipboard;
#[cfg(feature = "rich-content")]
mod rich_convert;
#[cfg(feature = "rich-content")]
mod rich_engine;
#[cfg(feature = "rich-content")]
mod rich_types;
mod traits;
mod types;
#[cfg(feature = "windows-beta")]
mod windows;
#[cfg(feature = "windows-beta")]
mod windows_observer;
#[cfg(feature = "windows-beta")]
mod windows_runtime_adapter;
#[cfg(feature = "windows-beta")]
mod windows_subscriber;

#[cfg(feature = "async")]
pub use async_api::capture_async;
#[cfg(target_os = "macos")]
pub use ax_observer::{
    drain_events_for_monitor as ax_observer_drain_events_for_monitor, AxObserverBridge,
};
pub use engine::{capture, try_capture};
#[cfg(feature = "linux-alpha")]
pub use linux::{
    LinuxMonitorBackend, LinuxNativeEventPump, LinuxPlatform, LinuxSelectionMonitor,
    LinuxSelectionMonitorOptions,
};
#[cfg(feature = "linux-alpha")]
pub use linux_observer::{
    drain_events_for_monitor as linux_observer_drain_events_for_monitor, LinuxObserverBridge,
    LinuxObserverLifecycleHook,
};
#[cfg(feature = "linux-alpha")]
pub use linux_runtime_adapter::{
    install_default_linux_runtime_adapter_if_absent, linux_default_runtime_adapter_state,
    linux_default_runtime_event_source_registered, set_linux_default_runtime_event_source,
    LinuxDefaultRuntimeAdapterState, LinuxDefaultRuntimeEventSource,
};
#[cfg(feature = "linux-alpha")]
pub use linux_subscriber::{
    ensure_linux_native_subscriber_hook_installed, linux_native_subscriber_stats,
    set_linux_native_runtime_adapter, LinuxNativeRuntimeAdapter, LinuxNativeSubscriberStats,
};
#[cfg(target_os = "macos")]
pub use macos::{
    MacOSMonitorBackend, MacOSNativeEventPump, MacOSNativeEventSource, MacOSNativeObserverStats,
    MacOSPlatform, MacOSSelectionMonitor, MacOSSelectionMonitorOptions,
};
pub use monitor::{
    CaptureMetrics, CaptureMonitor, MethodMetrics, MonitorGuardStats, MonitorSpamGuard,
};
pub use platform::PlatformCapabilities;
pub use profile::{AppProfile, AppProfileUpdate, TriState};
#[cfg(feature = "rich-content")]
pub use rich_engine::{capture_rich, try_capture_rich};
#[cfg(feature = "rich-content")]
pub use rich_types::{
    CaptureRichOptions, CaptureRichOutcome, CaptureRichSuccess, CapturedContent, ContentMetadata,
    RichConversion, RichFormat, RichPayload, RichSource,
};
pub use traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform, MonitorPlatform};
pub use types::{
    ActiveApp, CGPoint, CGRect, CGSize, CaptureFailure, CaptureFailureContext, CaptureMethod,
    CaptureOptions, CaptureOutcome, CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus,
    FailureKind, PlatformAttemptResult, RetryPolicy, TraceEvent, UserHint, WouldBlock,
};
#[cfg(feature = "windows-beta")]
pub use windows::{
    WindowsMonitorBackend, WindowsNativeEventPump, WindowsPlatform, WindowsSelectionMonitor,
    WindowsSelectionMonitorOptions,
};
#[cfg(feature = "windows-beta")]
pub use windows_observer::{
    drain_events_for_monitor as windows_observer_drain_events_for_monitor, WindowsObserverBridge,
    WindowsObserverLifecycleHook,
};
#[cfg(feature = "windows-beta")]
pub use windows_runtime_adapter::{
    install_default_windows_runtime_adapter_if_absent, set_windows_default_runtime_event_source,
    windows_default_runtime_adapter_state, windows_default_runtime_event_source_registered,
    WindowsDefaultRuntimeAdapterState, WindowsDefaultRuntimeEventSource,
};
#[cfg(feature = "windows-beta")]
pub use windows_subscriber::{
    ensure_windows_native_subscriber_hook_installed, set_windows_native_runtime_adapter,
    windows_native_subscriber_stats, WindowsNativeRuntimeAdapter, WindowsNativeSubscriberStats,
};

/// Reads the currently focused window frame from the platform without running text capture.
pub fn capture_window_frame(platform: &impl CapturePlatform) -> Option<CGRect> {
    platform.focused_window_frame()
}
