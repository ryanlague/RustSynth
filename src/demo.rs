use crate::{envelope::ADSR, note::Note};

pub(crate) fn demo_1() -> Vec<Note> {
    vec![
        Note {
            frequency: 440.0,
            start_sec: 0.0,
            duration_sec: 3.0,
            amplitude: 0.5,
            env: ADSR::new(0.01, 0.1, 0.7, 0.2),
        },
        Note {
            frequency: 880.0,
            start_sec: 1.0,
            duration_sec: 1.0,
            amplitude: 0.5,
            env: ADSR::new(0.01, 0.1, 0.7, 0.2),
        },
    ]
}
