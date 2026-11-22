use crate::waveform::Waveform;

pub(crate) enum SynthEvent {
    On { id: u8, pitch: u8 },
    Off(u8),
    AllOff,
    SetWaveform(Waveform),
}
