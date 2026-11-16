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
Press Escape or close the window to exit. The window is an input surface; controls
are shown in its title and in the terminal.

Live mode uses eight sine-wave voices in polyphonic mode. Its synth settings are in
`src/live.rs`. The current audio output path requires an `f32` device configuration.

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
