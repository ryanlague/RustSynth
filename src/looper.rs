#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoopState {
    Stopped,
    Recording,
    Playing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LoopSettings {
    pub(crate) bpm: f64,
    pub(crate) beats: u32,
}

impl Default for LoopSettings {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            beats: 8,
        }
    }
}

impl LoopSettings {
    pub(crate) fn is_valid(&self) -> bool {
        self.bpm.is_finite() && (30.0..=300.0).contains(&self.bpm) && (1..=64).contains(&self.beats)
    }

    pub(crate) fn length_samples(&self, sample_rate: u32) -> usize {
        assert!(self.bpm.is_finite() && self.bpm > 0.0);
        assert!(self.beats > 0);
        assert!(sample_rate > 0);

        let seconds = self.beats as f64 * 60.0 / self.bpm;
        (seconds * sample_rate as f64).round().max(1.0) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LoopNoteEvent {
    On { id: u8, pitch: u8 },
    Off { id: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RecordedEvent {
    pub(crate) position_samples: usize,
    pub(crate) event: LoopNoteEvent,
}

pub(crate) struct Looper {
    state: LoopState,
    settings: LoopSettings,
    position_samples: usize,
    length_samples: usize,
    events: Vec<RecordedEvent>,
    next_event_idx: usize,
    held_notes: [bool; 256],
    armed: bool,
}

impl Looper {
    pub(crate) fn new(settings: LoopSettings, sample_rate: u32) -> Self {
        Self {
            state: LoopState::Stopped,
            settings,
            position_samples: 0,
            length_samples: settings.length_samples(sample_rate),
            events: Vec::with_capacity(16_384 + 256),
            next_event_idx: 0,
            held_notes: [false; 256],
            armed: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn events_for_test(&self) -> Vec<RecordedEvent> {
        self.events.clone()
    }

    pub(crate) fn record_event(&mut self, event: LoopNoteEvent) {
        if self.state != LoopState::Recording {
            return;
        }

        match event {
            LoopNoteEvent::On { id, .. } => {
                if self.held_notes[id as usize] || self.events.len() >= 16_384 {
                    return;
                }
            }
            LoopNoteEvent::Off { id } => {
                if !self.held_notes[id as usize] {
                    return;
                }
            }
        }
        self.events.push(RecordedEvent {
            position_samples: self.position_samples,
            event,
        });

        match event {
            LoopNoteEvent::On { id, .. } => {
                self.held_notes[id as usize] = true;
            }
            LoopNoteEvent::Off { id } => {
                self.held_notes[id as usize] = false;
            }
        }
    }

    pub(crate) fn configure(&mut self, settings: LoopSettings, sample_rate: u32) {
        assert_eq!(self.state, LoopState::Stopped);
        if self.settings != settings {
            self.events.clear();
        }
        self.settings = settings;
        self.length_samples = settings.length_samples(sample_rate);
    }

    pub(crate) fn state(&self) -> LoopState {
        self.state
    }
    #[cfg(test)]
    pub(crate) fn position(&self) -> usize {
        self.position_samples
    }
    #[cfg(test)]
    pub(crate) fn length(&self) -> usize {
        self.length_samples
    }
    #[cfg(test)]
    pub(crate) fn settings(&self) -> LoopSettings {
        self.settings
    }

    pub(crate) fn armed(&self) -> bool {
        self.armed
    }

    pub(crate) fn play(&mut self) {
        if self.state == LoopState::Stopped {
            self.position_samples = 0;
            self.next_event_idx = 0;
            self.armed = false;
            self.state = LoopState::Playing;
        }
    }

    pub(crate) fn arm_recording(&mut self) {
        if self.state == LoopState::Playing {
            self.armed = true;
        }
    }

    pub(crate) fn start_recording(&mut self) {
        self.armed = false;
        self.events.clear();
        self.held_notes.fill(false);
        self.position_samples = 0;
        self.next_event_idx = 0;
        self.state = LoopState::Recording;
    }

    fn finish_recording(&mut self, end_sample: usize) {
        for id in 0..self.held_notes.len() {
            if self.held_notes[id] {
                self.events.push(RecordedEvent {
                    position_samples: end_sample,
                    event: LoopNoteEvent::Off { id: id as u8 },
                });
                self.held_notes[id] = false;
            }
        }
    }

    pub(crate) fn stop(&mut self) {
        if self.state == LoopState::Recording {
            self.finish_recording(self.position_samples);
        }
        self.armed = false;
        self.position_samples = 0;
        self.next_event_idx = 0;
        self.state = LoopState::Stopped;
    }

    #[cfg(test)]
    pub(crate) fn advance(&mut self) {
        if self.state == LoopState::Stopped {
            return;
        }
        let next = self.position_samples + 1;
        let wrapped = next >= self.length_samples;
        self.follow_transport(if wrapped { 0 } else { next }, wrapped);
    }

    pub(crate) fn follow_transport(&mut self, position: usize, wrapped: bool) {
        if self.state == LoopState::Stopped {
            return;
        }
        if wrapped {
            if self.state == LoopState::Recording {
                self.finish_recording(self.length_samples - 1);
                self.state = LoopState::Playing;
            } else if self.armed {
                self.start_recording();
            }
            self.next_event_idx = 0;
        }
        self.position_samples = position;
    }

    pub(crate) fn next_event(&mut self) -> Option<LoopNoteEvent> {
        if self.state != LoopState::Playing {
            return None;
        }

        let recorded = self.events.get(self.next_event_idx)?;

        if recorded.position_samples != self.position_samples {
            return None;
        }

        self.next_event_idx += 1;
        Some(recorded.event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopping_a_partial_take_records_releases_before_replaying_it() {
        let mut looper = Looper::new(
            LoopSettings {
                bpm: 120.0,
                beats: 1,
            },
            8,
        );
        looper.start_recording();
        looper.record_event(LoopNoteEvent::On { id: 60, pitch: 60 });
        looper.advance();
        looper.stop();
        looper.play();
        assert!(matches!(
            looper.next_event(),
            Some(LoopNoteEvent::On { id: 60, .. })
        ));
        looper.advance();
        assert!(matches!(
            looper.next_event(),
            Some(LoopNoteEvent::Off { id: 60 })
        ));
    }

    #[test]
    fn armed_recording_waits_for_the_boundary_and_stop_cancels_it() {
        let mut looper = Looper::new(
            LoopSettings {
                bpm: 120.0,
                beats: 1,
            },
            8,
        );
        looper.play();
        looper.advance();
        looper.arm_recording();
        looper.record_event(LoopNoteEvent::On { id: 60, pitch: 60 });
        assert!(looper.events.is_empty());
        for _ in 0..2 {
            looper.advance();
        }
        assert_eq!(looper.state(), LoopState::Playing);
        assert!(looper.armed());
        assert_eq!(looper.position(), 3);
        looper.advance();
        assert_eq!(looper.state(), LoopState::Recording);
        assert_eq!(looper.position(), 0);
        assert!(!looper.armed());
        for _ in 0..4 {
            looper.advance();
        }
        assert_eq!(looper.state(), LoopState::Playing);
        looper.arm_recording();
        looper.stop();
        looper.play();
        for _ in 0..4 {
            looper.advance();
        }
        assert_eq!(looper.state(), LoopState::Playing);
        assert!(!looper.armed());
    }

    #[test]
    fn recorded_notes_repeat_and_held_notes_get_a_release() {
        let settings = LoopSettings {
            bpm: 120.0,
            beats: 1,
        };
        let mut looper = Looper::new(settings, 8);

        looper.start_recording();
        looper.record_event(LoopNoteEvent::On { id: 60, pitch: 60 });

        for _ in 0..4 {
            looper.advance();
        }

        // Check two complete playback passes.
        for _ in 0..2 {
            assert!(matches!(
                looper.next_event(),
                Some(LoopNoteEvent::On { id: 60, pitch: 60 })
            ));
            assert!(looper.next_event().is_none());

            for _ in 0..3 {
                looper.advance();
                if looper.position_samples < 3 {
                    assert!(looper.next_event().is_none());
                }
            }

            assert!(matches!(
                looper.next_event(),
                Some(LoopNoteEvent::Off { id: 60 })
            ));
            assert!(looper.next_event().is_none());

            looper.advance();
        }
    }

    #[test]
    fn recording_switches_to_playback_at_the_loop_boundary() {
        let settings = LoopSettings {
            bpm: 120.0,
            beats: 1,
        };
        let mut looper = Looper::new(settings, 8);

        // Half a second at 8 samples/sec gives a four-sample loop.
        assert_eq!(looper.length_samples, 4);

        looper.start_recording();
        for _ in 0..3 {
            looper.advance();
        }
        assert_eq!(looper.state, LoopState::Recording);
        assert_eq!(looper.position_samples, 3);

        looper.advance();
        assert_eq!(looper.state, LoopState::Playing);
        assert_eq!(looper.position_samples, 0);

        for _ in 0..4 {
            looper.advance();
        }
        assert_eq!(looper.state, LoopState::Playing);
        assert_eq!(looper.position_samples, 0);
    }
}
