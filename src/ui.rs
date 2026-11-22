use std::sync::mpsc::Sender;

use eframe::egui::{self, Event, Key};

use crate::{
    keyboard::Keys, oscilloscope::Oscilloscope, synth_event::SynthEvent, waveform::Waveform,
};

struct SynthApp {
    sender: Sender<SynthEvent>,
    keys: Keys,
    waveform: Waveform,
    focused: bool,
    scope: Oscilloscope,
    scope_samples: Vec<f32>,
    trigger_enabled: bool,
}

impl SynthApp {
    fn new(sender: Sender<SynthEvent>, waveform: Waveform, scope: Oscilloscope) -> Self {
        Self {
            sender,
            keys: Keys::default(),
            waveform,
            focused: true,
            scope_samples: vec![0.0; scope.sample_count()],
            scope,
            trigger_enabled: false,
        }
    }

    fn set_waveform(&mut self, waveform: Waveform) {
        if self.waveform != waveform {
            self.waveform = waveform;
            let _ = self.sender.send(SynthEvent::SetWaveform(waveform));
        }
    }

    fn release_all(&mut self) {
        self.keys.clear();
        let _ = self.sender.send(SynthEvent::AllOff);
    }

    // Releases are handled even while a UI control owns keyboard focus.
    // Otherwise opening the selector while holding a note could leave it stuck.
    fn handle_input(&mut self, events: &[Event], focused: bool, ui_has_keyboard: bool) -> bool {
        if !focused {
            if self.focused || self.keys.held_count() > 0 {
                self.release_all();
            }
            self.focused = false;
            return false;
        }
        self.focused = true;

        for event in events {
            match event {
                Event::WindowFocused(false) => self.release_all(),
                Event::Key {
                    key,
                    physical_key,
                    pressed,
                    repeat,
                    modifiers,
                } => {
                    let key = physical_key.unwrap_or(*key);
                    if *pressed {
                        if ui_has_keyboard || modifiers.ctrl || modifiers.alt || modifiers.command {
                            continue;
                        }
                        if *repeat {
                            continue;
                        }
                        if key == Key::Escape {
                            self.release_all();
                            return true;
                        }
                        let waveform = match key {
                            Key::Num1 => Some(Waveform::Sine),
                            Key::Num2 => Some(Waveform::Triangle),
                            Key::Num3 => Some(Waveform::Square),
                            Key::Num4 => Some(Waveform::Sawtooth),
                            _ => None,
                        };
                        if let Some(waveform) = waveform {
                            self.set_waveform(waveform);
                            continue;
                        }
                    }
                    if let Some(event) = self.keys.handle(key, *pressed, *repeat, modifiers.shift) {
                        let _ = self.sender.send(event);
                    }
                }
                _ => {}
            }
        }
        false
    }
}

