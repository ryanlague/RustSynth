use crate::{envelope::ADSR, live_engine::SynthSettings, synth::VoiceMode, waveform::Waveform};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SynthPreset {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) settings: SynthSettings,
}

pub(crate) fn synth_presets() -> [SynthPreset; 9] {
    use VoiceMode::{Legato, Monophonic, Polyphonic};
    use Waveform::{FilteredNoise, Kick, Sawtooth, Sine, Square, Triangle, WhiteNoise};

    [
        (
            "Sine keys",
            "A clean, sustained sine tone.",
            Sine,
            Polyphonic,
            0,
            ADSR::default(),
        ),
        (
            "Pluck",
            "A short triangle pluck that decays even while held.",
            Triangle,
            Polyphonic,
            0,
            ADSR::new(0.001, 0.22, 0.0, 0.08),
        ),
        (
            "Soft pad",
            "A gentle fade-in with a long release.",
            Sine,
            Polyphonic,
            0,
            ADSR::new(0.45, 0.5, 0.75, 1.1),
        ),
        (
            "Bass",
            "A monophonic sawtooth that starts two octaves lower.",
            Sawtooth,
            Monophonic,
            -2,
            ADSR::new(0.004, 0.18, 0.55, 0.1),
        ),
        (
            "Lead",
            "A square-wave lead with connected legato notes.",
            Square,
            Legato,
            1,
            ADSR::new(0.008, 0.12, 0.65, 0.16),
        ),
        (
            "Kick",
            "A low sine hit with a fast downward pitch sweep. Keys change the body pitch.",
            Kick,
            Polyphonic,
            -2,
            ADSR::new(0.001, 0.32, 0.0, 0.12),
        ),
        (
            "Hi-hat",
            "A very short burst of broadband noise.",
            WhiteNoise,
            Polyphonic,
            0,
            ADSR::new(0.0, 0.04, 0.0, 0.01),
        ),
        (
            "Snare",
            "A longer noise burst with a fast attack.",
            WhiteNoise,
            Polyphonic,
            0,
            ADSR::new(0.001, 0.16, 0.0, 0.04),
        ),
        (
            "Noise tom",
            "A filtered-noise hit; keys change its center frequency.",
            FilteredNoise,
            Polyphonic,
            0,
            ADSR::new(0.001, 0.24, 0.0, 0.08),
        ),
    ]
    .map(
        |(name, description, waveform, mode, octave_shift, env)| SynthPreset {
            name,
            description,
            settings: SynthSettings {
                octave_shift,
                waveform,
                mode,
                env,
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_unique_names_and_fit_the_editable_control_ranges() {
        let presets = synth_presets();
        for (index, preset) in presets.iter().enumerate() {
            assert!(!preset.name.is_empty());
            assert!(!preset.description.is_empty());
            assert!(
                presets[..index]
                    .iter()
                    .all(|other| other.name != preset.name)
            );
            let env = preset.settings.env;
            assert!((0.0..=2.0).contains(&env.attack_sec()));
            assert!((0.0..=2.0).contains(&env.decay_sec()));
            assert!((0.0..=1.0).contains(&env.sustain_level()));
            assert!((0.0..=3.0).contains(&env.release_sec()));
        }
        for preset in presets
            .iter()
            .filter(|preset| matches!(preset.name, "Pluck" | "Hi-hat" | "Snare" | "Noise tom"))
        {
            assert_eq!(preset.settings.env.gain(1.0, f64::INFINITY), 0.0);
        }
    }
}
