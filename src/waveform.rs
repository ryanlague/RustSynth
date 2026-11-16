#[derive(Clone, Copy)]
pub(crate) enum Waveform {
    Sine,
    Triangle,
    Square,
    Sawtooth,
}

impl Waveform {
    pub(crate) fn calculate(&self, phase: f64, amplitude: f64) -> f64 {
        match self {
            Waveform::Sine => phase.sin() * amplitude,
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