impl SynthApp {
    fn update_ui(&mut self, ctx: &egui::Context) {
        // Save the original events before widgets consume navigation keys.
        let (events, focused, closing) = ctx.input(|input| {
            (
                input.events.clone(),
                input.focused,
                input.viewport().close_requested(),
            )
        });
        let captured_before = ctx.wants_keyboard_input() || egui::Popup::is_any_open(ctx);
        let mut selected = self.waveform;
        self.scope
            .snapshot(&mut self.scope_samples, self.trigger_enabled);
        ctx.request_repaint_after(std::time::Duration::from_millis(33));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("RustSynth");
            ui.label("A small synthesizer, built in Rust.");
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.label("Waveform");
                let selector = egui::ComboBox::from_id_salt("waveform")
                    .selected_text(selected.name())
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for waveform in [
                            Waveform::Sine,
                            Waveform::Triangle,
                            Waveform::Square,
                            Waveform::Sawtooth,
                        ] {
                            ui.selectable_value(&mut selected, waveform, waveform.name());
                        }
                    });
                if selected != self.waveform {
                    selector.response.surrender_focus();
                }
                ui.weak("1 / 2 / 3 / 4");
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.weak("Live output · 25 ms · 4× display gain");
                ui.checkbox(&mut self.trigger_enabled, "Lock to lowest note")
                    .on_hover_text("Align the display to the lowest sounding voice, including release tails. Chords may still move. This does not change the audio.");
            });
            waveform_preview(ui, &self.scope_samples);
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(10.0);
            let play_area = egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.strong("Play with your keyboard");
                ui.monospace("Black keys    W  E     T  Y  U");
                ui.monospace("White keys   A  S  D  F  G  H  J  K");
                ui.label("Hold Shift for one octave up. Hold multiple keys for chords.");
                ui.label("Click here to return to playing after keyboard navigation.");
                ui.weak(format!(
                    "{} keys held · Escape to exit",
                    self.keys.held_count()
                ));
            });
            let response = ui.interact(
                play_area.response.rect,
                ui.id().with("play_area"),
                egui::Sense::click(),
            );
            if response.clicked() {
                ctx.memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.surrender_focus(id);
                    }
                });
            }
        });

        self.set_waveform(selected);
        let captured =
            captured_before || ctx.wants_keyboard_input() || egui::Popup::is_any_open(ctx);
        let previous = self.waveform;
        let held_before = self.keys.held_count();
        let close = self.handle_input(&events, focused, captured);
        if previous != self.waveform || held_before != self.keys.held_count() {
            ctx.request_repaint();
        }
        if closing || close {
            self.release_all();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.update_ui(ctx);
    }

    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.release_all();
    }
}

fn waveform_preview(ui: &mut egui::Ui, samples: &[f32]) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 76.0), egui::Sense::hover());
    let color = ui.visuals().selection.bg_fill;
    // Fixed gain preserves visible attack/release and relative loudness.
    // Only the display is clipped; playback samples are untouched.
    let points = samples
        .iter()
        .enumerate()
        .map(|(index, &sample)| {
            let progress = index as f32 / (samples.len() - 1) as f32;
            egui::pos2(
                rect.left() + progress * rect.width(),
                rect.center().y - (sample * 4.0).clamp(-1.0, 1.0) * (rect.height() * 0.45),
            )
        })
        .collect();
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    ui.painter()
        .add(egui::Shape::line(points, egui::Stroke::new(2.0_f32, color)));
}

