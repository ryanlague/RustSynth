pub(crate) struct Metronome {
    sample_rate: u32,
    enabled: bool,
    next_beat: u32,
    elapsed: usize,
    remaining: usize,
    frequency: f64,
    accent: bool,
}

impl Metronome {
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            enabled: true,
            next_beat: 0,
            elapsed: 0,
            remaining: 0,
            frequency: 0.0,
            accent: false,
        }
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.remaining = 0;
        }
    }

    pub(crate) fn next_sample(
        &mut self,
        position: Option<usize>,
        length: usize,
        beats: u32,
    ) -> f64 {
        let Some(position) = position else {
            self.remaining = 0;
            self.next_beat = 0;
            return 0.0;
        };
        if position == 0 {
            self.next_beat = 0;
        }
        let beat_sample =
            (f64::from(self.next_beat) * length as f64 / f64::from(beats)).round() as usize;
        if self.next_beat < beats && position >= beat_sample {
            if self.enabled {
                self.accent = self.next_beat == 0;
                self.frequency = if self.accent { 1760.0 } else { 1175.0 };
                self.elapsed = 0;
                self.remaining = (f64::from(self.sample_rate) * 0.015).ceil() as usize;
            }
            self.next_beat += 1;
        }
        if self.remaining == 0 {
            return 0.0;
        }
        let duration = self.elapsed + self.remaining;
        let gain = self.remaining as f64 / duration as f64;
        let phase = std::f64::consts::TAU * self.frequency * self.elapsed as f64
            / f64::from(self.sample_rate);
        self.elapsed += 1;
        self.remaining -= 1;
        phase.sin() * gain * gain * if self.accent { 0.16 } else { 0.12 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_beat_positions_stay_aligned_across_loop_passes() {
        let mut click = Metronome::new(8_000);
        click.set_enabled(true);
        let length = 11_670;
        for _ in 0..3 {
            for position in 0..length {
                click.next_sample(Some(position), length, 3);
                if position == 0 {
                    assert_eq!(click.frequency, 1760.0);
                }
                if position == 3_890 {
                    assert_eq!(click.frequency, 1175.0);
                    assert_eq!(click.next_beat, 2);
                }
                if position == 7_780 {
                    assert_eq!(click.next_beat, 3);
                }
            }
        }
        assert_eq!(click.next_sample(None, length, 3), 0.0);
    }
}
