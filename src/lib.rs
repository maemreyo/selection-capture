#[cfg(feature = "async")]
mod async_api;
#[cfg(target_os = "macos")]
mod ax_observer;
mod cache;
mod engine;
#[cfg(feature = "linux-alpha")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod monitor;
pub mod platform;
mod profile;
mod traits;
mod types;
#[cfg(feature = "windows-beta")]
mod windows;

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
pub use traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform, MonitorPlatform};
pub use types::{
    ActiveApp, CaptureFailure, CaptureFailureContext, CaptureMethod, CaptureOptions,
    CaptureOutcome, CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus, FailureKind,
    PlatformAttemptResult, RetryPolicy, TraceEvent, UserHint, WouldBlock,
};
#[cfg(feature = "windows-beta")]
pub use windows::{
    WindowsMonitorBackend, WindowsNativeEventPump, WindowsPlatform, WindowsSelectionMonitor,
    WindowsSelectionMonitorOptions,
};
