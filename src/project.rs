//! Versioned, editable recordings. Disk I/O and preparation run on the UI thread.
use crate::{
    live_engine::{LiveEngine, LoopMonitor, SynthSettings},
    looper::{LoopNoteEvent, LoopSettings, RecordedEvent},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ProjectSynth {
    pub id: u64,
    pub name: String,
    pub settings: SynthSettings,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct ProjectTrack {
    pub id: u64,
    pub synth_id: u64,
    pub muted: bool,
    #[serde(default)]
    pub solo: bool,
    pub volume: f32,
    pub events: Vec<RecordedEvent>,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Project {
    pub version: u32,
    pub sample_rate: u32,
    pub settings: LoopSettings,
    pub click_enabled: bool,
    pub selected_synth: u64,
    pub synths: Vec<ProjectSynth>,
    pub tracks: Vec<ProjectTrack>,
}
impl Project {
    pub fn read(path: &str) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        // Bound untrusted input before allocating/deserializing it.
        if file.metadata().map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
            return Err("Project exceeds 64 MiB".into());
        }
        let project: Self = serde_json::from_reader(file).map_err(|e| e.to_string())?;
        project.validate()?;
        Ok(project)
    }
    pub fn write(&self, path: &str) -> Result<(), String> {
        self.validate()?;
        let data = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if let Some(parent) = std::path::Path::new(path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // Write beside the destination, then replace it only after a complete write.
        let temporary = format!("{path}.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        use std::io::Write;
        let result = (|| {
            file.write_all(&data)?;
            file.sync_all()?;
            if std::path::Path::new(path).exists() {
                std::fs::copy(path, format!("{path}.bak"))?;
            }
            std::fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|e| e.to_string())
    }
    pub fn validate(&self) -> Result<(), String> {
        let fail = || Err("Invalid project settings, routing, or note events".into());
        if self.version != 2 {
            return Err(format!("Unsupported project version {}", self.version));
        }
        if !(1..=384_000).contains(&self.sample_rate)
            || !self.settings.is_valid()
            || self.synths.is_empty()
        {
            return fail();
        }
        let mut ids = std::collections::HashSet::new();
        for synth in &self.synths {
            let s = synth.settings;
            if synth.id == 0
                || synth.id == u64::MAX
                || !ids.insert(synth.id)
                || synth.name.len() > 1024
                || !(-3..=3).contains(&s.octave_shift)
            {
                return fail();
            }
            for (v, max) in [
                (s.env.attack_sec(), 2.0),
                (s.env.decay_sec(), 2.0),
                (s.env.sustain_level(), 1.0),
                (s.env.release_sec(), 3.0),
            ] {
                if !v.is_finite() || !(0.0..=max).contains(&v) {
                    return fail();
                }
            }
        }
        if !ids.contains(&self.selected_synth) {
            return fail();
        }
        let length = f64::from(self.settings.beats);
        let mut tracks = std::collections::HashSet::new();
        for track in &self.tracks {
            if track.id == 0
                || track.id == u64::MAX
                || !tracks.insert(track.id)
                || !ids.contains(&track.synth_id)
                || !track.volume.is_finite()
                || !(0.0..=1.0).contains(&track.volume)
                || track.events.len() > 16_640
            {
                return fail();
            }
            let mut held = [false; 256];
            let mut previous = 0.0;
            for e in &track.events {
                if !e.position_beats.is_finite()
                    || e.position_beats < previous
                    || e.position_beats >= length
                {
                    return fail();
                }
                previous = e.position_beats;
                match e.event {
                    LoopNoteEvent::On { id, pitch } => {
                        if pitch > 127 || held[id as usize] {
                            return fail();
                        }
                        held[id as usize] = true;
                    }
                    LoopNoteEvent::Off { id } => {
                        if !held[id as usize] {
                            return fail();
                        }
                        held[id as usize] = false;
                    }
                }
            }
            if held.iter().any(|h| *h) {
                return fail();
            }
        }
        Ok(())
    }
    pub fn prepare(&self, rate: u32, monitor: LoopMonitor) -> Box<LiveEngine> {
        LiveEngine::from_project(self, rate, monitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{envelope::ADSR, synth::VoiceMode, synth_event::SynthEvent, waveform::Waveform};
    fn example() -> Project {
        Project {
            version: 2,
            sample_rate: 8000,
            settings: LoopSettings {
                bpm: 300.0,
                beats: 1,
            },
            click_enabled: false,
            selected_synth: 7,
            synths: vec![ProjectSynth {
                id: 7,
                name: "Bass".into(),
                settings: SynthSettings {
                    waveform: Waveform::Sine,
                    mode: VoiceMode::Polyphonic,
                    env: ADSR::new(0.0, 0.0, 1.0, 0.005),
                    octave_shift: -2,
                },
            }],
            tracks: vec![ProjectTrack {
                id: 9,
                synth_id: 7,
                muted: false,
                solo: false,
                volume: 0.5,
                events: vec![
                    RecordedEvent {
                        position_beats: 0.0,
                        event: LoopNoteEvent::On { id: 60, pitch: 60 },
                    },
                    RecordedEvent {
                        position_beats: 0.5,
                        event: LoopNoteEvent::Off { id: 60 },
                    },
                ],
            }],
        }
    }
    #[test]
    fn solo_is_saved_and_older_projects_default_to_unsoloed() {
        let mut p = example();
        p.tracks[0].solo = true;
        let mut json = serde_json::to_value(&p).unwrap();
        assert!(
            serde_json::from_value::<Project>(json.clone())
                .unwrap()
                .tracks[0]
                .solo
        );
        json["tracks"][0].as_object_mut().unwrap().remove("solo");
        assert!(!serde_json::from_value::<Project>(json).unwrap().tracks[0].solo);
    }

    #[test]
    fn json_round_trip_restores_audio_routing_and_rescales_timing() {
        let original = example();
        let json = serde_json::to_string_pretty(&original).unwrap();
        let restored: Project = serde_json::from_str(&json).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.synths[0].name, "Bass");
        for rate in [8000, 16000] {
            let mut engine = restored.prepare(rate, LoopMonitor::default());
            engine.handle(SynthEvent::PlayLoop(restored.settings));
            let audio: Vec<_> = (0..rate / 5).map(|_| engine.next_sample()).collect();
            assert!(audio[..(rate / 10) as usize].iter().any(|v| v.abs() > 0.01));
            assert!(
                audio[(rate / 10 + rate / 100) as usize..]
                    .iter()
                    .all(|v| *v == 0.0)
            );
            let mut reference = crate::synth::Synth::new(8, Waveform::Sine, VoiceMode::Polyphonic);
            reference.set_envelope(original.synths[0].settings.env);
            let freq = 440.0 * 2.0_f64.powf((36.0 - 69.0) / 12.0);
            reference.note_on(60, freq, 0.5, 0.25, rate);
            for sample in audio.iter().take((rate / 10) as usize) {
                assert_eq!(*sample, (reference.next_sample(rate) * 0.5) as f32);
            }
        }
    }
    #[test]
    fn invalid_projects_are_rejected_before_preparation() {
        let mut p = example();
        p.version = 3;
        assert!(p.validate().is_err());
        p.version = 2;
        p.tracks[0].synth_id = 100;
        assert!(p.validate().is_err());
        p.tracks[0].synth_id = 7;
        p.tracks[0].events[1].position_beats = 1.0;
        assert!(p.validate().is_err());
        p.tracks[0].events[1].position_beats = 0.5;
        p.tracks[0].events.pop();
        assert!(p.validate().is_err());
        assert!(serde_json::from_str::<Project>("not json").is_err());
    }
    #[test]
    fn disk_save_is_reloadable_and_can_replace_an_existing_project() {
        let path = std::env::temp_dir().join(format!(
            "rustsynth-project-test-{}.json",
            std::process::id()
        ));
        let path = path.to_str().unwrap();
        let mut p = example();
        p.write(path).unwrap();
        p.synths[0].name = "Edited".into();
        p.write(path).unwrap();
        assert_eq!(Project::read(path).unwrap().synths[0].name, "Edited");
        assert_eq!(
            Project::read(&format!("{path}.bak")).unwrap().synths[0].name,
            "Bass"
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(format!("{path}.bak")).unwrap();
    }
}
