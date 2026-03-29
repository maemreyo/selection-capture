mod engine;
#[cfg(target_os = "macos")]
mod macos;
pub mod platform;
mod profile;
mod traits;
mod types;

pub use engine::capture;
#[cfg(target_os = "macos")]
pub use macos::MacOSPlatform;
pub use platform::PlatformCapabilities;
pub use profile::{AppProfile, AppProfileUpdate, TriState};
pub use traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
pub use types::{
    ActiveApp, CaptureFailure, CaptureFailureContext, CaptureMethod, CaptureOptions,
    CaptureOutcome, CaptureStatus, CaptureSuccess, CaptureTrace, CleanupStatus, FailureKind,
    PlatformAttemptResult, RetryPolicy, TraceEvent, UserHint,
};
