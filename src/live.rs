use std::sync::mpsc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::{
    live_engine::{LiveEngine, LoopMonitor},
    oscilloscope::Oscilloscope,
    synth::VoiceMode,
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

    let monitor = LoopMonitor::default();
    let mut engine = LiveEngine::new(waveform, voice_mode, sample_rate, monitor.clone());
    let env = engine.envelope();

    let (sender, receiver) = mpsc::channel();
    let scope = Oscilloscope::new(sample_rate);
    let audio_scope = scope.clone();

    let stream = device
        .build_output_stream(
            stream_config,
            move |data: &mut [f32], _| {
                for event in receiver.try_iter() {
                    engine.handle(event);
                }

                let mut capture = audio_scope.try_capture();
                for frame in data.chunks_mut(channels) {
                    let trigger = capture.is_some() && engine.scope_trigger();
                    let sample = engine.next_sample();
                    frame.fill(sample);
                    if let Some(history) = &mut capture {
                        history.push(sample, trigger);
                    }
                }
                engine.publish_status();
            },
            |error| {
                eprintln!("Audio stream error: {error}");
            },
            None,
        )
        .expect("Could not create audio stream");

    stream.play().expect("Could not start audio stream");

    ui::run(sender, waveform, voice_mode, scope, env, monitor);
    // Let releases finish before dropping the stream.
    std::thread::sleep(std::time::Duration::from_millis(250));
}
