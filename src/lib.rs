mod engine;
#[cfg(feature = "linux-alpha")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(feature = "windows-beta")]
mod windows;
pub mod platform;
mod profile;
mod traits;
mod types;

pub use engine::capture;
#[cfg(feature = "linux-alpha")]
pub use linux::LinuxPlatform;
#[cfg(target_os = "macos")]
pub use macos::MacOSPlatform;
pub use platform::PlatformCapabilities;
pub use profile::{AppProfile, AppProfileUpdate, TriState};
pub use traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
#[cfg(feature = "windows-beta")]
pub use windows::WindowsPlatform;
pub use types::{
    ActiveApp, CaptureFailure, CaptureFailureContext, CaptureMethod, CaptureOptions,
    CaptureOutcome, CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus, FailureKind,
    PlatformAttemptResult, RetryPolicy, TraceEvent, UserHint,
};
