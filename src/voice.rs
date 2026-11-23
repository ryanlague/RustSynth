use crate::{
    envelope::ADSR,
    note::Note,
    waveform::{Noise, Waveform},
};

pub(crate) struct Voice {
    pub(crate) active: bool,
    frequency: f64,
    amplitude: f64,
    phase: f64,
    waveform: Waveform,
    elapsed_samples: usize,
    note_off_sec: f64,
    env: ADSR,
    noise: Noise,
}

impl Voice {
    pub(crate) fn new(frequency: f64, amplitude: f64, waveform: Waveform) -> Self {
        Self {
            active: false,
            frequency,
            amplitude,
            phase: 0.0,
            waveform,
            elapsed_samples: 0,
            note_off_sec: 0.0,
            env: ADSR::new(0.0, 0.0, 1.0, 0.0),
            noise: Noise::new(),
        }
    }

    pub(crate) fn set_waveform(&mut self, waveform: Waveform) {
        if self.waveform != waveform {
            self.noise.reset_filter();
        }
        self.waveform = waveform;
    }

    pub(crate) fn set_envelope(&mut self, env: ADSR) {
        self.env = env;
    }

    pub(crate) fn oscillator_position(&self) -> Option<(f64, f64)> {
        if self.active
            && self.frequency > 0.0
            && !matches!(
                self.waveform,
                Waveform::WhiteNoise | Waveform::FilteredNoise | Waveform::Kick
            )
        {
            Some((self.frequency, self.phase))
        } else {
            None
        }
    }

    pub(crate) fn release(&mut self, sample_rate: u32) {
        if !self.active {
            return;
        }

        let elapsed = self.elapsed_samples as f64 / sample_rate as f64;
        self.note_off_sec = self.note_off_sec.min(elapsed);
    }

    pub(crate) fn start(&mut self, note: &Note, global_amplitude: f64) {
        self.frequency = note.frequency;
        self.amplitude = note.amplitude * global_amplitude;
        self.note_off_sec = note.duration_sec;
        self.set_envelope(note.env);

        self.noise.reset_filter();
        self.phase = 0.0;
        self.elapsed_samples = 0;
        self.active = true;
    }

    pub(crate) fn start_held(&mut self, note: &Note, global_amplitude: f64) {
        self.start(note, global_amplitude);
        self.note_off_sec = f64::INFINITY;
    }

    pub(crate) fn next_sample(&mut self, sample_rate: u32) -> f64 {
        if !self.active {
            0.0
        } else {
            let time = self.elapsed_samples as f64 / sample_rate as f64;

            if time >= self.note_off_sec + self.env.release_sec() {
                self.active = false;
                return 0.0;
            }

            if self.waveform == Waveform::FilteredNoise {
                self.noise.tune(self.frequency, sample_rate);
            }
            let gain = self.env.gain(time, self.note_off_sec);
            let sample =
                self.waveform
                    .calculate(self.phase, self.amplitude * gain, &mut self.noise);

            let frequency = self.waveform.frequency_at(self.frequency, time);
            let phase_step = std::f64::consts::TAU * frequency / sample_rate as f64;
            self.phase = (self.phase + phase_step) % std::f64::consts::TAU;
            self.elapsed_samples += 1;

            sample
        }
    }

    pub(crate) fn transition(&mut self, note: &Note, global_amplitude: f64, sample_rate: u32) {
        self.frequency = note.frequency;
        self.amplitude = note.amplitude * global_amplitude as f64;

        let elapsed = self.elapsed_samples as f64 / sample_rate as f64;
        self.note_off_sec = elapsed + note.duration_sec;
    }

    pub(crate) fn transition_held(&mut self, note: &Note, global_amplitude: f64, sample_rate: u32) {
        self.transition(note, global_amplitude, sample_rate);
        self.note_off_sec = f64::INFINITY;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kick_retriggers_consistently_and_decays_while_held() {
        let preset = crate::presets::synth_presets()
            .into_iter()
            .find(|preset| preset.name == "Kick")
            .unwrap();
        let note = Note {
            frequency: 65.0,
            start_sec: 0.0,
            duration_sec: 0.0,
            amplitude: 0.5,
            env: preset.settings.env,
        };
        let mut voice = Voice::new(0.0, 0.0, Waveform::Kick);
        voice.start_held(&note, 0.25);
        let first: Vec<_> = (0..24_000).map(|_| voice.next_sample(48_000)).collect();
        assert!(
            first
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 0.125)
        );
        assert!(first[..10_000].iter().any(|sample| sample.abs() > 0.05));
        assert!(first[16_000..].iter().all(|sample| *sample == 0.0));
        voice.start_held(&note, 0.25);
        for sample in first.iter().take(2_000) {
            assert_eq!(voice.next_sample(48_000), *sample);
        }
    }

    #[test]
    fn noise_percussion_decays_to_silence_while_held_and_releases_normally() {
        for waveform in [Waveform::WhiteNoise, Waveform::FilteredNoise] {
            let mut voice = Voice::new(0.0, 0.0, waveform);
            let note = Note {
                frequency: 440.0,
                start_sec: 0.0,
                duration_sec: 0.0,
                amplitude: 0.5,
                env: ADSR::new(0.0, 0.02, 0.0, 0.01),
            };
            voice.start_held(&note, 0.25);
            let samples: Vec<_> = (0..1_440).map(|_| voice.next_sample(48_000)).collect();
            assert!(samples[..960].iter().any(|sample| *sample != 0.0));
            assert!(samples.iter().all(|sample| sample.is_finite()));
            assert!(samples[960..].iter().all(|sample| *sample == 0.0));
            assert!(voice.oscillator_position().is_none());
            voice.release(48_000);
            for _ in 0..500 {
                voice.next_sample(48_000);
            }
            assert!(!voice.active);
        }
    }
}
