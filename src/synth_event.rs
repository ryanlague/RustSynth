use crate::envelope::ADSR;
use crate::live_engine::{Instrument, LoopTrack, SynthSettings};
use crate::looper::LoopSettings;
use crate::synth::VoiceMode;
use crate::waveform::Waveform;

pub(crate) enum SynthEvent {
    Snapshot(
        Box<crate::project::Project>,
        std::sync::mpsc::SyncSender<Result<Box<crate::project::Project>, String>>,
    ),
    ReplaceEngine(
        Box<crate::live_engine::LiveEngine>,
        std::sync::mpsc::SyncSender<Box<crate::live_engine::LiveEngine>>,
    ),
    On {
        id: u8,
        pitch: u8,
    },
    AddSynth(Box<Instrument>),
    SelectSynth(u64),
    SetSynthSettings(SynthSettings),
    TrackSynth {
        id: u64,
        synth_id: u64,
    },
    Off(u8),
    AllOff,
    PlayLoop(LoopSettings),
    ArmTrack(u64),
    AddTrack(Box<LoopTrack>),
    DeleteTrack(u64),
    SoloTrack {
        id: u64,
        solo: bool,
    },
    MuteTrack {
        id: u64,
        muted: bool,
    },
    TrackVolume {
        id: u64,
        volume: f32,
    },
    SetBpm(f64),
    StopLoop,
    SetClick(bool),
    Shutdown,
    SetWaveform(Waveform),
    SetEnvelope(ADSR),
    SetOctaveShift(i8),
    SetVoiceMode(VoiceMode),
}
