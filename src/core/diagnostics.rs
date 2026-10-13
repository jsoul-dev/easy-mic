//! Lightweight latency instrumentation for hotkeys, microphone operations, sound, and UI.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static CORRELATION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_correlation_id() -> u64 {
    CORRELATION_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct LatencyRecord {
    pub correlation_id: u64,
    pub action_name: String,
    pub t_input_event: Instant,
    pub t_hotkey_recognized: Option<Instant>,
    pub t_action_queued: Option<Instant>,
    pub t_worker_started: Option<Instant>,
    pub t_endpoint_call_begin: Option<Instant>,
    pub t_endpoint_call_end: Option<Instant>,
    pub t_mute_verified: Option<Instant>,
    pub t_sound_requested: Option<Instant>,
    pub t_sound_started: Option<Instant>,
    pub t_ui_posted: Option<Instant>,
    pub t_ui_updated: Option<Instant>,
}

impl LatencyRecord {
    pub fn new(correlation_id: u64, action_name: &str, t_input_event: Instant) -> Self {
        Self {
            correlation_id,
            action_name: action_name.to_string(),
            t_input_event,
            t_hotkey_recognized: None,
            t_action_queued: None,
            t_worker_started: None,
            t_endpoint_call_begin: None,
            t_endpoint_call_end: None,
            t_mute_verified: None,
            t_sound_requested: None,
            t_sound_started: None,
            t_ui_posted: None,
            t_ui_updated: None,
        }
    }

    pub fn input_to_recognition_dur(&self) -> Option<Duration> {
        self.t_hotkey_recognized
            .map(|t| t.saturating_duration_since(self.t_input_event))
    }

    pub fn queue_delay_dur(&self) -> Option<Duration> {
        match (self.t_action_queued, self.t_worker_started) {
            (Some(q), Some(w)) => Some(w.saturating_duration_since(q)),
            _ => None,
        }
    }

    pub fn endpoint_call_dur(&self) -> Option<Duration> {
        match (self.t_endpoint_call_begin, self.t_endpoint_call_end) {
            (Some(b), Some(e)) => Some(e.saturating_duration_since(b)),
            _ => None,
        }
    }

    pub fn verified_transition_latency(&self) -> Option<Duration> {
        self.t_mute_verified
            .map(|v| v.saturating_duration_since(self.t_input_event))
    }

    pub fn sound_start_latency(&self) -> Option<Duration> {
        self.t_sound_started
            .map(|s| s.saturating_duration_since(self.t_input_event))
    }

    pub fn ui_update_delay(&self) -> Option<Duration> {
        self.t_ui_updated
            .map(|u| u.saturating_duration_since(self.t_input_event))
    }

    pub fn summary(&self) -> String {
        format!(
            "Action [{}] ID {}: Recog={:?}, QueueDelay={:?}, Endpoint={:?}, TotalVerified={:?}, SoundStart={:?}, UI={:?}",
            self.action_name,
            self.correlation_id,
            self.input_to_recognition_dur().unwrap_or_default(),
            self.queue_delay_dur().unwrap_or_default(),
            self.endpoint_call_dur().unwrap_or_default(),
            self.verified_transition_latency().unwrap_or_default(),
            self.sound_start_latency().unwrap_or_default(),
            self.ui_update_delay().unwrap_or_default(),
        )
    }
}

pub struct DiagnosticsBuffer {
    records: Mutex<Vec<LatencyRecord>>,
    max_capacity: usize,
}

impl Default for DiagnosticsBuffer {
    fn default() -> Self {
        Self::new(64)
    }
}

impl DiagnosticsBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            records: Mutex::new(Vec::with_capacity(max_capacity)),
            max_capacity,
        }
    }

    pub fn record(&self, item: LatencyRecord) {
        let mut recs = self.records.lock();
        if recs.len() >= self.max_capacity {
            recs.remove(0);
        }
        recs.push(item);
    }

    pub fn latest(&self) -> Option<LatencyRecord> {
        let recs = self.records.lock();
        recs.last().cloned()
    }

    pub fn get_summary(&self) -> DiagnosticsSummary {
        let recs = self.records.lock();
        if recs.is_empty() {
            return DiagnosticsSummary::default();
        }

        let mut sum_ep = 0.0;
        let mut count_ep = 0;
        let mut sum_total = 0.0;
        let mut count_total = 0;

        for r in recs.iter() {
            if let Some(d) = r.endpoint_call_dur() {
                sum_ep += d.as_secs_f64() * 1000.0;
                count_ep += 1;
            }
            if let Some(d) = r.verified_transition_latency() {
                sum_total += d.as_secs_f64() * 1000.0;
                count_total += 1;
            }
        }

        DiagnosticsSummary {
            total_count: recs.len(),
            avg_endpoint_latency_ms: if count_ep > 0 {
                sum_ep / count_ep as f64
            } else {
                0.0
            },
            avg_total_latency_ms: if count_total > 0 {
                sum_total / count_total as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticsSummary {
    pub total_count: usize,
    pub avg_endpoint_latency_ms: f64,
    pub avg_total_latency_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_record_calculations() {
        let now = Instant::now();
        let mut rec = LatencyRecord::new(1, "Toggle mute", now);
        rec.t_hotkey_recognized = Some(now + Duration::from_millis(1));
        rec.t_action_queued = Some(now + Duration::from_millis(2));
        rec.t_worker_started = Some(now + Duration::from_millis(4));
        rec.t_endpoint_call_begin = Some(now + Duration::from_millis(5));
        rec.t_endpoint_call_end = Some(now + Duration::from_millis(8));
        rec.t_mute_verified = Some(now + Duration::from_millis(9));
        rec.t_sound_started = Some(now + Duration::from_millis(11));
        rec.t_ui_updated = Some(now + Duration::from_millis(14));

        assert!(rec.input_to_recognition_dur().unwrap() <= Duration::from_millis(2));
        assert_eq!(rec.queue_delay_dur().unwrap(), Duration::from_millis(2));
        assert_eq!(rec.endpoint_call_dur().unwrap(), Duration::from_millis(3));
        assert_eq!(
            rec.verified_transition_latency().unwrap(),
            Duration::from_millis(9)
        );
        assert_eq!(
            rec.sound_start_latency().unwrap(),
            Duration::from_millis(11)
        );
        assert_eq!(rec.ui_update_delay().unwrap(), Duration::from_millis(14));
    }
}
