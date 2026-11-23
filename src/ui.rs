use std::sync::mpsc::Sender;

use eframe::egui::{self, Event, Key};

use crate::{
    envelope::ADSR,
    keyboard::Keys,
    live_engine::{Instrument, LoopMonitor, LoopStatus, LoopTrack, SynthSettings},
    looper::{LoopSettings, LoopState},
    oscilloscope::Oscilloscope,
    synth::VoiceMode,
    synth_event::SynthEvent,
    waveform::Waveform,
};

struct NamedSynth {
    id: u64,
    name: String,
    settings: SynthSettings,
}

struct SynthApp {
    sender: Sender<SynthEvent>,
    keys: Keys,
    waveform: Waveform,
    voice_mode: VoiceMode,
    env: ADSR,
    focused: bool,
    scope: Oscilloscope,
    scope_samples: Vec<f32>,
    trigger_enabled: bool,
    loop_monitor: LoopMonitor,
    loop_status: LoopStatus,
    loop_settings: LoopSettings,
    click_enabled: bool,
    next_track_id: u64,
    synths: Vec<NamedSynth>,
    selected_synth: u64,
    next_synth_id: u64,
}

impl SynthApp {
    fn new(
        sender: Sender<SynthEvent>,
        waveform: Waveform,
        voice_mode: VoiceMode,
        scope: Oscilloscope,
        env: ADSR,
    ) -> Self {
        Self {
            sender,
            keys: Keys::default(),
            waveform,
            voice_mode,
            env,
            focused: true,
            scope_samples: vec![0.0; scope.sample_count()],
            scope,
            trigger_enabled: false,
            loop_monitor: LoopMonitor::default(),
            loop_status: LoopStatus::default(),
            loop_settings: LoopSettings::default(),
            click_enabled: true,
            next_track_id: 2,
            synths: vec![NamedSynth {
                id: 1,
                name: "Synth 1".into(),
                settings: SynthSettings {
                    waveform,
                    mode: voice_mode,
                    env,
                },
            }],
            selected_synth: 1,
            next_synth_id: 2,
        }
    }

    fn remember_synth_settings(&mut self) {
        let settings = SynthSettings {
            waveform: self.waveform,
            mode: self.voice_mode,
            env: self.env,
        };
        self.synths
            .iter_mut()
            .find(|synth| synth.id == self.selected_synth)
            .unwrap()
            .settings = settings;
    }

    fn select_synth(&mut self, id: u64) {
        if self.selected_synth == id {
            return;
        }
        let Some(settings) = self
            .synths
            .iter()
            .find(|synth| synth.id == id)
            .map(|synth| synth.settings)
        else {
            return;
        };
        self.keys.clear();
        self.selected_synth = id;
        self.waveform = settings.waveform;
        self.voice_mode = settings.mode;
        self.env = settings.env;
        let _ = self.sender.send(SynthEvent::SelectSynth(id));
    }

    fn add_synth(&mut self) {
        let id = self.next_synth_id;
        self.next_synth_id = id.checked_add(1).expect("Synth ID exhausted");
        let settings = SynthSettings {
            waveform: self.waveform,
            mode: self.voice_mode,
            env: self.env,
        };
        let instrument = Instrument::new(id, settings);
        self.synths.push(NamedSynth {
            id,
            name: format!("Synth {id}"),
            settings,
        });
        let _ = self.sender.send(SynthEvent::AddSynth(instrument));
        self.select_synth(id);
    }

