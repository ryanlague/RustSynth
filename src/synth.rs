use crate::{note::Note, voice::Voice, waveform::Waveform};

pub(crate) enum VoiceMode {
    Polyphonic,
    Monophonic,
    Legato,
}

struct LiveNote {
    id: u8,
    note: Note,
    voice_index: usize,
}

pub(crate) struct Synth {
    voices: Vec<Voice>,
    next_voice: usize,
    mode: VoiceMode,
    held_notes: Vec<Note>,
    live_notes: Vec<LiveNote>,
}

impl Synth {
    pub(crate) fn new(voice_count: usize, waveform: Waveform, mode: VoiceMode) -> Self {
        assert!(voice_count > 0, "Synth needs at least one voice");

        let mut voices = Vec::new();

        for _ in 0..voice_count {
            voices.push(Voice::new(0.0, 0.0, waveform.clone()))
        }

        Self {
            voices,
            next_voice: 0,
            mode,
            held_notes: Vec::new(),
            live_notes: Vec::new(),
        }
    }

    pub(crate) fn set_waveform(&mut self, waveform: Waveform) {
        for voice in &mut self.voices {
            voice.set_waveform(waveform);
        }
    }

    fn allocate_voice(&mut self) -> usize {
        match self.mode {
            VoiceMode::Monophonic | VoiceMode::Legato => return 0,
            VoiceMode::Polyphonic => {
                let count = self.voices.len();
                let mut selected = self.next_voice;

                for offset in 0..count {
                    let idx = (self.next_voice + offset) % count;

                    if !self.voices[idx].active {
                        selected = idx;
                        break;
                    }
                }

                self.next_voice = (selected + 1) % count;
                return selected;
            }
        }
    }

    pub(crate) fn play_note(&mut self, note: &Note, global_amplitude: f64, sample_rate: u32) {
        let keep_envelope = matches!(self.mode, VoiceMode::Legato) && !self.held_notes.is_empty();

        match self.mode {
            VoiceMode::Monophonic | VoiceMode::Legato => {
                self.held_notes.push(note.clone());
            }
            VoiceMode::Polyphonic => {}
        }
        let voice_idx = self.allocate_voice();
        let voice = &mut self.voices[voice_idx];

        if keep_envelope {
            voice.transition(note, global_amplitude, sample_rate)
        } else {
            voice.start(note, global_amplitude);
        }
    }

    pub(crate) fn note_on(&mut self, id: u8, note: &Note, global_amplitude: f64, sample_rate: u32) {
        if self.live_notes.iter().any(|held| held.id == id) {
            return;
        }

        let keep_envelope = matches!(self.mode, VoiceMode::Legato) && !self.live_notes.is_empty();
        let voice_index = self.allocate_voice();

        if matches!(self.mode, VoiceMode::Polyphonic) {
            self.live_notes
                .retain(|held| held.voice_index != voice_index);
        }

        let voice = &mut self.voices[voice_index];

        if keep_envelope {
            voice.transition_held(note, global_amplitude, sample_rate);
        } else {
            voice.start_held(note, global_amplitude);
        }

        self.live_notes.push(LiveNote {
            id,
            note: note.clone(),
            voice_index,
        });
    }

    pub(crate) fn note_off(&mut self, id: u8, global_amplitude: f64, sample_rate: u32) {
        let Some(index) = self.live_notes.iter().position(|held| held.id == id) else {
            return;
        };

        let was_latest = index == self.live_notes.len() - 1;
        let released = self.live_notes.remove(index);

        match self.mode {
            VoiceMode::Polyphonic => {
                self.voices[released.voice_index].release(sample_rate);
            }
            VoiceMode::Monophonic | VoiceMode::Legato => {
                if !was_latest {
                    return;
                }

                if let Some(held) = self.live_notes.last() {
                    let voice = &mut self.voices[0];
                    if matches!(self.mode, VoiceMode::Legato) {
                        voice.transition_held(&held.note, global_amplitude, sample_rate);
                    } else {
                        voice.start_held(&held.note, global_amplitude);
                    }
                } else {
                    self.voices[0].release(sample_rate);
                }
            }
        }
    }

    pub(crate) fn all_notes_off(&mut self, sample_rate: u32) {
        self.live_notes.clear();
        for voice in &mut self.voices {
            voice.release(sample_rate);
        }
    }

    fn remove_ended_note(&mut self, time: f64) -> bool {
        let latest_ended = match self.held_notes.last() {
            Some(note) => time >= note.start_sec + note.duration_sec,
            None => false,
        };

        self.held_notes
            .retain(|note| time < note.start_sec + note.duration_sec);

        latest_ended
    }

    pub(crate) fn update_held_notes(&mut self, time: f64, global_amplitude: f64, sample_rate: u32) {
        if !self.remove_ended_note(time) {
            return;
        }

        if let Some(note) = self.held_notes.last() {
            let mut resumed_note = note.clone();

            resumed_note.duration_sec = note.start_sec + note.duration_sec - time;

            match self.mode {
                VoiceMode::Legato => {
                    self.voices[0].transition(&resumed_note, global_amplitude, sample_rate)
                }
                VoiceMode::Monophonic => {
                    self.voices[0].start(&resumed_note, global_amplitude);
                }
                VoiceMode::Polyphonic => {}
            }
        }
    }

    pub(crate) fn next_sample(&mut self, sample_rate: u32) -> f64 {
        let mut mixed_sample = 0.0;

        for voice in &mut self.voices {
            mixed_sample += voice.next_sample(sample_rate);
        }
        mixed_sample
    }

