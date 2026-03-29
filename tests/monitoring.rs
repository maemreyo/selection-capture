use selection_capture::{CaptureMonitor, MonitorPlatform};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct StubMonitorPlatform {
    events: Arc<Mutex<VecDeque<Option<String>>>>,
}

impl StubMonitorPlatform {
    fn new(events: Vec<Option<&str>>) -> Self {
        Self {
            events: Arc::new(Mutex::new(
                events
                    .into_iter()
                    .map(|event| event.map(str::to_owned))
                    .collect(),
            )),
        }
    }
}

impl MonitorPlatform for StubMonitorPlatform {
    fn next_selection_change(&self) -> Option<String> {
        self.events.lock().unwrap().pop_front().flatten()
    }
}

#[test]
fn monitor_emits_events_in_backend_order() {
    let platform = StubMonitorPlatform::new(vec![Some("first"), Some("second"), None]);
    let monitor = CaptureMonitor::new(platform);

    assert_eq!(monitor.next_event(), Some("first".to_string()));
    assert_eq!(monitor.next_event(), Some("second".to_string()));
    assert_eq!(monitor.next_event(), None);
}
