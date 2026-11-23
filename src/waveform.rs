use crate::filter::BandPass;

use std::sync::atomic::{AtomicU64, Ordering};

/// Per-voice pseudo-random state: no locks or allocation during playback.
pub(crate) struct Noise {
    state: u64,
    filter: BandPass,
}

impl Noise {
    pub(crate) fn new() -> Self {
        static NEXT_SEED: AtomicU64 = AtomicU64::new(0x1234_5678_9abc_def0);
        Self {
            state: NEXT_SEED.fetch_add(1, Ordering::Relaxed),
            filter: BandPass::default(),
        }
    }

    pub(crate) fn tune(&mut self, frequency: f64, sample_rate: u32) {
        self.filter.tune(frequency, sample_rate);
    }

    pub(crate) fn reset_filter(&mut self) {
        self.filter.reset();
    }

    fn next_sample(&mut self) -> f64 {
        // SplitMix64, mapped uniformly to [-1, 1).
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut bits = self.state;
        bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        bits ^= bits >> 31;
        (bits >> 11) as f64 * (2.0 / 9_007_199_254_740_992.0) - 1.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Waveform {
    Sine,
    Triangle,
    Square,
    Sawtooth,
    WhiteNoise,
    FilteredNoise,
}

impl Waveform {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Sine => "Sine",
            Self::Triangle => "Triangle",
            Self::Square => "Square",
            Self::Sawtooth => "Sawtooth",
            Self::WhiteNoise => "White noise",
            Self::FilteredNoise => "Filtered noise",
        }
    }

    pub(crate) fn calculate(&self, phase: f64, amplitude: f64, noise: &mut Noise) -> f64 {
        match self {
            Waveform::Sine => phase.sin() * amplitude,
            Waveform::WhiteNoise => noise.next_sample() * amplitude,
            Waveform::FilteredNoise => {
                let sample = noise.next_sample();
                noise.filter.process(sample) * amplitude
            }
            Waveform::Square => {
                if phase.sin() >= 0.0 {
                    amplitude
                } else {
                    -amplitude
                }
            }
            Waveform::Sawtooth => {
                let cycle_position = phase / std::f64::consts::TAU;
                (2.0 * cycle_position - 1.0) * amplitude
            }
            Waveform::Triangle => {
                let cycle_position = phase / std::f64::consts::TAU;
                (1.0 - 4.0 * (cycle_position - 0.5).abs()) * amplitude
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_noise_is_bounded_balanced_and_not_tied_to_note_phase() {
        let mut a = Noise {
            state: 1,
            filter: BandPass::default(),
        };
        let mut b = Noise {
            state: 1,
            filter: BandPass::default(),
        };
        let mut sum = 0.0;
        let mut energy = 0.0;
        let mut correlation = 0.0;
        let mut previous = 0.0;
        for index in 0..16_384 {
            let sample = Waveform::WhiteNoise.calculate(0.0, 1.0, &mut a);
            assert_eq!(
                sample,
                Waveform::WhiteNoise.calculate(index as f64, 1.0, &mut b)
            );
            assert!((-1.0..1.0).contains(&sample));
            sum += sample;
            energy += sample * sample;
            correlation += previous * sample;
            previous = sample;
        }
        assert!((sum / 16_384.0).abs() < 0.02);
        assert!((0.3..0.37).contains(&(energy / 16_384.0)));
        assert!((correlation / energy).abs() < 0.04);
    }

    #[test]
    fn different_voices_do_not_start_with_identical_noise() {
        let mut a = Noise::new();
        let mut b = Noise::new();
        assert_ne!(a.next_sample(), b.next_sample());
    }
}
