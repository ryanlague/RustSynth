use crate::envelope::ADSR;
use crate::live_engine::{Instrument, LoopTrack};
use crate::looper::LoopSettings;
use crate::synth::VoiceMode;
use crate::waveform::Waveform;

pub(crate) enum SynthEvent {
    On { id: u8, pitch: u8 },
    AddSynth(Box<Instrument>),
    SelectSynth(u64),
    TrackSynth { id: u64, synth_id: u64 },
    Off(u8),
    AllOff,
    PlayLoop(LoopSettings),
    ArmTrack(u64),
    AddTrack(Box<LoopTrack>),
    DeleteTrack(u64),
    MuteTrack { id: u64, muted: bool },
    TrackVolume { id: u64, volume: f32 },
    StopLoop,
    SetClick(bool),
    Shutdown,
    SetWaveform(Waveform),
    SetEnvelope(ADSR),
    SetVoiceMode(VoiceMode),
}
