use crate::traits::MonitorPlatform;

pub struct CaptureMonitor<P> {
    platform: P,
}

impl<P> CaptureMonitor<P>
where
    P: MonitorPlatform,
{
    pub fn new(platform: P) -> Self {
        Self { platform }
    }

    pub fn next_event(&self) -> Option<String> {
        self.platform.next_selection_change()
    }
}
