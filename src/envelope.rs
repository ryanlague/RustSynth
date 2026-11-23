#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ADSR {
    attack_sec: f64,
    decay_sec: f64,
    sustain_level: f64,
    release_sec: f64,
}

impl ADSR {
    pub(crate) fn new(a: f64, d: f64, s: f64, r: f64) -> Self {
        Self {
            attack_sec: a,
            decay_sec: d,
            sustain_level: s,
            release_sec: r,
        }
    }

    pub(crate) fn attack_sec(&self) -> f64 {
        self.attack_sec
    }

    pub(crate) fn decay_sec(&self) -> f64 {
        self.decay_sec
    }

    pub(crate) fn sustain_level(&self) -> f64 {
        self.sustain_level
    }

    pub(crate) fn release_sec(&self) -> f64 {
        self.release_sec
    }

    fn held_gain(&self, time: f64) -> f64 {
        if time < self.attack_sec {
            time / self.attack_sec
        } else if time < self.attack_sec + self.decay_sec {
            let progress = (time - self.attack_sec) / self.decay_sec;
            1.0 + (self.sustain_level - 1.0) * progress
        } else {
            self.sustain_level
        }
    }

    pub(crate) fn gain(&self, time: f64, note_off: f64) -> f64 {
        if time < note_off {
            self.held_gain(time)
        } else if self.release_sec > 0.0 && time < note_off + self.release_sec {
            let release_start_gain = self.held_gain(note_off);
            let progress = (time - note_off) / self.release_sec;
            release_start_gain * (1.0 - progress)
        } else {
            0.0
        }
    }
}

impl Default for ADSR {
    fn default() -> Self {
        Self::new(0.01, 0.1, 0.7, 0.2)
    }
}
