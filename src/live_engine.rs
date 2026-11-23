use std::sync::{Arc, Mutex};

use crate::{
    envelope::ADSR,
    looper::{LoopNoteEvent, LoopSettings, LoopState, Looper},
    metronome::Metronome,
    synth::{Synth, VoiceMode},
    synth_event::SynthEvent,
    transport::Transport,
    waveform::Waveform,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SynthSettings {
    pub(crate) waveform: Waveform,
    pub(crate) mode: VoiceMode,
    pub(crate) env: ADSR,
}

/// A reusable instrument definition plus its live-keyboard voice pool.
/// Track voice pools stay independent, while sharing this definition's settings.
pub(crate) struct Instrument {
    id: u64,
    settings: SynthSettings,
    keyboard: Option<Synth>,
    next: Option<Box<Instrument>>,
}

impl Instrument {
    pub(crate) fn new(id: u64, settings: SynthSettings) -> Box<Self> {
        let mut keyboard = Synth::new(8, settings.waveform, settings.mode);
        keyboard.set_envelope(settings.env);
        Box::new(Self {
            id,
            settings,
            keyboard: Some(keyboard),
            next: None,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LoopStatus {
    pub(crate) state: LoopState,
    pub(crate) settings: LoopSettings,
    pub(crate) position: usize,
    pub(crate) length: usize,
    pub(crate) armed: bool,
    pub(crate) sample_rate: u32,
}

impl Default for LoopStatus {
    fn default() -> Self {
        Self {
            state: LoopState::Stopped,
            settings: LoopSettings::default(),
            position: 0,
            length: 1,
            armed: false,
            sample_rate: 48_000,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TrackStatus {
    pub(crate) id: u64,
    pub(crate) synth_id: u64,
    pub(crate) state: LoopState,
    pub(crate) armed: bool,
    pub(crate) muted: bool,
    pub(crate) volume: f32,
}

impl TrackStatus {
    fn new(id: u64) -> Self {
        Self {
            id,
            synth_id: 1,
            state: LoopState::Stopped,
            armed: false,
            muted: false,
            volume: 1.0,
        }
    }
}

#[derive(Clone)]
pub(crate) struct LoopMonitor {
    status: Arc<Mutex<LoopStatus>>,
    tracks: Arc<Mutex<Vec<TrackStatus>>>,
    retired: Arc<Mutex<Option<Box<LoopTrack>>>>,
}

impl Default for LoopMonitor {
    fn default() -> Self {
        Self {
            status: Arc::new(Mutex::new(LoopStatus::default())),
            tracks: Arc::new(Mutex::new(vec![TrackStatus::new(1)])),
            retired: Arc::new(Mutex::new(None)),
        }
    }
}

impl LoopMonitor {
    pub(crate) fn read(&self) -> LoopStatus {
        // Destruction of deleted tracks happens on the UI thread, never in audio.
        let mut retired = self.retired.lock().unwrap().take();
        while let Some(mut node) = retired {
            retired = node.next.take();
        }
        *self.status.lock().unwrap()
    }
    pub(crate) fn read_tracks(&self) -> Vec<TrackStatus> {
        self.tracks.lock().unwrap().clone()
    }
    pub(crate) fn register_track(&self, id: u64) {
        self.tracks.lock().unwrap().push(TrackStatus::new(id));
    }
    pub(crate) fn assign_synth(&self, id: u64, synth_id: u64) {
        if let Some(track) = self
            .tracks
            .lock()
            .unwrap()
            .iter_mut()
            .find(|track| track.id == id)
        {
            track.synth_id = synth_id;
        }
    }

    pub(crate) fn remove_track(&self, id: u64) {
        self.tracks.lock().unwrap().retain(|track| track.id != id);
    }
}

/// Prepared on the UI thread, then transferred into an intrusive list without
/// allocating or resizing a collection in the audio callback.
pub(crate) struct LoopTrack {
    id: u64,
    pub(crate) synth_id: u64,
    playback: Synth,
    looper: Looper,
    muted: bool,
    volume: f32,
    next: Option<Box<LoopTrack>>,
}

impl LoopTrack {
    pub(crate) fn new(
        id: u64,
        waveform: Waveform,
        mode: VoiceMode,
        env: ADSR,
        sample_rate: u32,
    ) -> Box<Self> {
        let mut playback = Synth::new(8, waveform, mode);
        playback.set_envelope(env);
        Box::new(Self {
            id,
            synth_id: 1,
            playback,
            looper: Looper::new(LoopSettings::default(), sample_rate),
            muted: false,
            volume: 1.0,
            next: None,
        })
    }
}

fn detach(list: &mut Option<Box<LoopTrack>>, id: u64) -> Option<Box<LoopTrack>> {
    let mut link = list;
    while link.as_ref().is_some_and(|track| track.id != id) {
        link = &mut link.as_mut().unwrap().next;
    }
    let mut track = link.take()?;
    *link = track.next.take();
    Some(track)
}

pub(crate) struct LiveEngine {
    keyboard: Synth,
    instruments: Option<Box<Instrument>>,
    selected_synth: u64,
    tracks: Option<Box<LoopTrack>>,
    retiring: Option<Box<LoopTrack>>,
    garbage: Option<Box<LoopTrack>>,
    transport: Transport,
    metronome: Metronome,
    keys_held: [Option<u8>; 256],
    sample_rate: u32,
    monitor: LoopMonitor,
    waveform: Waveform,
    mode: VoiceMode,
}

impl LiveEngine {
    pub(crate) fn new(
        waveform: Waveform,
        mode: VoiceMode,
        sample_rate: u32,
        monitor: LoopMonitor,
    ) -> Self {
        let mut first = Instrument::new(
            1,
            SynthSettings {
                waveform,
                mode,
                env: ADSR::default(),
            },
        );
        let keyboard = first.keyboard.take().unwrap();
        let mut engine = Self {
            keyboard,
            instruments: Some(first),
            selected_synth: 1,
            tracks: Some(LoopTrack::new(
                1,
                waveform,
                mode,
                ADSR::default(),
                sample_rate,
            )),
            retiring: None,
            garbage: None,
            transport: Transport::new(sample_rate),
            metronome: Metronome::new(sample_rate),
            keys_held: [None; 256],
            sample_rate,
            monitor,
            waveform,
            mode,
        };
        engine.publish_status();
        engine
    }
    pub(crate) fn envelope(&self) -> ADSR {
        self.keyboard.envelope()
    }
    fn instrument(&self, id: u64) -> Option<&Instrument> {
        std::iter::successors(self.instruments.as_deref(), |node| node.next.as_deref())
            .find(|node| node.id == id)
    }
    fn instrument_mut(&mut self, id: u64) -> Option<&mut Instrument> {
        let mut cursor = self.instruments.as_deref_mut();
        while let Some(node) = cursor {
            if node.id == id {
                return Some(node);
            }
            cursor = node.next.as_deref_mut();
        }
        None
    }
    fn select_synth(&mut self, id: u64) {
        if id == self.selected_synth || self.instrument(id).is_none() {
            return;
        }
        self.release_keyboard();
        let next = self.instrument_mut(id).unwrap().keyboard.take().unwrap();
        let old = std::mem::replace(&mut self.keyboard, next);
        let old_id = self.selected_synth;
        self.instrument_mut(old_id).unwrap().keyboard = Some(old);
        self.selected_synth = id;
        let settings = self.instrument(id).unwrap().settings;
        self.waveform = settings.waveform;
        self.mode = settings.mode;
    }

    fn each_track(&mut self, mut operation: impl FnMut(&mut LoopTrack)) {
        let mut cursor = self.tracks.as_deref_mut();
        while let Some(track) = cursor {
            operation(track);
            cursor = track.next.as_deref_mut();
        }
    }
    fn track(&self, id: u64) -> Option<&LoopTrack> {
        std::iter::successors(self.tracks.as_deref(), |track| track.next.as_deref())
            .find(|track| track.id == id)
    }
    fn track_mut(&mut self, id: u64) -> Option<&mut LoopTrack> {
        let mut cursor = self.tracks.as_deref_mut();
        while let Some(track) = cursor {
            if track.id == id {
                return Some(track);
            }
            cursor = track.next.as_deref_mut();
        }
        None
    }
    fn release_keyboard(&mut self) {
        let held = self.keys_held;
        self.each_track(|track| {
            for (id, pitch) in held.iter().enumerate() {
                if pitch.is_some() {
                    track
                        .looper
                        .record_event(LoopNoteEvent::Off { id: id as u8 });
                }
            }
        });
        self.keys_held.fill(None);
        self.keyboard.all_notes_off(self.sample_rate);
    }
    fn stop_loop(&mut self) {
        self.transport.stop();
        let rate = self.sample_rate;
        self.each_track(|track| {
            track.looper.stop();
            track.playback.all_notes_off(rate);
        });
    }
    pub(crate) fn handle(&mut self, event: SynthEvent) {
        match event {
            SynthEvent::AddSynth(mut instrument) => {
                if self.instrument(instrument.id).is_some() {
                    return;
                }
                instrument.next = self.instruments.take();
                self.instruments = Some(instrument);
            }
            SynthEvent::SelectSynth(id) => self.select_synth(id),
            SynthEvent::TrackSynth { id, synth_id } => {
                let Some(settings) = self.instrument(synth_id).map(|node| node.settings) else {
                    return;
                };
                let rate = self.sample_rate;
                if let Some(track) = self.track_mut(id) {
                    if track.synth_id != synth_id {
                        track.playback.all_notes_off(rate);
                        track.synth_id = synth_id;
                        track.playback.set_waveform(settings.waveform);
                        track.playback.set_envelope(settings.env);
                        track.playback.set_voice_mode(settings.mode, rate);
                    }
                }
            }
            SynthEvent::On { id, pitch } => {
                if self.keys_held[id as usize].is_some() {
                    return;
                }
                self.keys_held[id as usize] = Some(pitch);
                self.each_track(|track| track.looper.record_event(LoopNoteEvent::On { id, pitch }));
                Self::note_on(&mut self.keyboard, id, pitch, self.sample_rate);
            }
            SynthEvent::Off(id) => {
                self.keys_held[id as usize] = None;
                self.each_track(|track| track.looper.record_event(LoopNoteEvent::Off { id }));
                self.keyboard.note_off(id, 0.25, self.sample_rate);
            }
            SynthEvent::AllOff => self.release_keyboard(),
            SynthEvent::PlayLoop(settings) => {
                if !self.transport.running && settings.is_valid() {
                    self.release_keyboard();
                    self.transport.play(settings, self.sample_rate);
                    let rate = self.sample_rate;
                    self.each_track(|track| {
                        track.playback.all_notes_off(rate);
                        track.looper.configure(settings, rate);
                        track.looper.play();
                    });
                }
            }
            SynthEvent::ArmTrack(id) => {
                if self.transport.running {
                    if let Some(synth_id) = self.track(id).map(|track| track.synth_id) {
                        self.select_synth(synth_id);
                        self.track_mut(id).unwrap().looper.arm_recording();
                    }
                }
            }
            SynthEvent::AddTrack(mut track) => {
                // IDs are stable, even when another track has been deleted.
                if self.track(track.id).is_some() {
                    track.next = self.garbage.take();
                    self.garbage = Some(track);
                    return;
                }
                let settings = self
                    .instrument(track.synth_id)
                    .unwrap_or_else(|| self.instrument(1).unwrap())
                    .settings;
                if self.instrument(track.synth_id).is_none() {
                    track.synth_id = 1;
                }
                track.playback.set_waveform(settings.waveform);
                track.playback.set_envelope(settings.env);
                track
                    .playback
                    .set_voice_mode(settings.mode, self.sample_rate);
                track
                    .looper
                    .configure(self.transport.settings, self.sample_rate);
                if self.transport.running {
                    track.looper.play();
                    track
                        .looper
                        .follow_transport(self.transport.position, false);
                }
                track.next = self.tracks.take();
                self.tracks = Some(track);
            }
            SynthEvent::DeleteTrack(id) => {
                if let Some(mut track) = detach(&mut self.tracks, id) {
                    track.looper.stop();
                    track.playback.all_notes_off(self.sample_rate);
                    track.next = self.retiring.take();
                    self.retiring = Some(track);
                }
            }
            SynthEvent::MuteTrack { id, muted } => {
                if let Some(track) = self.track_mut(id) {
                    track.muted = muted;
                }
            }
            SynthEvent::TrackVolume { id, volume } => {
                if volume.is_finite() {
                    if let Some(track) = self.track_mut(id) {
                        track.volume = volume.clamp(0.0, 1.0);
                    }
                }
            }
            SynthEvent::StopLoop => self.stop_loop(),
            SynthEvent::SetClick(enabled) => self.metronome.set_enabled(enabled),
            SynthEvent::SetWaveform(waveform) => {
                self.waveform = waveform;
                let id = self.selected_synth;
                self.instrument_mut(id).unwrap().settings.waveform = waveform;
                self.keyboard.set_waveform(waveform);
                self.each_track(|track| {
                    if track.synth_id == id {
                        track.playback.set_waveform(waveform);
                    }
                });
            }
            SynthEvent::SetEnvelope(env) => {
                let id = self.selected_synth;
                self.instrument_mut(id).unwrap().settings.env = env;
                self.keyboard.set_envelope(env);
                self.each_track(|track| {
                    if track.synth_id == id {
                        track.playback.set_envelope(env);
                    }
                });
            }
            SynthEvent::SetVoiceMode(mode) => {
                self.release_keyboard();
                self.mode = mode;
                let id = self.selected_synth;
                self.instrument_mut(id).unwrap().settings.mode = mode;
                let rate = self.sample_rate;
                self.keyboard.set_voice_mode(mode, rate);
                self.each_track(|track| {
                    if track.synth_id == id {
                        track.playback.set_voice_mode(mode, rate);
                    }
                });
            }
            SynthEvent::Shutdown => {
                self.release_keyboard();
                self.stop_loop();
                self.metronome.set_enabled(false);
                let rate = self.sample_rate;
                let mut nodes = self.instruments.as_deref_mut();
                while let Some(node) = nodes {
                    if let Some(keyboard) = &mut node.keyboard {
                        keyboard.all_notes_off(rate);
                    }
                    nodes = node.next.as_deref_mut();
                }
            }
        }
    }
    fn note_on(synth: &mut Synth, id: u8, pitch: u8, sample_rate: u32) {
        let frequency = 440.0 * 2.0_f64.powf((f64::from(pitch) - 69.0) / 12.0);
        synth.note_on(id, frequency, 0.5, 0.25, sample_rate);
    }
    pub(crate) fn next_sample(&mut self) -> f32 {
        let rate = self.sample_rate;
        let mut mix = self.keyboard.next_sample(rate);
        let mut instruments = self.instruments.as_deref_mut();
        while let Some(instrument) = instruments {
            if let Some(keyboard) = &mut instrument.keyboard {
                mix += keyboard.next_sample(rate);
            }
            instruments = instrument.next.as_deref_mut();
        }
        let mut cursor = self.tracks.as_deref_mut();
        while let Some(track) = cursor {
            while let Some(event) = track.looper.next_event() {
                match event {
                    LoopNoteEvent::On { id, pitch } => {
                        Self::note_on(&mut track.playback, id, pitch, rate)
                    }
                    LoopNoteEvent::Off { id } => track.playback.note_off(id, 0.25, rate),
                }
            }
            let sample = track.playback.next_sample(rate);
            if !track.muted {
                mix += sample * f64::from(track.volume);
            }
            cursor = track.next.as_deref_mut();
        }
        let mut link = &mut self.retiring;
        while link.is_some() {
            let track = link.as_mut().unwrap();
            let sample = track.playback.next_sample(rate);
            if !track.muted {
                mix += sample * f64::from(track.volume);
            }
            if track.playback.is_active() {
                link = &mut link.as_mut().unwrap().next;
            } else {
                let mut done = link.take().unwrap();
                *link = done.next.take();
                done.next = self.garbage.take();
                self.garbage = Some(done);
            }
        }
        mix += self.metronome.next_sample(
            self.transport.running.then_some(self.transport.position),
            self.transport.length,
            self.transport.settings.beats,
        );
        let wrapped = self.transport.advance();
        let mut finished = false;
        let mut recording = false;
        let held = self.keys_held;
        let position = self.transport.position;
        self.each_track(|track| {
            let before = track.looper.state();
            track.looper.follow_transport(position, wrapped);
            let after = track.looper.state();
            if before == LoopState::Playing && after == LoopState::Recording {
                track.playback.all_notes_off(rate);
                for (id, pitch) in held.iter().enumerate() {
                    if let Some(pitch) = pitch {
                        track.looper.record_event(LoopNoteEvent::On {
                            id: id as u8,
                            pitch: *pitch,
                        });
                    }
                }
            }
            finished |= before == LoopState::Recording && after == LoopState::Playing;
            recording |= after == LoopState::Recording;
        });
        if finished && !recording {
            self.release_keyboard();
        }
        mix.clamp(-1.0, 1.0) as f32
    }
    pub(crate) fn scope_trigger(&self) -> bool {
        let tracks = std::iter::successors(self.tracks.as_deref(), |track| track.next.as_deref())
            .filter(|track| !track.muted && track.volume > 0.0)
            .filter_map(|track| track.playback.lowest_oscillator());
        let retiring =
            std::iter::successors(self.retiring.as_deref(), |track| track.next.as_deref())
                .filter(|track| !track.muted && track.volume > 0.0)
                .filter_map(|track| track.playback.lowest_oscillator());
        let other_synths =
            std::iter::successors(self.instruments.as_deref(), |node| node.next.as_deref())
                .filter_map(|node| node.keyboard.as_ref().and_then(Synth::lowest_oscillator));
        self.keyboard
            .lowest_oscillator()
            .into_iter()
            .chain(other_synths)
            .chain(tracks)
            .chain(retiring)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .is_some_and(|(frequency, phase)| {
                phase < std::f64::consts::TAU * frequency / f64::from(self.sample_rate)
            })
    }
    pub(crate) fn publish_status(&mut self) {
        let mut state = if self.transport.running {
            LoopState::Playing
        } else {
            LoopState::Stopped
        };
        let mut armed = false;
        let mut cursor = self.tracks.as_deref();
        if let Ok(mut statuses) = self.monitor.tracks.try_lock() {
            while let Some(track) = cursor {
                if track.looper.state() == LoopState::Recording {
                    state = LoopState::Recording;
                }
                armed |= track.looper.armed();
                if let Some(status) = statuses.iter_mut().find(|status| status.id == track.id) {
                    *status = TrackStatus {
                        id: track.id,
                        synth_id: track.synth_id,
                        state: track.looper.state(),
                        armed: track.looper.armed(),
                        muted: track.muted,
                        volume: track.volume,
                    };
                }
                cursor = track.next.as_deref();
            }
        } else {
            while let Some(track) = cursor {
                if track.looper.state() == LoopState::Recording {
                    state = LoopState::Recording;
                }
                armed |= track.looper.armed();
                cursor = track.next.as_deref();
            }
        }
        if let Ok(mut status) = self.monitor.status.try_lock() {
            *status = LoopStatus {
                state,
                settings: self.transport.settings,
                position: self.transport.position,
                length: self.transport.length,
                armed,
                sample_rate: self.sample_rate,
            };
        }
        if let Ok(mut retired) = self.monitor.retired.try_lock() {
            if retired.is_none() {
                *retired = self.garbage.take();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiple_tracks_share_one_synths_settings_without_affecting_other_synths() {
        let (mut engine, _) = recorded(VoiceMode::Polyphonic);
        let (mut reference, _) = recorded(VoiceMode::Polyphonic);
        let noise = SynthSettings {
            waveform: Waveform::WhiteNoise,
            mode: VoiceMode::Polyphonic,
            env: ADSR::new(0.0, 0.1, 0.0, 0.03),
        };
        engine.handle(SynthEvent::AddSynth(Instrument::new(2, noise)));
        engine.handle(SynthEvent::SelectSynth(2));
        for id in 2..=4 {
            let mut track = LoopTrack::new(id, noise.waveform, noise.mode, noise.env, 8_000);
            track.synth_id = 2;
            engine.monitor.register_track(id);
            engine.handle(SynthEvent::AddTrack(track));
        }
        let edited = ADSR::new(0.0, 0.15, 0.0, 0.06);
        engine.handle(SynthEvent::SetEnvelope(edited));
        engine.handle(SynthEvent::SetWaveform(Waveform::FilteredNoise));
        engine.handle(SynthEvent::SetVoiceMode(VoiceMode::Monophonic));
        assert!(engine.transport.running);
        assert_eq!(
            engine.instrument(1).unwrap().settings.waveform,
            Waveform::Sine
        );
        assert_eq!(
            engine.instrument(2).unwrap().settings.waveform,
            Waveform::FilteredNoise
        );
        for id in 2..=4 {
            assert_eq!(engine.track(id).unwrap().playback.envelope(), edited);
        }
        assert_eq!(
            engine.track(1).unwrap().playback.envelope(),
            reference.track(1).unwrap().playback.envelope()
        );
        // Editing the noise instrument does not alter the already playing pitched track.
        for _ in 0..1_600 {
            assert_eq!(engine.next_sample(), reference.next_sample());
        }
        engine.handle(SynthEvent::SelectSynth(1));
        assert_eq!(engine.waveform, Waveform::Sine);
        assert_eq!(engine.mode, VoiceMode::Polyphonic);
    }

    #[test]
    fn track_routing_changes_sound_settings_without_changing_the_shared_clock() {
        let (mut engine, _) = recorded(VoiceMode::Polyphonic);
        let settings = SynthSettings {
            waveform: Waveform::FilteredNoise,
            mode: VoiceMode::Polyphonic,
            env: ADSR::new(0.0, 0.1, 0.0, 0.03),
        };
        engine.handle(SynthEvent::AddSynth(Instrument::new(2, settings)));
        for _ in 0..317 {
            engine.next_sample();
        }
        let position = engine.transport.position;
        let events = engine.track(1).unwrap().looper.events_for_test();
        engine.handle(SynthEvent::TrackSynth { id: 1, synth_id: 2 });
        assert_eq!(engine.transport.position, position);
        assert_eq!(engine.track(1).unwrap().synth_id, 2);
        assert_eq!(engine.track(1).unwrap().playback.envelope(), settings.env);
        assert_eq!(engine.track(1).unwrap().looper.events_for_test(), events);
        engine.handle(SynthEvent::TrackSynth {
            id: 1,
            synth_id: 999,
        });
        assert_eq!(engine.track(1).unwrap().synth_id, 2);
        // Arming a routed track also selects its instrument for live playing.
        engine.handle(SynthEvent::ArmTrack(1));
        assert_eq!(engine.selected_synth, 2);
        for _ in 317..1_600 {
            assert!(engine.next_sample().is_finite());
        }
        assert_eq!(
            engine.track(1).unwrap().looper.state(),
            LoopState::Recording
        );
    }

    #[test]
    fn switching_live_synths_keeps_release_tails_and_key_ownership_separate() {
        let mut engine = LiveEngine::new(
            Waveform::Sine,
            VoiceMode::Polyphonic,
            8_000,
            LoopMonitor::default(),
        );
        engine.handle(SynthEvent::AddSynth(Instrument::new(
            2,
            SynthSettings {
                waveform: Waveform::Triangle,
                mode: VoiceMode::Legato,
                env: ADSR::default(),
            },
        )));
        engine.handle(SynthEvent::On { id: 60, pitch: 60 });
        for _ in 0..100 {
            engine.next_sample();
        }
        engine.handle(SynthEvent::SelectSynth(2));
        assert!(engine.keys_held.iter().all(Option::is_none));
        assert!(
            engine
                .instrument(1)
                .unwrap()
                .keyboard
                .as_ref()
                .unwrap()
                .is_active()
        );
        engine.handle(SynthEvent::On { id: 60, pitch: 72 });
        for _ in 0..100 {
            assert!(engine.next_sample().is_finite());
        }
        engine.handle(SynthEvent::Off(60));
        for _ in 0..2_000 {
            engine.next_sample();
        }
        assert!(!engine.keyboard.is_active());
        assert!(
            !engine
                .instrument(1)
                .unwrap()
                .keyboard
                .as_ref()
                .unwrap()
                .is_active()
        );
        engine.handle(SynthEvent::SelectSynth(1));
        assert_eq!(engine.waveform, Waveform::Sine);
    }

    fn add_track(engine: &mut LiveEngine, id: u64) {
        engine.monitor.register_track(id);
        let track = LoopTrack::new(
            id,
            engine.waveform,
            engine.mode,
            engine.envelope(),
            engine.sample_rate,
        );
        engine.handle(SynthEvent::AddTrack(track));
    }

    #[test]
    fn new_tracks_join_the_shared_clock_and_arm_on_the_same_boundary() {
        let mut engine = LiveEngine::new(
            Waveform::Sine,
            VoiceMode::Polyphonic,
            8_000,
            LoopMonitor::default(),
        );
        engine.handle(SynthEvent::PlayLoop(LoopSettings {
            bpm: 123.4,
            beats: 3,
        }));
        for _ in 0..731 {
            engine.next_sample();
        }
        for id in 2..=40 {
            add_track(&mut engine, id);
        }
        assert_eq!(engine.transport.position, 731);
        for id in 1..=40 {
            let track = engine.track(id).unwrap();
            assert_eq!(track.looper.position(), engine.transport.position);
            assert_eq!(track.looper.length(), engine.transport.length);
            engine.handle(SynthEvent::ArmTrack(id));
        }
        let remaining = engine.transport.length - engine.transport.position;
        for _ in 0..remaining {
            engine.next_sample();
        }
        for id in 1..=40 {
            let track = engine.track(id).unwrap();
            assert_eq!(track.looper.state(), LoopState::Recording);
            assert_eq!(track.looper.position(), 0);
        }
        for _ in 0..engine.transport.length {
            engine.next_sample();
        }
        for id in 1..=40 {
            assert_eq!(engine.track(id).unwrap().looper.state(), LoopState::Playing);
            assert_eq!(engine.track(id).unwrap().looper.position(), 0);
        }
    }

    #[test]
    fn recording_a_second_track_preserves_the_first_take_and_playback() {
        let (mut engine, _) = recorded(VoiceMode::Polyphonic);
        let first_events = engine.track(1).unwrap().looper.events_for_test();
        for _ in 0..317 {
            engine.next_sample();
        }
        add_track(&mut engine, 2);
        engine.handle(SynthEvent::ArmTrack(2));
        for _ in 317..1_600 {
            engine.next_sample();
        }
        assert_eq!(engine.track(1).unwrap().looper.state(), LoopState::Playing);
        assert_eq!(
            engine.track(2).unwrap().looper.state(),
            LoopState::Recording
        );
        engine.handle(SynthEvent::On { id: 60, pitch: 67 });
        for _ in 0..400 {
            engine.next_sample();
        }
        engine.handle(SynthEvent::Off(60));
        for _ in 400..1_600 {
            engine.next_sample();
        }
        assert_eq!(
            engine.track(1).unwrap().looper.events_for_test(),
            first_events
        );
        assert_eq!(engine.track(2).unwrap().looper.state(), LoopState::Playing);
        assert_eq!(engine.track(2).unwrap().looper.events_for_test().len(), 2);
        engine.handle(SynthEvent::MuteTrack { id: 1, muted: true });
        engine.handle(SynthEvent::MuteTrack { id: 2, muted: true });
        for _ in 0..1_600 {
            assert_eq!(engine.next_sample(), 0.0);
        }
        engine.handle(SynthEvent::MuteTrack {
            id: 2,
            muted: false,
        });
        assert!((0..100).any(|_| engine.next_sample() != 0.0));
        // Muting did not reset or suspend either track's clock.
        assert_eq!(
            engine.track(1).unwrap().looper.position(),
            engine.track(2).unwrap().looper.position()
        );
    }

    #[test]
    fn deletion_keeps_stable_ids_and_reclaims_storage_outside_audio() {
        let (mut engine, _) = recorded(VoiceMode::Polyphonic);
        add_track(&mut engine, 2);
        engine.next_sample();
        let position = engine.transport.position;
        engine.monitor.remove_track(1);
        engine.handle(SynthEvent::DeleteTrack(1));
        assert!(engine.track(1).is_none());
        assert!(engine.track(2).is_some());
        assert_eq!(engine.transport.position, position);
        for _ in 0..100 {
            engine.next_sample();
        }
        engine.publish_status();
        assert!(engine.monitor.retired.lock().unwrap().is_some());
        engine.monitor.read();
        assert!(engine.monitor.retired.lock().unwrap().is_none());
        add_track(&mut engine, 3);
        engine.handle(SynthEvent::TrackVolume {
            id: 3,
            volume: 0.25,
        });
        assert_eq!(engine.track(3).unwrap().volume, 0.25);
        assert_eq!(engine.track(2).unwrap().volume, 1.0);
        engine.handle(SynthEvent::DeleteTrack(2));
        engine.handle(SynthEvent::DeleteTrack(3));
        assert!(engine.tracks.is_none());
        engine.handle(SynthEvent::StopLoop);
        for _ in 0..100 {
            engine.next_sample();
        }
        assert_eq!(engine.next_sample(), 0.0);
    }

    #[test]
    fn track_volume_changes_only_that_tracks_contribution() {
        let (mut engine, _) = recorded(VoiceMode::Polyphonic);
        let (mut reference, _) = recorded(VoiceMode::Polyphonic);
        engine.handle(SynthEvent::TrackVolume {
            id: 1,
            volume: 0.25,
        });
        for _ in 0..1_600 {
            assert!((engine.next_sample() - reference.next_sample() * 0.25).abs() < 0.000001);
        }
    }

    #[test]
    fn play_starts_click_and_held_notes_are_recorded_at_the_armed_boundary() {
        let mut engine = LiveEngine::new(
            Waveform::Sine,
            VoiceMode::Polyphonic,
            8_000,
            LoopMonitor::default(),
        );
        engine.handle(SynthEvent::PlayLoop(LoopSettings {
            bpm: 300.0,
            beats: 1,
        }));
        let click: Vec<_> = (0..100).map(|_| engine.next_sample()).collect();
        assert!(click.iter().any(|sample| *sample != 0.0));
        engine.handle(SynthEvent::SetClick(false));
        engine.handle(SynthEvent::On { id: 60, pitch: 60 });
        engine.handle(SynthEvent::ArmTrack(1));
        for _ in 0..1_499 {
            engine.next_sample();
        }
        assert_eq!(engine.track(1).unwrap().looper.state(), LoopState::Playing);
        engine.next_sample();
        assert_eq!(
            engine.track(1).unwrap().looper.state(),
            LoopState::Recording
        );
        assert_eq!(engine.track(1).unwrap().looper.position(), 0);
        for _ in 0..100 {
            engine.next_sample();
        }
        engine.handle(SynthEvent::Off(60));
        for _ in 0..1_500 {
            engine.next_sample();
        }
        assert_eq!(engine.track(1).unwrap().looper.state(), LoopState::Playing);
        assert!(matches!(
            engine.track_mut(1).unwrap().looper.next_event(),
            Some(LoopNoteEvent::On { id: 60, pitch: 60 })
        ));
    }

    fn recorded(mode: VoiceMode) -> (LiveEngine, Vec<f32>) {
        let mut engine = LiveEngine::new(Waveform::Sine, mode, 8_000, LoopMonitor::default());
        engine.handle(SynthEvent::SetEnvelope(ADSR::new(0.0, 0.0, 1.0, 0.005)));
        engine.handle(SynthEvent::PlayLoop(LoopSettings {
            bpm: 300.0,
            beats: 1,
        }));
        engine.handle(SynthEvent::SetClick(false));
        engine.handle(SynthEvent::ArmTrack(1));
        for _ in 0..1_600 {
            engine.next_sample();
        }
        assert_eq!(
            engine.track(1).unwrap().looper.state(),
            LoopState::Recording
        );
        engine.handle(SynthEvent::On { id: 60, pitch: 60 });
        let mut take = Vec::new();
        for position in 0..1_600 {
            if position == 800 {
                engine.handle(SynthEvent::Off(60));
            }
            take.push(engine.next_sample());
        }
        assert_eq!(engine.track(1).unwrap().looper.state(), LoopState::Playing);
        assert_eq!(engine.track(1).unwrap().looper.position(), 0);
        (engine, take)
    }

    #[test]
    fn audio_repeats_the_take_at_exact_boundaries_in_every_voice_mode() {
        for mode in [
            VoiceMode::Polyphonic,
            VoiceMode::Monophonic,
            VoiceMode::Legato,
        ] {
            let (mut engine, take) = recorded(mode);
            for _ in 0..3 {
                let playback: Vec<_> = (0..1_600).map(|_| engine.next_sample()).collect();
                assert_eq!(take, playback);
                assert_eq!(engine.track(1).unwrap().looper.position(), 0);
            }
            engine.handle(SynthEvent::StopLoop);
            for _ in 0..50 {
                engine.next_sample();
            }
            assert_eq!(engine.next_sample(), 0.0);
            assert_eq!(engine.track(1).unwrap().looper.state(), LoopState::Stopped);
        }
    }

    #[test]
    fn live_key_releases_and_focus_loss_do_not_stop_the_loop() {
        let (mut engine, _) = recorded(VoiceMode::Polyphonic);
        let (mut reference, _) = recorded(VoiceMode::Polyphonic);
        engine.handle(SynthEvent::On { id: 60, pitch: 72 });
        for _ in 0..100 {
            engine.next_sample();
            reference.next_sample();
        }
        engine.handle(SynthEvent::Off(60));
        engine.handle(SynthEvent::AllOff);
        for _ in 0..50 {
            engine.next_sample();
            reference.next_sample();
        }
        for _ in 0..1_600 {
            assert_eq!(engine.next_sample(), reference.next_sample());
        }
        assert_eq!(engine.track(1).unwrap().looper.state(), LoopState::Playing);
    }

    #[test]
    fn click_is_not_recorded_and_muting_it_does_not_reset_the_clock() {
        let mut engine = LiveEngine::new(
            Waveform::Sine,
            VoiceMode::Polyphonic,
            8_000,
            LoopMonitor::default(),
        );
        engine.handle(SynthEvent::SetClick(true));
        engine.handle(SynthEvent::PlayLoop(LoopSettings {
            bpm: 300.0,
            beats: 1,
        }));
        engine.handle(SynthEvent::ArmTrack(1));
        for _ in 0..1_600 {
            engine.next_sample();
        }
        let take: Vec<_> = (0..1_600).map(|_| engine.next_sample()).collect();
        assert!(take.iter().any(|sample| *sample != 0.0));
        for _ in 0..17 {
            engine.next_sample();
        }
        let position = engine.track(1).unwrap().looper.position();
        engine.handle(SynthEvent::SetClick(false));
        assert_eq!(engine.track(1).unwrap().looper.position(), position);
        for _ in 0..3_200 {
            assert_eq!(engine.next_sample(), 0.0);
        }
        engine.handle(SynthEvent::Shutdown);
        assert_eq!(engine.track(1).unwrap().looper.state(), LoopState::Stopped);
    }

    #[test]
    fn invalid_settings_are_rejected_and_settings_stay_fixed_while_running() {
        let mut engine = LiveEngine::new(
            Waveform::Sine,
            VoiceMode::Polyphonic,
            8_000,
            LoopMonitor::default(),
        );
        engine.handle(SynthEvent::PlayLoop(LoopSettings {
            bpm: f64::NAN,
            beats: 0,
        }));
        assert_eq!(engine.track(1).unwrap().looper.state(), LoopState::Stopped);
        let settings = LoopSettings {
            bpm: 123.4,
            beats: 3,
        };
        engine.handle(SynthEvent::PlayLoop(settings));
        engine.next_sample();
        engine.handle(SynthEvent::PlayLoop(LoopSettings::default()));
        assert_eq!(engine.track(1).unwrap().looper.settings(), settings);
        assert_eq!(engine.track(1).unwrap().looper.position(), 1);
    }
}
