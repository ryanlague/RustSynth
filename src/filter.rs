/// A unity-peak band-pass biquad with a fixed, broad Q of 1.
/// Coefficients: https://www.w3.org/TR/audio-eq-cookbook/ (RBJ).
#[derive(Default)]
pub(crate) struct BandPass {
    frequency: f64,
    sample_rate: u32,
    b0: f64,
    a1: f64,
    a2: f64,
    delay1: f64,
    delay2: f64,
}

impl BandPass {
    pub(crate) fn tune(&mut self, frequency: f64, sample_rate: u32) {
        let rate = sample_rate.max(1);
        let frequency = if frequency.is_finite() {
            frequency
        } else {
            440.0
        };
        let frequency = frequency.clamp(0.001, f64::from(rate) * 0.45);
        if self.frequency == frequency && self.sample_rate == rate {
            return;
        }
        self.frequency = frequency;
        self.sample_rate = rate;
        let omega = std::f64::consts::TAU * frequency / f64::from(rate);
        let alpha = omega.sin() / 2.0;
        let a0 = 1.0 + alpha;
        self.b0 = alpha / a0;
        self.a1 = -2.0 * omega.cos() / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    pub(crate) fn reset(&mut self) {
        self.delay1 = 0.0;
        self.delay2 = 0.0;
    }

    pub(crate) fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.delay1;
        self.delay1 = -self.a1 * output + self.delay2;
        self.delay2 = -self.b0 * input - self.a2 * output;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(center: f64, input_frequency: f64) -> f64 {
        let mut filter = BandPass::default();
        filter.tune(center, 48_000);
        let mut energy = 0.0;
        for index in 0..48_000 {
            let input = (std::f64::consts::TAU * input_frequency * index as f64 / 48_000.0).sin();
            let output = filter.process(input);
            assert!(output.is_finite());
            if index >= 4_000 {
                energy += output * output;
            }
        }
        (energy / 44_000.0).sqrt()
    }

    #[test]
    fn band_pass_tracks_its_center_and_attenuates_distant_frequencies() {
        let center = response(1_000.0, 1_000.0);
        assert!((0.69..0.72).contains(&center));
        assert!(response(1_000.0, 100.0) < center * 0.15);
        assert!(response(1_000.0, 10_000.0) < center * 0.15);
        assert!(response(100.0, 100.0) > response(1_000.0, 100.0) * 5.0);
    }
}
