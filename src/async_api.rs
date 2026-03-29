use crate::traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
use crate::types::{CaptureOptions, CaptureOutcome};

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
        Ok(_) | Err(_) => crate::capture(platform, store, cancel, adapters, options),
    }
}