    fn synth_controls(&mut self, ui: &mut egui::Ui) {
        let mut selected = self.selected_synth;
        let name = self
            .synths
            .iter()
            .find(|synth| synth.id == selected)
            .unwrap()
            .name
            .clone();
        ui.horizontal(|ui| {
            ui.label("Synth");
            let selector = egui::ComboBox::from_id_salt("synth_bank")
                .selected_text(name)
                .width(180.0)
                .show_ui(ui, |ui| {
                    for synth in &self.synths {
                        ui.selectable_value(&mut selected, synth.id, &synth.name);
                    }
                });
            if selected != self.selected_synth {
                selector.response.surrender_focus();
            }
            if ui.button("Add synth").clicked() {
                self.add_synth();
                selected = self.selected_synth;
            }
        });
        self.select_synth(selected);
        ui.horizontal(|ui| {
            ui.label("Name");
            let synth = self
                .synths
                .iter_mut()
                .find(|synth| synth.id == self.selected_synth)
                .unwrap();
            ui.text_edit_singleline(&mut synth.name);
        });
        ui.weak("Edit this synth’s sound. Tracks linked to it share these settings.");
    }

    fn set_voice_mode(&mut self, voice_mode: VoiceMode) {
        if self.voice_mode != voice_mode {
            self.keys.clear();
            self.voice_mode = voice_mode;
            self.remember_synth_settings();
            let _ = self.sender.send(SynthEvent::SetVoiceMode(voice_mode));
        }
    }

    fn set_waveform(&mut self, waveform: Waveform) {
        if self.waveform != waveform {
            self.waveform = waveform;
            self.remember_synth_settings();
            let _ = self.sender.send(SynthEvent::SetWaveform(waveform));
        }
    }

    fn set_envelope(&mut self, env: ADSR) {
        if self.env != env {
            self.env = env;
            self.remember_synth_settings();
            let _ = self.sender.send(SynthEvent::SetEnvelope(env));
        }
    }

