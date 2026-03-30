use crate::traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
use crate::types::{CaptureOptions, CaptureOutcome};

/// Async wrapper around [`crate::capture`].
///
/// On a **multi-thread** Tokio runtime this uses `block_in_place` to offload
/// the blocking work to the current thread without stalling other tasks.
///
/// On a **current-thread** (`LocalSet`) runtime `block_in_place` is unavailable
/// because there is only one worker thread. In that case the underlying sync
/// call runs directly on the async task thread and will block the executor for
/// its duration.  If you are using a current-thread runtime, call the
/// synchronous [`crate::capture`] API from a `spawn_blocking` closure at the
/// call site instead — that avoids blocking all other tasks on the runtime.
pub async fn capture_async(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureOptions,
) -> CaptureOutcome {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| {
                crate::capture(platform, store, cancel, adapters, options)
            })
        }
        // Current-thread or no-runtime path: run synchronously.
        // NOTE: This blocks the executor thread for the duration of the capture.
        // See the function-level doc comment for the recommended workaround.
        Ok(_) | Err(_) => crate::capture(platform, store, cancel, adapters, options),
    }
}