    /// Marks the start of a cycle of the lowest sounding voice, including tails.
    pub(crate) fn scope_trigger(&self, sample_rate: u32) -> bool {
        self.voices
            .iter()
            .filter_map(Voice::oscillator_position)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .is_some_and(|(frequency, phase)| {
                phase < std::f64::consts::TAU * frequency / sample_rate as f64
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_1;

    #[test]
    fn live_notes_hold_until_release_in_all_modes() {
        let note = demo_1().remove(0);
        let sample_rate = 1_000;
        for mode in [
            VoiceMode::Polyphonic,
            VoiceMode::Monophonic,
            VoiceMode::Legato,
        ] {
            let mut synth = Synth::new(2, Waveform::Sine, mode);
            synth.note_on(1, &note, 0.25, sample_rate);
            for _ in 0..4_000 {
                synth.next_sample(sample_rate);
            }
            assert!(synth.voices[0].active);
            synth.note_off(1, 0.25, sample_rate);
            assert!(synth.voices[0].active);
            for _ in 0..210 {
                synth.next_sample(sample_rate);
            }
            assert!(!synth.voices[0].active);
        }
    }

    #[test]
    fn releasing_stolen_note_does_not_release_its_replacement() {
        let notes = demo_1();
        let mut synth = Synth::new(1, Waveform::Sine, VoiceMode::Polyphonic);
        synth.note_on(1, &notes[0], 0.25, 1_000);
        synth.note_on(2, &notes[1], 0.25, 1_000);
        synth.note_off(1, 0.25, 1_000);
        for _ in 0..2_000 {
            synth.next_sample(1_000);
        }
        assert!(synth.voices[0].active);
        synth.note_off(2, 0.25, 1_000);
        for _ in 0..210 {
            synth.next_sample(1_000);
        }
        assert!(!synth.voices[0].active);
    }

    #[test]
    fn mono_and_legato_return_to_the_previous_held_note() {
        let notes = demo_1();
        for mode in [VoiceMode::Monophonic, VoiceMode::Legato] {
            let legato = matches!(mode, VoiceMode::Legato);
            let mut synth = Synth::new(2, Waveform::Sine, mode);
            let mut expected = Voice::new(0.0, 0.0, Waveform::Sine);
            synth.note_on(1, &notes[0], 0.25, 1_000);
            expected.start_held(&notes[0], 0.25);
            for _ in 0..37 {
                assert_eq!(synth.next_sample(1_000), expected.next_sample(1_000));
            }
            synth.note_on(2, &notes[1], 0.25, 1_000);
            if legato {
                expected.transition_held(&notes[1], 0.25, 1_000);
            } else {
                expected.start_held(&notes[1], 0.25);
            }
            for _ in 0..53 {
                assert_eq!(synth.next_sample(1_000), expected.next_sample(1_000));
            }
            synth.note_off(2, 0.25, 1_000);
            if legato {
                expected.transition_held(&notes[0], 0.25, 1_000);
            } else {
                expected.start_held(&notes[0], 0.25);
            }
            for _ in 0..100 {
                assert_eq!(synth.next_sample(1_000), expected.next_sample(1_000));
            }
            assert_eq!(synth.live_notes.len(), 1);
            assert_eq!(synth.live_notes[0].id, 1);
        }
    }

    #[test]
    fn all_notes_off_clears_held_keys_and_finishes_release_tails() {
        let notes = demo_1();
        for mode in [
            VoiceMode::Polyphonic,
            VoiceMode::Monophonic,
            VoiceMode::Legato,
        ] {
            let mut synth = Synth::new(2, Waveform::Sine, mode);
            synth.note_on(1, &notes[0], 0.25, 1_000);
            synth.note_on(2, &notes[1], 0.25, 1_000);
            for _ in 0..100 {
                synth.next_sample(1_000);
            }
            synth.all_notes_off(1_000);
            assert!(synth.live_notes.is_empty());
            for _ in 0..210 {
                synth.next_sample(1_000);
            }
            assert!(synth.voices.iter().all(|voice| !voice.active));
        }
    }

    #[test]
    fn scope_trigger_follows_the_lowest_sounding_voice_and_its_release_tail() {
        let mut synth = Synth::new(2, Waveform::Sine, VoiceMode::Polyphonic);
        let mut note = demo_1().remove(0);
        note.frequency = 125.0;
        assert!(!synth.scope_trigger(1_000));
        synth.note_on(1, &note, 0.25, 1_000);
        note.frequency = 250.0;
        synth.note_on(2, &note, 0.25, 1_000);
        assert!(synth.scope_trigger(1_000));
        for _ in 0..4 {
            synth.next_sample(1_000);
        }
        // The higher voice has wrapped, but the lower one is only half-way.
        assert!(!synth.scope_trigger(1_000));
        synth.note_off(1, 0.25, 1_000);
        assert!(!synth.scope_trigger(1_000));
        for _ in 0..4 {
            synth.next_sample(1_000);
        }
        assert!(synth.scope_trigger(1_000));
        for _ in 0..201 {
            synth.next_sample(1_000);
        }
        assert!(!synth.voices[0].active);
        assert!(synth.voices[1].active);
        // Read-only display queries do not affect synthesis.
        let before = synth.voices[1].oscillator_position();
        synth.scope_trigger(1_000);
        assert_eq!(before, synth.voices[1].oscillator_position());
    }
}
