use crate::looper::LoopSettings;

/// The single sample clock followed by every loop track and the metronome.
pub(crate) struct Transport {
    pub(crate) running: bool,
    pub(crate) settings: LoopSettings,
    pub(crate) position: usize,
    pub(crate) length: usize,
}

impl Transport {
    pub(crate) fn new(sample_rate: u32) -> Self {
        let settings = LoopSettings::default();
        Self {
            running: false,
            settings,
            position: 0,
            length: settings.length_samples(sample_rate),
        }
    }

    pub(crate) fn play(&mut self, settings: LoopSettings, sample_rate: u32) {
        self.settings = settings;
        self.length = settings.length_samples(sample_rate);
        self.position = 0;
        self.running = true;
    }

    pub(crate) fn set_bpm(&mut self, bpm: f64, sample_rate: u32) {
        let old_position = self.position;
        let new_position = (self.position as f64 * self.settings.bpm / bpm).round() as usize;
        self.settings.bpm = bpm;
        self.length = self.settings.length_samples(sample_rate);
        self.position = new_position
            .max(usize::from(old_position > 0))
            .min(self.length - 1);
    }

    pub(crate) fn stop(&mut self) {
        self.running = false;
        self.position = 0;
    }

    pub(crate) fn advance(&mut self) -> bool {
        if !self.running {
            return false;
        }
        self.position += 1;
        if self.position == self.length {
            self.position = 0;
            return true;
        }
        false
    }
}
