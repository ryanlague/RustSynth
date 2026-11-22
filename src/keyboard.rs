use eframe::egui::Key;

use crate::synth_event::SynthEvent;

#[derive(Default)]
pub(crate) struct Keys {
    held: [bool; 13],
}

impl Keys {
    pub(crate) fn handle(
        &mut self,
        key: Key,
        pressed: bool,
        repeat: bool,
        shift: bool,
    ) -> Option<SynthEvent> {
        let id = key_note(key)?;
        let held = &mut self.held[usize::from(id - 60)];
        if pressed && !repeat && !*held {
            *held = true;
            let pitch = if shift { id + 12 } else { id };
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
            keys.handle(Key::A, true, false, false),
            Some(SynthEvent::On { id: 60, pitch: 60 })
        ));
        assert!(keys.handle(Key::A, true, true, false).is_none());
        assert!(keys.handle(Key::A, true, false, false).is_none());
        assert!(matches!(
            keys.handle(Key::A, false, false, false),
            Some(SynthEvent::Off(60))
        ));
        assert!(keys.handle(Key::A, false, false, false).is_none());
    }

    #[test]
    fn shift_changes_pitch_but_not_the_key_identity() {
        let mut keys = Keys::default();
        assert!(matches!(
            keys.handle(Key::A, true, false, true),
            Some(SynthEvent::On { id: 60, pitch: 72 })
        ));
        assert!(matches!(
            keys.handle(Key::K, true, false, false),
            Some(SynthEvent::On { id: 72, pitch: 72 })
        ));
        assert_eq!(keys.held_count(), 2);
        assert!(matches!(
            keys.handle(Key::A, false, false, false),
            Some(SynthEvent::Off(60))
        ));
        assert_eq!(keys.held_count(), 1);
    }

    #[test]
    fn clearing_keys_allows_a_fresh_press_after_focus_returns() {
        let mut keys = Keys::default();
        keys.handle(Key::H, true, false, true);
        keys.clear();
        assert!(matches!(
            keys.handle(Key::H, true, false, false),
            Some(SynthEvent::On { id: 69, pitch: 69 })
        ));
        assert_eq!(key_note(Key::K), Some(72));
        assert_eq!(key_note(Key::Space), None);
    }
}
