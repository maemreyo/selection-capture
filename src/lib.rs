#[cfg(feature = "async")]
mod async_api;
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
pub use engine::{capture, try_capture};
#[cfg(feature = "linux-alpha")]
pub use linux::{LinuxPlatform, LinuxSelectionMonitor};
#[cfg(target_os = "macos")]
pub use macos::{MacOSPlatform, MacOSSelectionMonitor};
pub use monitor::{CaptureMetrics, CaptureMonitor, MethodMetrics};
pub use platform::PlatformCapabilities;
pub use profile::{AppProfile, AppProfileUpdate, TriState};
pub use traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform, MonitorPlatform};
pub use types::{
    ActiveApp, CaptureFailure, CaptureFailureContext, CaptureMethod, CaptureOptions,
    CaptureOutcome, CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus, FailureKind,
    PlatformAttemptResult, RetryPolicy, TraceEvent, UserHint, WouldBlock,
};
#[cfg(feature = "windows-beta")]
pub use windows::{WindowsPlatform, WindowsSelectionMonitor};