    fn envelope_controls(&mut self, ui: &mut egui::Ui) {
        ui.strong("Envelope");
        let mut attack = self.env.attack_sec();
        let mut decay = self.env.decay_sec();
        let mut sustain = self.env.sustain_level();
        let mut release = self.env.release_sec();
        let mut changed = false;
        changed |= ui
            .add(
                egui::Slider::new(&mut attack, 0.0..=2.0)
                    .text("Attack")
                    .suffix(" s")
                    .max_decimals(3),
            )
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut decay, 0.0..=2.0)
                    .text("Decay")
                    .suffix(" s")
                    .max_decimals(3),
            )
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut sustain, 0.0..=1.0)
                    .text("Sustain")
                    .max_decimals(2),
            )
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut release, 0.0..=3.0)
                    .text("Release")
                    .suffix(" s")
                    .max_decimals(3),
            )
            .changed();
        ui.weak("Held notes keep their envelope. Legato keeps it for the connected phrase.");
        if changed {
            self.set_envelope(ADSR::new(attack, decay, sustain, release));
        }
    }

    fn play_loop(&mut self) {
        if self.loop_status.state == LoopState::Stopped && self.loop_settings.is_valid() {
            let _ = self.sender.send(SynthEvent::PlayLoop(self.loop_settings));
        }
    }

    fn arm_track(&mut self, id: u64) {
        if self.loop_status.state != LoopState::Stopped {
            if let Some(synth_id) = self
                .loop_monitor
                .read_tracks()
                .iter()
                .find(|track| track.id == id)
                .map(|track| track.synth_id)
            {
                self.select_synth(synth_id);
            }
            let _ = self.sender.send(SynthEvent::ArmTrack(id));
        }
    }

    fn add_track(&mut self) {
        let id = self.next_track_id;
        self.next_track_id = id.checked_add(1).expect("Track ID exhausted");
        // Allocate voices and recording storage here, outside the audio callback.
        let mut track = LoopTrack::new(
            id,
            self.waveform,
            self.voice_mode,
            self.env,
            self.loop_status.sample_rate,
        );
        track.synth_id = self.selected_synth;
        self.loop_monitor.register_track(id);
        self.loop_monitor.assign_synth(id, self.selected_synth);
        let _ = self.sender.send(SynthEvent::AddTrack(track));
    }

    fn track_controls(&mut self, ui: &mut egui::Ui) {
        let tracks = self.loop_monitor.read_tracks();
        egui::ScrollArea::vertical()
            .id_salt("tracks")
            .max_height(160.0)
            .show(ui, |ui| {
                for mut track in tracks {
                    ui.push_id(track.id, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().slider_width = 60.0;
                            ui.label(format!("Track {}", track.id));
                            let state = match track.state {
                                LoopState::Recording => "Recording",
                                _ if track.armed => "Armed",
                                LoopState::Playing => "Playing",
                                LoopState::Stopped => "Stopped",
                            };
                            ui.weak(state);
                            let previous_synth = track.synth_id;
                            let name = self
                                .synths
                                .iter()
                                .find(|synth| synth.id == track.synth_id)
                                .map(|synth| synth.name.as_str())
                                .unwrap_or("Synth 1");
                            egui::ComboBox::from_id_salt("track_synth")
                                .selected_text(name)
                                .width(95.0)
                                .show_ui(ui, |ui| {
                                    for synth in &self.synths {
                                        ui.selectable_value(
                                            &mut track.synth_id,
                                            synth.id,
                                            &synth.name,
                                        );
                                    }
                                });
                            if previous_synth != track.synth_id {
                                self.loop_monitor.assign_synth(track.id, track.synth_id);
                                let _ = self.sender.send(SynthEvent::TrackSynth {
                                    id: track.id,
                                    synth_id: track.synth_id,
                                });
                            }
                            let can_record = self.loop_status.state != LoopState::Stopped
                                && track.state != LoopState::Recording
                                && !track.armed;
                            if ui
                                .add_enabled(can_record, egui::Button::new("Record"))
                                .clicked()
                            {
                                self.arm_track(track.id);
                            }
                            if ui.checkbox(&mut track.muted, "Mute").changed() {
                                let _ = self.sender.send(SynthEvent::MuteTrack {
                                    id: track.id,
                                    muted: track.muted,
                                });
                            }
                            if ui
                                .add(
                                    egui::Slider::new(&mut track.volume, 0.0..=1.0)
                                        .show_value(false)
                                        .text("Volume"),
                                )
                                .changed()
                            {
                                let _ = self.sender.send(SynthEvent::TrackVolume {
                                    id: track.id,
                                    volume: track.volume,
                                });
                            }
                            if ui.button("Delete").clicked() {
                                self.loop_monitor.remove_track(track.id);
                                let _ = self.sender.send(SynthEvent::DeleteTrack(track.id));
                            }
                        });
                    });
                }
            });
        if ui.button("Add track").clicked() {
            self.add_track();
        }
    }

    fn loop_controls(&mut self, ui: &mut egui::Ui) {
        ui.strong("Loop");
        let stopped = self.loop_status.state == LoopState::Stopped;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(stopped, |ui| {
                ui.label("BPM");
                ui.add(
                    egui::DragValue::new(&mut self.loop_settings.bpm)
                        .range(30.0..=300.0)
                        .speed(0.1)
                        .max_decimals(1),
                );
                ui.label("Beats");
                ui.add(egui::DragValue::new(&mut self.loop_settings.beats).range(1..=64));
            });
            if ui.checkbox(&mut self.click_enabled, "Click").changed() {
                let _ = self.sender.send(SynthEvent::SetClick(self.click_enabled));
            }
        });
        ui.horizontal(|ui| {
            if ui.add_enabled(stopped, egui::Button::new("Play")).clicked() {
                self.play_loop();
            }
            if ui
                .add_enabled(!stopped, egui::Button::new("Stop"))
                .clicked()
            {
                let _ = self.sender.send(SynthEvent::StopLoop);
            }
            ui.weak("Each track records on the next loop boundary.");
        });
        let progress = self.loop_status.position as f32 / self.loop_status.length.max(1) as f32;
        let state = match self.loop_status.state {
            LoopState::Stopped => "Stopped",
            LoopState::Recording => "Recording",
            LoopState::Playing if self.loop_status.armed => "Armed: recording next loop",
            LoopState::Playing => "Playing",
        };
        let text = if stopped {
            format!(
                "{state} · {:.1} seconds",
                self.loop_settings.beats as f64 * 60.0 / self.loop_settings.bpm
            )
        } else {
            let beat = (progress * self.loop_status.settings.beats as f32) as u32 + 1;
            format!(
                "{state} · beat {beat} / {}",
                self.loop_status.settings.beats
            )
        };
        ui.add(egui::ProgressBar::new(progress).text(text));
        ui.weak("Play starts the click. Record replaces only that track’s take.");
        self.track_controls(ui);
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
                        if ui_has_keyboard || modifiers.alt || modifiers.mac_cmd {
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
                            Key::Num5 => Some(Waveform::WhiteNoise),
                            Key::Num6 => Some(Waveform::FilteredNoise),
                            _ => None,
                        };
                        if let Some(waveform) = waveform {
                            self.set_waveform(waveform);
                            continue;
                        }
                    }
                    if let Some(event) =
                        self.keys
                            .handle(key, *pressed, *repeat, modifiers.shift, modifiers.ctrl)
                    {
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
        self.loop_status = self.loop_monitor.read();
        let mut selected_waveform = self.waveform;
        let mut selected_voice_mode = self.voice_mode;
        let mut edited_synth = self.selected_synth;
        self.scope
            .snapshot(&mut self.scope_samples, self.trigger_enabled);
        ctx.request_repaint_after(std::time::Duration::from_millis(33));

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("main_controls").show(ui, |ui| {
            ui.add_space(12.0);
            ui.heading("RustSynth");
            ui.label("A small synthesizer, built in Rust.");
                self.synth_controls(ui);
                selected_waveform = self.waveform;
                selected_voice_mode = self.voice_mode;
                edited_synth = self.selected_synth;
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.label("Waveform");
                let selector = egui::ComboBox::from_id_salt("waveform")
                    .selected_text(selected_waveform.name())
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for waveform in [
                            Waveform::Sine,
                            Waveform::Triangle,
                            Waveform::Square,
                            Waveform::Sawtooth,
                            Waveform::WhiteNoise,
                            Waveform::FilteredNoise,
                        ] {
                            ui.selectable_value(&mut selected_waveform, waveform, waveform.name());
                        }
                    });
                if selected_waveform != self.waveform {
                    selector.response.surrender_focus();
                }
                ui.weak("1 / 2 / 3 / 4 / 5 / 6");
            });
            ui.horizontal(|ui| {
                ui.label("Voice mode");
                let selector = egui::ComboBox::from_id_salt("voice_mode")
                    .selected_text(selected_voice_mode.name())
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for mode in [VoiceMode::Polyphonic, VoiceMode::Monophonic, VoiceMode::Legato] {
                            ui.selectable_value(&mut selected_voice_mode, mode, mode.name());
                        }
                    });
                if selected_voice_mode != self.voice_mode {
                    selector.response.surrender_focus();
                }
                selector.response.on_hover_text("Polyphonic: chords. Monophonic: one note with a retriggered envelope. Legato: one note with a continuous envelope. Changes affect only this synth and its linked tracks. Changing mode releases their notes; the shared loop clock keeps running.");
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.weak("Live output · 25 ms · 4× display gain");
                ui.checkbox(&mut self.trigger_enabled, "Stabilize")
                    .on_hover_text("Align the display to the lowest sounding voice, including release tails. Chords may still move. This does not change the audio.");
            });
            waveform_preview(ui, &self.scope_samples);
            ui.add_space(12.0);
            self.envelope_controls(ui);
            ui.add_space(12.0);
            self.loop_controls(ui);
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(10.0);
            let play_area = egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.strong("Play with your keyboard");
                ui.monospace("Black keys    W  E     T  Y  U     O  P");
                ui.monospace("White keys   A  S  D  F  G  H  J  K  L  ;  '");
                ui.label("Shift: octave up. Control: octave down. Hold multiple keys for chords.");
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
        });

        if edited_synth == self.selected_synth {
            self.set_waveform(selected_waveform);
            self.set_voice_mode(selected_voice_mode);
        }
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
            let _ = self.sender.send(SynthEvent::Shutdown);
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
        let _ = self.sender.send(SynthEvent::Shutdown);
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

