use crate::traits::{CancelSignal, MonitorPlatform};
use crate::types::{CaptureMethod, CaptureOutcome, CaptureStatus, TraceEvent};
use std::thread;
use std::time::{Duration, Instant};

pub struct CaptureMonitor<P> {
    platform: P,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorSpamGuard {
    pub suppress_identical: bool,
    pub min_emit_interval: Duration,
    pub min_emit_interval_same_text: Duration,
    pub normalize_whitespace: bool,
}

impl Default for MonitorSpamGuard {
    fn default() -> Self {
        Self {
            suppress_identical: true,
            min_emit_interval: Duration::ZERO,
            min_emit_interval_same_text: Duration::ZERO,
            normalize_whitespace: false,
        }
    }
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

    pub fn run<F>(&self, mut on_event: F) -> usize
    where
        F: FnMut(String),
    {
        let mut processed = 0;
        while let Some(event) = self.next_event() {
            on_event(event);
            processed += 1;
        }
        processed
    }

    pub fn run_with_limit<F>(&self, max_events: usize, mut on_event: F) -> usize
    where
        F: FnMut(String),
    {
        if max_events == 0 {
            return 0;
        }
        let mut processed = 0;
        while processed < max_events {
            match self.next_event() {
                Some(event) => {
                    on_event(event);
                    processed += 1;
                }
                None => break,
            }
        }
        processed
    }

    pub fn collect_events(&self, max_events: usize) -> Vec<String> {
        let mut events = Vec::new();
        self.run_with_limit(max_events, |event| events.push(event));
        events
    }

    pub fn poll_until<F, C>(
        &self,
        poll_interval: Duration,
        mut should_continue: C,
        mut on_event: F,
    ) -> usize
    where
        F: FnMut(String),
        C: FnMut() -> bool,
    {
        let mut processed = 0;
        while should_continue() {
            if let Some(event) = self.next_event() {
                on_event(event);
                processed += 1;
                continue;
            }
            thread::sleep(poll_interval);
        }
        processed
    }

    pub fn poll_until_cancelled<F, S>(
        &self,
        poll_interval: Duration,
        cancel: &S,
        on_event: F,
    ) -> usize
    where
        F: FnMut(String),
        S: CancelSignal,
    {
        self.poll_until(poll_interval, || !cancel.is_cancelled(), on_event)
    }

    pub fn poll_until_cancelled_coalesced<F, S>(
        &self,
        poll_interval: Duration,
        min_emit_interval: Duration,
        cancel: &S,
        mut on_event: F,
    ) -> usize
    where
        F: FnMut(String),
        S: CancelSignal,
    {
        let mut processed = 0;
        let mut last_emit_at: Option<Instant> = None;

        while !cancel.is_cancelled() {
            if let Some(event) = self.next_event() {
                let should_emit = last_emit_at
                    .map(|last| last.elapsed() >= min_emit_interval)
                    .unwrap_or(true);
                if should_emit {
                    on_event(event);
                    processed += 1;
                    last_emit_at = Some(Instant::now());
                }
                continue;
            }
            thread::sleep(poll_interval);
        }

        processed
    }

