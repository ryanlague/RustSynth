use crate::{envelope::ADSR, note::Note};

pub(crate) fn demo_1() -> Vec<Note> {
    // An A-minor phrase. Long anchor notes return between the shorter notes
    // above them when played in legato or monophonic mode.
    // Each entry is (frequency, start seconds, held seconds, amplitude).
    let melody = [
        (220.00, 0.00, 2.75, 0.42), // A3: first anchor
        (329.63, 0.50, 0.40, 0.48), // E4
        (440.00, 1.00, 0.40, 0.52), // A4
        (523.25, 1.50, 0.40, 0.56), // C5
        (493.88, 2.00, 0.30, 0.48), // B4, then back to A3
        (293.66, 3.00, 0.55, 0.48), // D4: a new phrase after a short rest
        (349.23, 3.50, 0.55, 0.50), // F4
        (440.00, 4.00, 0.55, 0.54), // A4
        (659.26, 4.50, 0.45, 0.58), // E5: peak
        (220.00, 5.00, 2.75, 0.42), // A3: final anchor
        (587.33, 5.00, 0.40, 0.54), // D5
        (523.25, 5.50, 0.40, 0.50), // C5
        (493.88, 6.00, 0.40, 0.48), // B4
        (440.00, 6.50, 0.55, 0.52), // A4, then settle back to A3
    ];

    melody
        .into_iter()
        .map(|(frequency, start_sec, duration_sec, amplitude)| Note {
            frequency,
            start_sec,
            duration_sec,
            amplitude,
            env: ADSR::new(0.01, 0.1, 0.7, 0.2),
        })
        .collect()
}
