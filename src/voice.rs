use crate::{envelope::ADSR, note::Note, waveform::Waveform};

pub(crate) struct Voice {
    pub(crate) active: bool,
    frequency: f64,
    amplitude: f64,
    phase: f64,
    waveform: Waveform,
    elapsed_samples: usize,
    note_off_sec: f64,
    env: ADSR,
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
        }
    }

    pub(crate) fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    pub(crate) fn set_envelope(&mut self, env: ADSR) {
        self.env = env;
    }

    pub(crate) fn oscillator_position(&self) -> Option<(f64, f64)> {
        if self.active && self.frequency > 0.0 {
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

            let gain = self.env.gain(time, self.note_off_sec);
            let sample = self.waveform.calculate(self.phase, self.amplitude * gain);

            let phase_step = std::f64::consts::TAU * self.frequency / sample_rate as f64;
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