    pub fn poll_until_cancelled_guarded<F, S>(
        &self,
        poll_interval: Duration,
        cancel: &S,
        guard: &MonitorSpamGuard,
        mut on_event: F,
    ) -> usize
    where
        F: FnMut(String),
        S: CancelSignal,
    {
        let mut processed = 0;
        let mut last_emit_at: Option<Instant> = None;
        let mut last_emitted_text: Option<String> = None;

        while !cancel.is_cancelled() {
            if let Some(event) = self.next_event() {
                let normalized = normalize_event_text(&event, guard.normalize_whitespace);
                let now = Instant::now();
                let too_soon_global = last_emit_at
                    .map(|last| now.duration_since(last) < guard.min_emit_interval)
                    .unwrap_or(false);
                let same_as_last = last_emitted_text
                    .as_ref()
                    .map(|last| last == &normalized)
                    .unwrap_or(false);
                let too_soon_same = same_as_last
                    && last_emit_at
                        .map(|last| now.duration_since(last) < guard.min_emit_interval_same_text)
                        .unwrap_or(false);
                let blocked_duplicate = guard.suppress_identical && same_as_last;

                if too_soon_global || too_soon_same || blocked_duplicate {
                    continue;
                }

                on_event(event);
                processed += 1;
                last_emit_at = Some(now);
                last_emitted_text = Some(normalized);
                continue;
            }
            thread::sleep(poll_interval);
        }

        processed
    }
}

fn normalize_event_text(input: &str, normalize_whitespace: bool) -> String {
    if !normalize_whitespace {
        return input.to_string();
    }

    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MethodMetrics {
    pub attempts: u64,
    pub successes: u64,
    pub empty_results: u64,
    pub failures: u64,
    pub total_latency: Duration,
}

impl MethodMetrics {
    pub fn success_rate(&self) -> f64 {
        if self.attempts == 0 {
            return 0.0;
        }
        self.successes as f64 / self.attempts as f64
    }

    pub fn average_latency(&self) -> Duration {
        if self.attempts == 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(self.total_latency.as_secs_f64() / self.attempts as f64)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaptureMetrics {
    pub total_captures: u64,
    pub successes: u64,
    pub failures: u64,
    pub timed_out: u64,
    pub cancelled: u64,
    pub total_latency: Duration,
    methods: Vec<(CaptureMethod, MethodMetrics)>,
}

impl CaptureMetrics {
    pub fn record_outcome(&mut self, outcome: &CaptureOutcome) {
        self.total_captures += 1;
        match outcome {
            CaptureOutcome::Success(success) => {
                self.successes += 1;
                if let Some(trace) = &success.trace {
                    self.total_latency += trace.total_elapsed;
                    self.record_trace_events(&trace.events);
                }
            }
            CaptureOutcome::Failure(failure) => {
                self.failures += 1;
                if failure.status == CaptureStatus::TimedOut {
                    self.timed_out += 1;
                }
                if failure.status == CaptureStatus::Cancelled {
                    self.cancelled += 1;
                }
                if let Some(trace) = &failure.trace {
                    self.total_latency += trace.total_elapsed;
                    self.record_trace_events(&trace.events);
                }
            }
        }
    }

    pub fn overall_success_rate(&self) -> f64 {
        if self.total_captures == 0 {
            return 0.0;
        }
        self.successes as f64 / self.total_captures as f64
    }

    pub fn average_latency(&self) -> Duration {
        if self.total_captures == 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(self.total_latency.as_secs_f64() / self.total_captures as f64)
    }

    pub fn method_metrics(&self, method: CaptureMethod) -> Option<&MethodMetrics> {
        self.methods
            .iter()
            .find_map(|(candidate, metrics)| (*candidate == method).then_some(metrics))
    }

    fn record_trace_events(&mut self, events: &[TraceEvent]) {
        for event in events {
            match event {
                TraceEvent::MethodFinished { method, elapsed } => {
                    let metrics = self.metrics_mut(*method);
                    metrics.attempts += 1;
                    metrics.total_latency += *elapsed;
                }
                TraceEvent::MethodSucceeded(method) => {
                    self.metrics_mut(*method).successes += 1;
                }
                TraceEvent::MethodReturnedEmpty(method) => {
                    self.metrics_mut(*method).empty_results += 1;
                }
                TraceEvent::MethodFailed { method, .. } => {
                    self.metrics_mut(*method).failures += 1;
                }
                _ => {}
            }
        }
    }

    fn metrics_mut(&mut self, method: CaptureMethod) -> &mut MethodMetrics {
        if let Some(index) = self
            .methods
            .iter()
            .position(|(candidate, _)| *candidate == method)
        {
            return &mut self.methods[index].1;
        }
        self.methods.push((method, MethodMetrics::default()));
        let index = self.methods.len() - 1;
        &mut self.methods[index].1
    }
}
