# RustSynth

A Rust synth learning project with live keyboard playback and WAV rendering.

## Play live

```sh
cargo run
```

Focus the RustSynth window, then hold keys to play notes. Multiple keys play chords.
The window captures physical key positions (the labels below assume QWERTY).

| Computer key | Note |
| --- | --- |
| A | C4 |
| W | C♯4 |
| S | D4 |
| E | D♯4 |
| D | E4 |
| F | F4 |
| T | F♯4 |
| G | G4 |
| Y | G♯4 |
| H | A4 (440 Hz) |
| U | A♯4 |
| J | B4 |
| K | C5 |

Releasing a key starts its release tail. Changing window focus releases all notes.
Hold Shift when pressing a key to play one octave higher. Releasing Shift does
not change notes that are already sounding.

Choose Sine, Triangle, Square, or Sawtooth from the waveform dropdown, or press
1–4 while playing. The selector stays synchronized with the number-key shortcuts. Changes
affect both held notes and future notes without restarting their envelopes.

Use **Voice mode** to select Polyphonic (independent voices and chords),
Monophonic (last-note priority with a retriggered envelope), or Legato (last-note
priority while preserving phase and envelope). Changing modes releases held notes
and clears key tracking; release and press the keys again to play in the new mode.
Existing release tails finish fading out. Selecting the current mode has no effect.

The ADSR sliders control the synth's live envelope setting: Attack and Decay
range from 0–2 seconds, Sustain from 0–1, and Release from 0–3 seconds.
Changes apply to new notes. Existing voices and release tails keep their settings;
connected legato notes preserve the current phrase's envelope. Returning
monophonic notes use the envelope saved when their key was first pressed.
Scheduled WAV notes continue to carry their own envelopes.

The live waveform shows the most recent 25 milliseconds of the actual output,
including chords and envelope changes. It refreshes about 30 times per second and
returns to a flat line after notes finish releasing. A fixed 4× display gain makes
quiet notes easier to see; it does not change playback volume. Loud peaks can
extend beyond the display range.

Enable **Lock to lowest note** to align the trace to the cycle start of the lowest
sounding voice, including release tails. Turn it off for the free-running view.
The toggle affects only the display; chords may still move as higher notes shift
relative to the reference. With no suitable trigger, the view uses the latest samples.

While navigating a UI control, musical shortcuts are paused. Key releases still
work. Click the play area to return to musical keyboard input. Escape dismisses
an open selector; otherwise, while playing, it closes the window. You can also
close the window with its close button.

Live mode starts with eight sine-wave voices in polyphonic mode. Initial waveform
and voice mode are chosen in `src/main.rs`; audio setup is in `src/live.rs`.
`src/ui.rs` draws the window, `src/keyboard.rs` tracks musical keys, and
`src/synth_event.rs` defines messages sent to the audio callback. The current audio output path requires an `f32` device configuration.

## Render the demo

```sh
cargo run -- --wav
```

Writes the existing legato sawtooth demo to `wave.wav` as mono, 32-bit float audio.
You can also select live mode explicitly with `cargo run -- --live`.

## Check the project

```sh
cargo fmt --check
cargo check
cargo test
```
