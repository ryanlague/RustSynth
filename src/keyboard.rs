use eframe::egui::Key;

use crate::synth_event::SynthEvent;

#[derive(Default)]
pub(crate) struct Keys {
    held: [bool; 18],
}

impl Keys {
    pub(crate) fn handle(
        &mut self,
        key: Key,
        pressed: bool,
        repeat: bool,
        shift: bool,
        control: bool,
    ) -> Option<SynthEvent> {
        let id = key_note(key)?;
        let held = &mut self.held[usize::from(id - 60)];
        if pressed && !repeat && !*held {
            *held = true;
            let pitch = match (shift, control) {
                (true, false) => id + 12,
                (false, true) => id - 12,
                _ => id,
            };
            Some(SynthEvent::On { id, pitch })
        } else if !pressed && *held {
            *held = false;
            Some(SynthEvent::Off(id))
        } else {
            None
        }
    }

    pub(crate) fn clear(&mut self) {
        self.held.fill(false);
    }

    pub(crate) fn held_count(&self) -> usize {
        self.held.iter().filter(|&&held| held).count()
    }
}

fn key_note(key: Key) -> Option<u8> {
    Some(match key {
        Key::A => 60,
        Key::W => 61,
        Key::S => 62,
        Key::E => 63,
        Key::D => 64,
        Key::F => 65,
        Key::T => 66,
        Key::G => 67,
        Key::Y => 68,
        Key::H => 69,
        Key::U => 70,
        Key::J => 71,
        Key::K => 72,
        Key::O => 73,
        Key::L => 74,
        Key::P => 75,
        Key::Semicolon => 76,
        Key::Quote => 77,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_repeat_and_unmatched_releases_are_ignored() {
        let mut keys = Keys::default();
        assert!(matches!(
            keys.handle(Key::A, true, false, false, false),
            Some(SynthEvent::On { id: 60, pitch: 60 })
        ));
        assert!(keys.handle(Key::A, true, true, false, false).is_none());
        assert!(keys.handle(Key::A, true, false, false, false).is_none());
        assert!(matches!(
            keys.handle(Key::A, false, false, false, false),
            Some(SynthEvent::Off(60))
        ));
        assert!(keys.handle(Key::A, false, false, false, false).is_none());
    }

    #[test]
    fn shift_changes_pitch_but_not_the_key_identity() {
        let mut keys = Keys::default();
        assert!(matches!(
            keys.handle(Key::A, true, false, true, false),
            Some(SynthEvent::On { id: 60, pitch: 72 })
        ));
        assert!(matches!(
            keys.handle(Key::K, true, false, false, false),
            Some(SynthEvent::On { id: 72, pitch: 72 })
        ));
        assert_eq!(keys.held_count(), 2);
        assert!(matches!(
            keys.handle(Key::A, false, false, false, false),
            Some(SynthEvent::Off(60))
        ));
        assert_eq!(keys.held_count(), 1);
    }

    #[test]
    fn clearing_keys_allows_a_fresh_press_after_focus_returns() {
        let mut keys = Keys::default();
        keys.handle(Key::H, true, false, true, false);
        keys.clear();
        assert!(matches!(
            keys.handle(Key::H, true, false, false, false),
            Some(SynthEvent::On { id: 69, pitch: 69 })
        ));
        assert_eq!(key_note(Key::K), Some(72));
        assert_eq!(key_note(Key::Space), None);
    }

    #[test]
    fn extended_keys_and_octave_modifiers_keep_overlapping_pitches_independent() {
        let mut keys = Keys::default();
        assert_eq!(key_note(Key::O), Some(73));
        assert_eq!(key_note(Key::L), Some(74));
        assert_eq!(key_note(Key::P), Some(75));
        assert_eq!(key_note(Key::Semicolon), Some(76));
        assert_eq!(key_note(Key::Quote), Some(77));
        assert!(matches!(
            keys.handle(Key::L, true, false, false, true),
            Some(SynthEvent::On { id: 74, pitch: 62 })
        ));
        assert!(matches!(
            keys.handle(Key::S, true, false, false, false),
            Some(SynthEvent::On { id: 62, pitch: 62 })
        ));
        assert!(matches!(
            keys.handle(Key::L, false, false, false, false),
            Some(SynthEvent::Off(74))
        ));
        assert_eq!(keys.held_count(), 1);
        assert!(matches!(
            keys.handle(Key::Quote, true, false, true, true),
            Some(SynthEvent::On { id: 77, pitch: 77 })
        ));
    }
}
