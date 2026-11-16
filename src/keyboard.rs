use std::sync::mpsc::Sender;

use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

pub(crate) enum NoteEvent {
    On(u8),
    Off(u8),
    AllOff,
}

#[derive(Default)]
struct Keys {
    held: [bool; 13],
}

impl Keys {
    fn handle(&mut self, code: KeyCode, state: ElementState, repeat: bool) -> Option<NoteEvent> {
        let id = key_note(code)?;
        let held = &mut self.held[usize::from(id - 60)];
        match state {
            ElementState::Pressed if !repeat && !*held => {
                *held = true;
                Some(NoteEvent::On(id))
            }
            ElementState::Released if *held => {
                *held = false;
                Some(NoteEvent::Off(id))
            }
            _ => None,
        }
    }

    fn clear(&mut self) {
        self.held.fill(false);
    }
}

fn key_note(code: KeyCode) -> Option<u8> {
    Some(match code {
        KeyCode::KeyA => 60,
        KeyCode::KeyW => 61,
        KeyCode::KeyS => 62,
        KeyCode::KeyE => 63,
        KeyCode::KeyD => 64,
        KeyCode::KeyF => 65,
        KeyCode::KeyT => 66,
        KeyCode::KeyG => 67,
        KeyCode::KeyY => 68,
        KeyCode::KeyH => 69,
        KeyCode::KeyU => 70,
        KeyCode::KeyJ => 71,
        KeyCode::KeyK => 72,
        _ => return None,
    })
}

struct KeyboardApp {
    window: Option<Window>,
    sender: Sender<NoteEvent>,
    keys: Keys,
}

impl KeyboardApp {
    fn release_all(&mut self) {
        self.keys.clear();
        let _ = self.sender.send(NoteEvent::AllOff);
    }
}

impl ApplicationHandler for KeyboardApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.window = Some(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("RustSynth | A W S E D F T G Y H U J K | Esc to exit")
                            .with_inner_size(LogicalSize::new(720.0, 240.0)),
                    )
                    .expect("Could not create keyboard window"),
            );
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.release_all();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.release_all();
                event_loop.exit();
            }
            WindowEvent::Focused(false) => self.release_all(),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if code == KeyCode::Escape && event.state == ElementState::Pressed {
                        self.release_all();
                        event_loop.exit();
                        return;
                    }
                    if let Some(message) = self.keys.handle(code, event.state, event.repeat) {
                        let _ = self.sender.send(message);
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn run(sender: Sender<NoteEvent>) {
    println!(
        "Focus the RustSynth window to play.\nWhite keys: A S D F G H J K (C4 to C5)\nBlack keys: W E T Y U\nRelease a key to release its note. Escape closes the window."
    );
    let event_loop = EventLoop::new().expect("Could not create keyboard event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = KeyboardApp {
        window: None,
        sender,
        keys: Keys::default(),
    };
    event_loop
        .run_app(&mut app)
        .expect("Keyboard event loop failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_repeat_and_unmatched_releases_are_ignored() {
        let mut keys = Keys::default();
        assert!(matches!(
            keys.handle(KeyCode::KeyA, ElementState::Pressed, false),
            Some(NoteEvent::On(60))
        ));
        assert!(
            keys.handle(KeyCode::KeyA, ElementState::Pressed, true)
                .is_none()
        );
        assert!(
            keys.handle(KeyCode::KeyA, ElementState::Pressed, false)
                .is_none()
        );
        assert!(matches!(
            keys.handle(KeyCode::KeyA, ElementState::Released, false),
            Some(NoteEvent::Off(60))
        ));
        assert!(
            keys.handle(KeyCode::KeyA, ElementState::Released, false)
                .is_none()
        );
    }

    #[test]
    fn clearing_keys_allows_a_fresh_press_after_focus_returns() {
        let mut keys = Keys::default();
        keys.handle(KeyCode::KeyH, ElementState::Pressed, false);
        keys.clear();
        assert!(matches!(
            keys.handle(KeyCode::KeyH, ElementState::Pressed, false),
            Some(NoteEvent::On(69))
        ));
        assert_eq!(key_note(KeyCode::KeyK), Some(72));
        assert_eq!(key_note(KeyCode::Space), None);
    }
}