pub(crate) fn run(sender: Sender<SynthEvent>, waveform: Waveform, scope: Oscilloscope) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([620.0, 400.0])
            .with_min_inner_size([540.0, 380.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RustSynth",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(SynthApp::new(sender, waveform, scope)))
        }),
    )
    .expect("Could not run synth UI");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn key(key: Key, pressed: bool) -> Event {
        Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }
    }

    #[test]
    fn waveform_changes_send_one_message_and_shortcuts_update_selection() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(sender, Waveform::Triangle, Oscilloscope::new(48_000));
        assert_eq!(app.waveform, Waveform::Triangle);
        app.set_waveform(Waveform::Triangle);
        assert!(receiver.try_recv().is_err());
        app.set_waveform(Waveform::Square);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::SetWaveform(Waveform::Square))
        ));
        app.handle_input(&[key(Key::Num4, true)], true, false);
        assert_eq!(app.waveform, Waveform::Sawtooth);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::SetWaveform(Waveform::Sawtooth))
        ));
    }

    #[test]
    fn ui_navigation_blocks_note_presses_but_allows_existing_releases() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(sender, Waveform::Sine, Oscilloscope::new(48_000));
        app.handle_input(&[key(Key::A, true)], true, false);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::On { id: 60, .. })
        ));
        app.handle_input(
            &[key(Key::S, true), key(Key::Num4, true), key(Key::A, false)],
            true,
            true,
        );
        assert!(matches!(receiver.try_recv(), Ok(SynthEvent::Off(60))));
        assert!(receiver.try_recv().is_err());
        assert_eq!(app.waveform, Waveform::Sine);
        assert_eq!(app.keys.held_count(), 0);
    }

    #[test]
    fn focus_loss_and_exit_release_notes() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(sender, Waveform::Sine, Oscilloscope::new(48_000));
        app.handle_input(&[key(Key::A, true)], true, false);
        receiver.try_recv().unwrap();
        app.handle_input(&[], false, false);
        assert!(matches!(receiver.try_recv(), Ok(SynthEvent::AllOff)));
        assert_eq!(app.keys.held_count(), 0);
        assert!(app.handle_input(&[key(Key::Escape, true)], true, false));
        assert!(matches!(receiver.try_recv(), Ok(SynthEvent::AllOff)));
    }

    fn draw(app: &mut SynthApp, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(620.0, 400.0),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ctx| app.update_ui(ctx),
        )
    }

    fn text_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|clipped| {
                if let egui::Shape::Text(text) = &clipped.shape {
                    if text.galley.job.text == label {
                        return Some(text.pos + text.galley.size() * 0.5);
                    }
                }
                None
            })
            .unwrap_or_else(|| panic!("Missing visible label: {label}"))
    }

    fn click(app: &mut SynthApp, ctx: &egui::Context, pos: egui::Pos2) -> egui::FullOutput {
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        draw(app, ctx, vec![Event::PointerMoved(pos), button(true)]);
        draw(app, ctx, vec![Event::PointerMoved(pos), button(false)])
    }

    #[test]
    fn clicking_the_dropdown_selects_a_waveform_and_updates_the_label() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(sender, Waveform::Sine, Oscilloscope::new(48_000));
        let ctx = egui::Context::default();
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        let selector = text_position(&output, "Sine");
        click(&mut app, &ctx, selector);
        let output = draw(&mut app, &ctx, vec![]);
        let square = text_position(&output, "Square");
        click(&mut app, &ctx, square);
        assert_eq!(app.waveform, Waveform::Square);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::SetWaveform(Waveform::Square))
        ));
        let output = draw(&mut app, &ctx, vec![]);
        text_position(&output, "Square");
        assert!(!egui::Popup::is_any_open(&ctx));
        draw(&mut app, &ctx, vec![key(Key::A, true)]);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::On { id: 60, pitch: 60 })
        ));
    }

    #[test]
    fn live_trace_uses_captured_output_and_returns_to_a_flat_line() {
        let (sender, _) = mpsc::channel();
        let scope = Oscilloscope::new(160);
        let mut app = SynthApp::new(sender, Waveform::Sine, scope.clone());
        let ctx = egui::Context::default();
        scope.capture(&[0.1, -0.1, 0.05, -0.05], 1);
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        assert_eq!(app.scope_samples, [0.1, -0.1, 0.05, -0.05]);
        let trace = |output: &egui::FullOutput| {
            output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Path(path) = &shape.shape {
                        if !path.closed && path.points.len() == 4 {
                            return Some(path.points.clone());
                        }
                    }
                    None
                })
                .expect("Live trace should be drawn")
        };
        let points = trace(&output);
        assert!(points[0].y < points[1].y);
        assert!(points[0].x < points[1].x);
        scope.capture(&[0.0; 4], 1);
        let output = draw(&mut app, &ctx, vec![]);
        let points = trace(&output);
        assert!(points.iter().all(|point| point.y == points[0].y));
    }

    #[test]
    fn trigger_toggle_switches_display_mode_without_sending_audio_commands() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(sender, Waveform::Sine, Oscilloscope::new(48_000));
        let ctx = egui::Context::default();
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        let toggle = text_position(&output, "Lock to lowest note");
        click(&mut app, &ctx, toggle);
        assert!(app.trigger_enabled);
        click(&mut app, &ctx, toggle);
        assert!(!app.trigger_enabled);
        assert!(receiver.try_recv().is_err());
    }
}
