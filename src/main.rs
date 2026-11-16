mod demo;
mod envelope;
mod keyboard;
mod live;
mod note;
mod synth;
mod voice;
mod wav;
mod waveform;

use crate::{
    demo::demo_1,
    synth::{Synth, VoiceMode},
    waveform::Waveform,
};

enum AudioMode {
    Live,
    Wav,
}

fn main() {
    let audio_mode = match std::env::args().nth(1).as_deref() {
        None | Some("--live") => AudioMode::Live,
        Some("--wav") => AudioMode::Wav,
        _ => {
            eprintln!("Usage: cargo run -- [--live | --wav]");
            std::process::exit(2);
        }
    };

    match audio_mode {
        AudioMode::Wav => render_wav(),
        AudioMode::Live => live::play_live(Waveform::Sine, VoiceMode::Polyphonic),
    }
}

fn render_wav() {
    let sample_rate = 44_100;
    let amplitude = 0.25;
    let waveform = Waveform::Sawtooth;

    let notes = demo_1();

    let mut synth = Synth::new(8, waveform, VoiceMode::Legato);
    let mut samples = Vec::new();

    let mut total_samples = 0;

    for note in &notes {
        let start_idx = (note.start_sec * sample_rate as f64) as usize;
        let length =
            ((note.duration_sec + note.env.release_sec()) * sample_rate as f64).ceil() as usize;

        total_samples = total_samples.max(start_idx + length);
    }

    for sample_idx in 0..total_samples {
        let time = sample_idx as f64 / sample_rate as f64;
        synth.update_held_notes(time, amplitude, sample_rate);

        for note in &notes {
            let start_idx = (note.start_sec * sample_rate as f64) as usize;
            if sample_idx == start_idx {
                synth.play_note(note, amplitude, sample_rate);
            }
        }

        samples.push(synth.next_sample(sample_rate));
    }

    wav::write_wav("wave.wav", &samples, sample_rate).expect("Could not write WAV file");
}
