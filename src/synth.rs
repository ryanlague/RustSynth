use crate::{envelope::ADSR, note::Note, voice::Voice, waveform::Waveform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VoiceMode {
    Polyphonic,
    Monophonic,
    Legato,
}

impl VoiceMode {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Polyphonic => "Polyphonic",
            Self::Monophonic => "Monophonic",
            Self::Legato => "Legato",
        }
    }
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
    env: ADSR,
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
            live_notes: Vec::with_capacity(256),
            env: ADSR::default(),
        }
    }

    pub(crate) fn set_waveform(&mut self, waveform: Waveform) {
        for voice in &mut self.voices {
            voice.set_waveform(waveform);
        }
    }

    pub(crate) fn set_envelope(&mut self, env: ADSR) {
        self.env = env;
    }

    pub(crate) fn set_voice_mode(&mut self, mode: VoiceMode, sample_rate: u32) {
        if self.mode != mode {
            self.all_notes_off(sample_rate);
            self.held_notes.clear();
            self.next_voice = 0;
            self.mode = mode;
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.voices.iter().any(|voice| voice.active)
    }

    pub(crate) fn envelope(&self) -> ADSR {
        self.env
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

    pub(crate) fn note_on(
        &mut self,
        id: u8,
        frequency: f64,
        amplitude: f64,
        global_amplitude: f64,
        sample_rate: u32,
    ) {
        if self.live_notes.iter().any(|held| held.id == id) {
            return;
        }

        // Each live press remembers the setting at note-on, including when
        // it is later resumed by monophonic last-note priority.
        let note = Note {
            frequency,
            start_sec: 0.0,
            duration_sec: 0.0,
            amplitude,
            env: self.env,
        };

        let keep_envelope = matches!(self.mode, VoiceMode::Legato) && !self.live_notes.is_empty();
        let voice_index = self.allocate_voice();

        if matches!(self.mode, VoiceMode::Polyphonic) {
            self.live_notes
                .retain(|held| held.voice_index != voice_index);
        }

        let voice = &mut self.voices[voice_index];

        if keep_envelope {
            voice.transition_held(&note, global_amplitude, sample_rate);
        } else {
            voice.start_held(&note, global_amplitude);
        }

        self.live_notes.push(LiveNote {
            id,
            note,
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

    pub(crate) fn lowest_oscillator(&self) -> Option<(f64, f64)> {
        self.voices
            .iter()
            .filter_map(Voice::oscillator_position)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    #[cfg(test)]
    pub(crate) fn scope_trigger(&self, sample_rate: u32) -> bool {
        self.lowest_oscillator().is_some_and(|(frequency, phase)| {
            phase < std::f64::consts::TAU * frequency / f64::from(sample_rate)
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
            synth.note_on(1, note.frequency, note.amplitude, 0.25, sample_rate);
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
        synth.note_on(1, notes[0].frequency, notes[0].amplitude, 0.25, 1_000);
        synth.note_on(2, notes[1].frequency, notes[1].amplitude, 0.25, 1_000);
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
            synth.note_on(1, notes[0].frequency, notes[0].amplitude, 0.25, 1_000);
            expected.start_held(&notes[0], 0.25);
            for _ in 0..37 {
                assert_eq!(synth.next_sample(1_000), expected.next_sample(1_000));
            }
            synth.note_on(2, notes[1].frequency, notes[1].amplitude, 0.25, 1_000);
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
            synth.note_on(1, notes[0].frequency, notes[0].amplitude, 0.25, 1_000);
            synth.note_on(2, notes[1].frequency, notes[1].amplitude, 0.25, 1_000);
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
        synth.note_on(1, note.frequency, note.amplitude, 0.25, 1_000);
        note.frequency = 250.0;
        synth.note_on(2, note.frequency, note.amplitude, 0.25, 1_000);
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

    #[test]
    fn live_envelope_changes_affect_new_notes_without_changing_release_tails() {
        let mut synth = Synth::new(2, Waveform::Sine, VoiceMode::Polyphonic);
        let mut first = demo_1().remove(0);
        first.env = synth.envelope();
        let mut old_voice = Voice::new(0.0, 0.0, Waveform::Sine);
        old_voice.start_held(&first, 0.25);
        synth.note_on(1, first.frequency, first.amplitude, 0.25, 1_000);
        for _ in 0..100 {
            assert_eq!(synth.next_sample(1_000), old_voice.next_sample(1_000));
        }
        synth.note_off(1, 0.25, 1_000);
        old_voice.release(1_000);
        let mut second = first.clone();
        second.frequency = 330.0;
        second.env = ADSR::new(0.3, 0.2, 0.25, 0.8);
        synth.set_envelope(second.env);
        let mut new_voice = Voice::new(0.0, 0.0, Waveform::Sine);
        new_voice.start_held(&second, 0.25);
        synth.note_on(2, second.frequency, second.amplitude, 0.25, 1_000);
        for _ in 0..500 {
            let expected = old_voice.next_sample(1_000) + new_voice.next_sample(1_000);
            assert_eq!(synth.next_sample(1_000), expected);
        }
    }

    #[test]
    fn returning_mono_notes_and_connected_legato_keep_their_envelopes() {
        for mode in [VoiceMode::Monophonic, VoiceMode::Legato] {
            let legato = matches!(mode, VoiceMode::Legato);
            let mut synth = Synth::new(1, Waveform::Sine, mode);
            let mut first = demo_1().remove(0);
            first.env = synth.envelope();
            let mut expected = Voice::new(0.0, 0.0, Waveform::Sine);
            expected.start_held(&first, 0.25);
            synth.note_on(1, first.frequency, first.amplitude, 0.25, 1_000);
            for _ in 0..37 {
                assert_eq!(synth.next_sample(1_000), expected.next_sample(1_000));
            }
            let mut second = first.clone();
            second.frequency = 330.0;
            second.env = ADSR::new(0.4, 0.2, 0.1, 0.5);
            synth.set_envelope(second.env);
            synth.note_on(2, second.frequency, second.amplitude, 0.25, 1_000);
            if legato {
                expected.transition_held(&second, 0.25, 1_000);
            } else {
                expected.start_held(&second, 0.25);
            }
            for _ in 0..53 {
                assert_eq!(synth.next_sample(1_000), expected.next_sample(1_000));
            }
            synth.note_off(2, 0.25, 1_000);
            if legato {
                expected.transition_held(&first, 0.25, 1_000);
            } else {
                expected.start_held(&first, 0.25);
            }
            for _ in 0..100 {
                assert_eq!(synth.next_sample(1_000), expected.next_sample(1_000));
            }
        }
    }

    #[test]
    fn scheduled_notes_use_their_own_envelopes() {
        let mut synth = Synth::new(1, Waveform::Sine, VoiceMode::Polyphonic);
        synth.set_envelope(ADSR::new(1.0, 1.0, 0.0, 2.0));
        let note = demo_1().remove(0);
        let mut expected = Voice::new(0.0, 0.0, Waveform::Sine);
        expected.start(&note, 0.25);
        synth.play_note(&note, 0.25, 1_000);
        for _ in 0..4_000 {
            assert_eq!(synth.next_sample(1_000), expected.next_sample(1_000));
        }
    }

    #[test]
    fn changing_voice_mode_releases_old_notes_and_uses_the_new_allocation() {
        let modes = [
            VoiceMode::Polyphonic,
            VoiceMode::Monophonic,
            VoiceMode::Legato,
        ];
        for from in modes {
            for to in modes {
                if from == to {
                    continue;
                }
                let mut synth = Synth::new(3, Waveform::Sine, from);
                synth.note_on(1, 220.0, 0.5, 0.25, 1_000);
                synth.note_on(2, 330.0, 0.5, 0.25, 1_000);
                for _ in 0..100 {
                    synth.next_sample(1_000);
                }
                synth.set_voice_mode(to, 1_000);
                assert_eq!(synth.mode, to);
                assert!(synth.live_notes.is_empty());
                assert!(synth.voices.iter().any(|voice| voice.active));
                for _ in 0..210 {
                    synth.next_sample(1_000);
                }
                assert!(synth.voices.iter().all(|voice| !voice.active));
                // IDs from the old mode can be pressed again, with fresh tracking.
                synth.note_on(1, 220.0, 0.5, 0.25, 1_000);
                synth.note_on(2, 330.0, 0.5, 0.25, 1_000);
                let active = synth.voices.iter().filter(|voice| voice.active).count();
                assert_eq!(active, if to == VoiceMode::Polyphonic { 2 } else { 1 });
                synth.note_off(2, 0.25, 1_000);
                synth.note_off(1, 0.25, 1_000);
                for _ in 0..210 {
                    synth.next_sample(1_000);
                }
                assert!(synth.voices.iter().all(|voice| !voice.active));
            }
        }
    }

    #[test]
    fn selecting_the_current_mode_does_not_release_held_notes() {
        let mut synth = Synth::new(2, Waveform::Sine, VoiceMode::Polyphonic);
        synth.note_on(1, 220.0, 0.5, 0.25, 1_000);
        synth.set_voice_mode(VoiceMode::Polyphonic, 1_000);
        assert_eq!(synth.live_notes.len(), 1);
        for _ in 0..1_000 {
            synth.next_sample(1_000);
        }
        assert!(synth.voices[0].active);
    }
}