pub(crate) fn run(
    sender: Sender<SynthEvent>,
    waveform: Waveform,
    voice_mode: VoiceMode,
    scope: Oscilloscope,
    env: ADSR,
    loop_monitor: LoopMonitor,
) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([680.0, 820.0])
            .with_min_inner_size([620.0, 780.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RustSynth",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let mut app = SynthApp::new(sender, waveform, voice_mode, scope, env);
            app.loop_monitor = loop_monitor;
            Ok(Box::new(app))
        }),
    )
    .expect("Could not run synth UI");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn synth_bank_add_edit_and_track_assignment_work_through_the_ui() {
        let (sender, receiver) = mpsc::channel();
        let monitor = LoopMonitor::default();
        let mut engine = crate::live_engine::LiveEngine::new(
            Waveform::Sine,
            VoiceMode::Polyphonic,
            8_000,
            monitor.clone(),
        );
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
        app.loop_monitor = monitor;
        let ctx = egui::Context::default();
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_position(&output, "Add synth"));
        let event = receiver.try_recv().unwrap();
        assert!(matches!(&event, SynthEvent::AddSynth(_)));
        engine.handle(event);
        let event = receiver.try_recv().unwrap();
        assert!(matches!(event, SynthEvent::SelectSynth(2)));
        engine.handle(event);
        app.set_waveform(Waveform::WhiteNoise);
        engine.handle(receiver.try_recv().unwrap());
        app.synths[1].name = "Noise".into();
        let output = draw(&mut app, &ctx, vec![]);
        let routing = row_text_position(&output, "Track 1", "Synth 1");
        click(&mut app, &ctx, routing);
        let output = draw(&mut app, &ctx, vec![]);
        let choice = output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    if text.galley.job.text == "Noise" {
                        return Some(text.pos + text.galley.size() * 0.5);
                    }
                }
                None
            })
            .expect("Routing should offer the new synth");
        click(&mut app, &ctx, choice);
        let event = receiver.try_recv().unwrap();
        assert!(matches!(
            event,
            SynthEvent::TrackSynth { id: 1, synth_id: 2 }
        ));
        engine.handle(event);
        engine.publish_status();
        assert_eq!(app.loop_monitor.read_tracks()[0].synth_id, 2);
        app.select_synth(1);
        assert_eq!(app.waveform, Waveform::Sine);
        engine.handle(receiver.try_recv().unwrap());
        app.select_synth(2);
        assert_eq!(app.waveform, Waveform::WhiteNoise);
        engine.handle(receiver.try_recv().unwrap());
        assert_eq!(app.synths[1].name, "Noise");
    }

    fn row_text_position(output: &egui::FullOutput, row: &str, label: &str) -> egui::Pos2 {
        let row_y = text_position(output, row).y;
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    let center = text.pos + text.galley.size() * 0.5;
                    if text.galley.job.text == label && (center.y - row_y).abs() < 3.0 {
                        return Some(center);
                    }
                }
                None
            })
            .unwrap_or_else(|| panic!("Missing {label} in {row}"))
    }

    #[test]
    fn add_record_mute_and_delete_controls_use_stable_track_ids() {
        let (sender, receiver) = mpsc::channel();
        let monitor = LoopMonitor::default();
        let mut engine = crate::live_engine::LiveEngine::new(
            Waveform::Sine,
            VoiceMode::Polyphonic,
            8_000,
            monitor.clone(),
        );
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
        app.loop_monitor = monitor;
        let ctx = egui::Context::default();
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_position(&output, "Add track"));
        let event = receiver.try_recv().unwrap();
        assert!(matches!(&event, SynthEvent::AddTrack(_)));
        engine.handle(event);
        engine.publish_status();
        let output = draw(&mut app, &ctx, vec![]);
        text_position(&output, "Track 2");
        click(&mut app, &ctx, text_position(&output, "Play"));
        engine.handle(receiver.try_recv().unwrap());
        engine.publish_status();
        let output = draw(&mut app, &ctx, vec![]);
        click(
            &mut app,
            &ctx,
            row_text_position(&output, "Track 2", "Record"),
        );
        let event = receiver.try_recv().unwrap();
        assert!(matches!(event, SynthEvent::ArmTrack(2)));
        engine.handle(event);
        engine.publish_status();
        let output = draw(&mut app, &ctx, vec![]);
        click(
            &mut app,
            &ctx,
            row_text_position(&output, "Track 2", "Mute"),
        );
        let event = receiver.try_recv().unwrap();
        assert!(matches!(
            event,
            SynthEvent::MuteTrack { id: 2, muted: true }
        ));
        engine.handle(event);
        engine.publish_status();
        let output = draw(&mut app, &ctx, vec![]);
        click(
            &mut app,
            &ctx,
            row_text_position(&output, "Track 1", "Delete"),
        );
        let event = receiver.try_recv().unwrap();
        assert!(matches!(event, SynthEvent::DeleteTrack(1)));
        engine.handle(event);
        engine.publish_status();
        let tracks = app.loop_monitor.read_tracks();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, 2);
        assert!(tracks[0].armed && tracks[0].muted);
        let output = draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_position(&output, "Add track"));
        engine.handle(receiver.try_recv().unwrap());
        engine.publish_status();
        let output = draw(&mut app, &ctx, vec![]);
        text_position(&output, "Track 3");
    }

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
    fn control_octave_down_reaches_the_audio_channel() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
        let mut event = key(Key::L, true);
        if let Event::Key { modifiers, .. } = &mut event {
            modifiers.ctrl = true;
        }
        app.handle_input(&[event], true, false);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::On { id: 74, pitch: 62 })
        ));
        app.handle_input(&[key(Key::L, false)], true, false);
        assert!(matches!(receiver.try_recv(), Ok(SynthEvent::Off(74))));
    }

    #[test]
    fn loop_controls_send_commands_and_display_audio_status() {
        let (sender, receiver) = mpsc::channel();
        let monitor = LoopMonitor::default();
        let mut engine = crate::live_engine::LiveEngine::new(
            Waveform::Sine,
            VoiceMode::Polyphonic,
            8_000,
            monitor.clone(),
        );
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
        app.loop_monitor = monitor;
        let ctx = egui::Context::default();
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_position(&output, "Click"));
        let event = receiver.try_recv().unwrap();
        assert!(matches!(&event, SynthEvent::SetClick(false)));
        engine.handle(event);
        let output = draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_position(&output, "Play"));
        let event = receiver.try_recv().unwrap();
        assert!(matches!(&event, SynthEvent::PlayLoop(_)));
        engine.handle(event);
        engine.publish_status();
        let output = draw(&mut app, &ctx, vec![]);
        assert_eq!(app.loop_status.state, LoopState::Playing);
        click(&mut app, &ctx, text_position(&output, "Record"));
        let event = receiver.try_recv().unwrap();
        assert!(matches!(&event, SynthEvent::ArmTrack(1)));
        engine.handle(event);
        engine.publish_status();
        draw(&mut app, &ctx, vec![]);
        assert!(app.loop_status.armed);
        for _ in 0..32_000 {
            engine.next_sample();
        }
        engine.publish_status();
        let output = draw(&mut app, &ctx, vec![]);
        assert_eq!(app.loop_status.state, LoopState::Recording);
        click(&mut app, &ctx, text_position(&output, "Stop"));
        let event = receiver.try_recv().unwrap();
        assert!(matches!(&event, SynthEvent::StopLoop));
        engine.handle(event);
        engine.publish_status();
        draw(&mut app, &ctx, vec![]);
        assert_eq!(app.loop_status.state, LoopState::Stopped);
    }

    #[test]
    fn waveform_changes_send_one_message_and_shortcuts_update_selection() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(
            sender,
            Waveform::Triangle,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
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
        app.handle_input(&[key(Key::Num5, true)], true, false);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::SetWaveform(Waveform::WhiteNoise))
        ));
        app.handle_input(&[key(Key::Num6, true)], true, false);
        assert_eq!(app.waveform, Waveform::FilteredNoise);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::SetWaveform(Waveform::FilteredNoise))
        ));
    }

    #[test]
    fn ui_navigation_blocks_note_presses_but_allows_existing_releases() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
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
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
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
                    egui::vec2(680.0, 820.0),
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
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
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
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            scope.clone(),
            ADSR::default(),
        );
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
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Polyphonic,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
        let ctx = egui::Context::default();
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        let toggle = text_position(&output, "Stabilize");
        click(&mut app, &ctx, toggle);
        assert!(app.trigger_enabled);
        click(&mut app, &ctx, toggle);
        assert!(!app.trigger_enabled);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn envelope_controls_start_from_the_supplied_setting_and_send_slider_changes() {
        for label in ["Attack", "Decay", "Sustain", "Release"] {
            let (sender, receiver) = mpsc::channel();
            let initial = ADSR::new(0.15, 0.25, 0.5, 0.75);
            let mut app = SynthApp::new(
                sender,
                Waveform::Sine,
                VoiceMode::Polyphonic,
                Oscilloscope::new(48_000),
                initial,
            );
            let ctx = egui::Context::default();
            draw(&mut app, &ctx, vec![]);
            let output = draw(&mut app, &ctx, vec![]);
            assert_eq!(app.env, initial);
            assert!(receiver.try_recv().is_err());
            let label_pos = text_position(&output, label);
            // Sliders start at the panel's left edge, before their value and label.
            let slider_pos = egui::pos2(ctx.style().spacing.slider_width * 0.75, label_pos.y);
            click(&mut app, &ctx, slider_pos);
            let SynthEvent::SetEnvelope(env) =
                receiver.try_recv().expect("Slider should send its setting")
            else {
                panic!("Expected envelope update");
            };
            assert_eq!(env, app.env);
            match label {
                "Attack" => assert_ne!(env.attack_sec(), initial.attack_sec()),
                "Decay" => assert_ne!(env.decay_sec(), initial.decay_sec()),
                "Sustain" => assert_ne!(env.sustain_level(), initial.sustain_level()),
                "Release" => assert_ne!(env.release_sec(), initial.release_sec()),
                _ => unreachable!(),
            }
            assert!((0.0..=1.0).contains(&env.sustain_level()));
        }
    }

    #[test]
    fn voice_mode_selector_starts_from_the_supplied_mode_and_resets_held_keys() {
        let (sender, receiver) = mpsc::channel();
        let mut app = SynthApp::new(
            sender,
            Waveform::Sine,
            VoiceMode::Legato,
            Oscilloscope::new(48_000),
            ADSR::default(),
        );
        app.set_voice_mode(VoiceMode::Legato);
        assert!(receiver.try_recv().is_err());
        app.handle_input(&[key(Key::A, true)], true, false);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::On { id: 60, .. })
        ));
        let ctx = egui::Context::default();
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_position(&output, "Legato"));
        let output = draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, text_position(&output, "Monophonic"));
        assert_eq!(app.voice_mode, VoiceMode::Monophonic);
        assert_eq!(app.keys.held_count(), 0);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::SetVoiceMode(VoiceMode::Monophonic))
        ));
        let output = draw(&mut app, &ctx, vec![]);
        text_position(&output, "Monophonic");
        // A release from before the mode change is ignored; a fresh press works.
        app.handle_input(&[key(Key::A, false)], true, false);
        assert!(receiver.try_recv().is_err());
        app.handle_input(&[key(Key::A, true)], true, false);
        assert!(matches!(
            receiver.try_recv(),
            Ok(SynthEvent::On { id: 60, .. })
        ));
    }
}
