use std::sync::mpsc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::{
    envelope::ADSR,
    note::Note,
    oscilloscope::Oscilloscope,
    synth::{Synth, VoiceMode},
    synth_event::SynthEvent,
    ui,
    waveform::Waveform,
};

pub(crate) fn play_live(waveform: Waveform, voice_mode: VoiceMode) {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("No audio output device found");

    let config = device
        .default_output_config()
        .expect("Could not get audio output configuration");

    println!("Audio output configuration: {:?}", config);

    assert_eq!(config.sample_format(), cpal::SampleFormat::F32);

    let stream_config: cpal::StreamConfig = config.into();
    let sample_rate = stream_config.sample_rate;
    let channels = stream_config.channels as usize;

    let mut synth = Synth::new(8, waveform, voice_mode);

    let (sender, receiver) = mpsc::channel();
    let scope = Oscilloscope::new(sample_rate);
    let audio_scope = scope.clone();

    let stream = device
        .build_output_stream(
            stream_config,
            move |data: &mut [f32], _| {
                for event in receiver.try_iter() {
                    match event {
                        SynthEvent::On { id, pitch } => {
                            let note = Note {
                                frequency: 440.0 * 2.0_f64.powf((f64::from(pitch) - 69.0) / 12.0),
                                start_sec: 0.0,
                                duration_sec: 0.0,
                                amplitude: 0.5,
                                env: ADSR::new(0.01, 0.1, 0.7, 0.2),
                            };
                            synth.note_on(id, &note, 0.25, sample_rate);
                        }
                        SynthEvent::Off(id) => synth.note_off(id, 0.25, sample_rate),
                        SynthEvent::AllOff => synth.all_notes_off(sample_rate),
                        SynthEvent::SetWaveform(waveform) => synth.set_waveform(waveform),
                    }
                }

                let mut capture = audio_scope.try_capture();
                for frame in data.chunks_mut(channels) {
                    let trigger = capture.is_some() && synth.scope_trigger(sample_rate);
                    let sample = synth.next_sample(sample_rate) as f32;
                    frame.fill(sample);
                    if let Some(history) = &mut capture {
                        history.push(sample, trigger);
                    }
                }
            },
            |error| {
                eprintln!("Audio stream error: {error}");
            },
            None,
        )
        .expect("Could not create audio stream");

    stream.play().expect("Could not start audio stream");

    ui::run(sender, waveform, scope);
    // Let releases finish before dropping the stream.
    std::thread::sleep(std::time::Duration::from_millis(250));
}
