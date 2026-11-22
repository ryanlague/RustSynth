use std::sync::{Arc, Mutex, MutexGuard};

/// A short history of the actual mono output, shared with the UI.
#[derive(Clone)]
pub(crate) struct Oscilloscope {
    buffer: Arc<Mutex<SampleBuffer>>,
    sample_count: usize,
}

struct SampleBuffer {
    samples: Vec<f32>,
    next: usize,
    triggers: Vec<bool>,
}

pub(crate) struct ScopeCapture<'a> {
    buffer: MutexGuard<'a, SampleBuffer>,
}

impl ScopeCapture<'_> {
    pub(crate) fn push(&mut self, sample: f32, trigger: bool) {
        let next = self.buffer.next;
        self.buffer.samples[next] = sample;
        self.buffer.triggers[next] = trigger;
        self.buffer.next = (next + 1) % self.buffer.samples.len();
    }
}

impl Oscilloscope {
    pub(crate) fn new(sample_rate: u32) -> Self {
        // Show 25 milliseconds at the output device's sample rate.
        let sample_count = (sample_rate as usize / 40).max(2);
        Self {
            buffer: Arc::new(Mutex::new(SampleBuffer {
                // Extra history lets a triggered view start earlier without
                // shortening or circularly rotating the displayed waveform.
                samples: vec![0.0; sample_count * 2],
                triggers: vec![false; sample_count * 2],
                next: 0,
            })),
            sample_count,
        }
    }

    pub(crate) fn sample_count(&self) -> usize {
        self.sample_count
    }

    pub(crate) fn try_capture(&self) -> Option<ScopeCapture<'_>> {
        // Audio never waits for drawing; all storage is allocated up front.
        self.buffer
            .try_lock()
            .ok()
            .map(|buffer| ScopeCapture { buffer })
    }

    #[cfg(test)]
    pub(crate) fn capture(&self, output: &[f32], channels: usize) {
        if let Some(mut capture) = self.try_capture() {
            for frame in output.chunks_exact(channels) {
                capture.push(frame[0], false);
            }
        }
    }

    pub(crate) fn snapshot(&self, output: &mut [f32], triggered: bool) {
        assert_eq!(output.len(), self.sample_count);
        if let Ok(buffer) = self.buffer.lock() {
            let history_len = buffer.samples.len();
            // Find the latest trigger that leaves a full 25 ms window after it.
            // With no usable trigger (silence, startup, very low frequencies),
            // fall back to the latest unaligned window.
            let start = if triggered {
                (0..=self.sample_count)
                    .rev()
                    .find(|offset| buffer.triggers[(buffer.next + offset) % history_len])
                    .unwrap_or(self.sample_count)
            } else {
                self.sample_count
            };
            for (offset, sample) in output.iter_mut().enumerate() {
                *sample = buffer.samples[(buffer.next + start + offset) % history_len];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_latest_frames_in_time_order_and_uses_one_channel() {
        let scope = Oscilloscope::new(160); // Four samples in 25 ms.
        let mut snapshot = vec![0.0; scope.sample_count()];
        scope.capture(&[0.1, 0.1, 0.2, 0.2], 2);
        scope.snapshot(&mut snapshot, false);
        assert_eq!(snapshot, [0.0, 0.0, 0.1, 0.2]);
        scope.capture(&[0.3, 0.3, 0.4, 0.4, 0.5, 0.5], 2);
        scope.snapshot(&mut snapshot, false);
        assert_eq!(snapshot, [0.2, 0.3, 0.4, 0.5]);
    }

    #[test]
    fn capture_skips_busy_ui_without_waiting_and_silence_clears_history() {
        let scope = Oscilloscope::new(160);
        let audio_scope = scope.clone();
        let guard = scope.buffer.lock().unwrap();
        // This would deadlock if capture waited on the UI's lock.
        audio_scope.capture(&[1.0; 4], 1);
        drop(guard);
        let mut snapshot = vec![0.0; scope.sample_count()];
        scope.snapshot(&mut snapshot, false);
        assert_eq!(snapshot, [0.0; 4]);
        audio_scope.capture(&[0.5; 4], 1);
        audio_scope.capture(&[0.0; 4], 1);
        scope.snapshot(&mut snapshot, false);
        assert_eq!(snapshot, [0.0; 4]);
    }

    #[test]
    fn trigger_selects_a_contiguous_full_window_even_after_history_wraps() {
        let scope = Oscilloscope::new(160);
        {
            let mut capture = scope.try_capture().unwrap();
            for index in 0..12 {
                capture.push(index as f32, index % 3 == 0);
            }
        }
        let mut snapshot = vec![0.0; 4];
        scope.snapshot(&mut snapshot, false);
        assert_eq!(snapshot, [8.0, 9.0, 10.0, 11.0]);
        scope.snapshot(&mut snapshot, true);
        assert_eq!(snapshot, [6.0, 7.0, 8.0, 9.0]);
        // Silence overwrites both the signal and its old trigger markers.
        scope.capture(&[0.0; 8], 1);
        scope.snapshot(&mut snapshot, true);
        assert_eq!(snapshot, [0.0; 4]);
    }

    #[test]
    fn no_trigger_falls_back_to_the_latest_samples() {
        let scope = Oscilloscope::new(160);
        scope.capture(&[0.1, -0.1, 0.2, -0.2, 0.3], 1);
        let mut snapshot = vec![0.0; 4];
        scope.snapshot(&mut snapshot, true);
        assert_eq!(snapshot, [-0.1, 0.2, -0.2, 0.3]);
    }
}
