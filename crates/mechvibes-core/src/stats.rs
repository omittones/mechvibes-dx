use std::sync::atomic::{AtomicU64, Ordering};

/// Events older than this when dequeued are discarded rather than played.
pub const STALE_THRESHOLD_MS: u64 = 500;

pub struct RuntimeStats {
    pub keypresses: AtomicU64,
    pub mouse_presses: AtomicU64,
    /// Sum of playback latencies in microseconds (for running average without floats).
    pub latency_sum_us: AtomicU64,
    pub latency_count: AtomicU64,
    pub discarded: AtomicU64,
}

impl RuntimeStats {
    pub fn record_latency(&self, received_at: std::time::Instant) {
        let us = received_at.elapsed().as_micros() as u64;
        self.latency_sum_us.fetch_add(us, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn avg_latency_ms(&self) -> f64 {
        let count = self.latency_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        self.latency_sum_us.load(Ordering::Relaxed) as f64 / count as f64 / 1000.0
    }
}

pub static STATS: RuntimeStats = RuntimeStats {
    keypresses: AtomicU64::new(0),
    mouse_presses: AtomicU64::new(0),
    latency_sum_us: AtomicU64::new(0),
    latency_count: AtomicU64::new(0),
    discarded: AtomicU64::new(0),
};
