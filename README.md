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
| O | C♯5 |
| L | D5 |
| P | D♯5 |
| ; | E5 |
| ' | F5 |

Releasing a key starts its release tail. Changing window focus releases live keys; loop playback continues.
Hold Shift for one octave up, or Control for one octave down. Holding both uses
the normal octave. Releasing modifiers does not change notes already sounding.
Overlapping pitches from different keys remain independently releasable.

Choose Sine, Triangle, Square, Sawtooth, White noise, or Filtered noise from the
waveform dropdown, or press 1–6 while playing. The selector stays synchronized with the number-key shortcuts. Changes affect held and future notes only on the selected synth and its linked tracks,
without restarting their envelopes.

Use **Voice mode** to select Polyphonic (independent voices and chords),
Monophonic (last-note priority with a retriggered envelope), or Legato (last-note
priority while preserving phase and envelope). The mode belongs to the selected
synth. Changing it releases that synth’s live and track notes without stopping
the shared transport or other synths. Release and press live keys again to play.
Selecting the current mode has no effect.

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

Enable **Stabilize** to align the trace to the cycle start of the lowest
sounding voice, including release tails. Turn it off for the free-running view.
The toggle affects only the display; chords may still move as higher notes shift
relative to the reference. With no suitable trigger, the view uses the latest samples.

While navigating a UI control, musical shortcuts are paused. Key releases still
work. Click the play area to return to musical keyboard input. Escape dismisses
an open selector; otherwise, while playing, it closes the window. You can also
close the window with its close button.

Live mode starts with sine waves in polyphonic mode. Live keys and each loop track have independent pools of eight voices, all using
their linked synth’s voice mode. Waveform and envelope controls apply only to the
selected synth and its linked tracks. Initial waveform
and voice mode are chosen in `src/main.rs`; audio setup is in `src/live.rs`.
`src/ui.rs` draws the window, `src/keyboard.rs` tracks musical keys, and
`src/synth_event.rs` defines messages sent to the audio callback. The current audio output path requires an `f32` device configuration.

## Multiple synths and track routing

The **Synth** dropdown chooses the instrument you edit and play live. **Add synth**
creates another instrument, copying the current waveform, ADSR, and voice mode.
Use **Name** to call instruments “Noise”, “Pitches”, or anything useful.
Each synth remembers its own settings when you select another one.

Each track has a synth-assignment dropdown. Several tracks can share one synth,
or every track can use a separate synth. For example, route three drum tracks to
“Noise” and the melodic tracks to “Pitches”. Editing an instrument updates only
its linked tracks. Tracks share settings, while keeping independent voices and
note releases. Routing changes preserve the recorded notes and the shared clock;
currently sounding notes on the rerouted track release before subsequent events
use the new instrument. ADSR edits continue to apply to new notes only.

New tracks use the currently selected synth. Pressing Record on an existing track
selects its assigned synth for live playing. Switching synths releases live keys,
while the previous synth’s release tails retain their sound. Synths and tracks
are session-only; saving projects is not implemented yet.

## Noise percussion

**5** selects broadband white noise. Its sound is independent of key pitch.
**6** selects filtered noise: a broad band-pass filter follows the played note's
frequency. Lower keys sound darker and higher keys brighter; Shift/Control octave
changes also move the filter. Each voice has its own noise generator and filter.

For a short snare/hat-like hit, try Attack **0 s**, Decay **0.08–0.15 s**, Sustain
**0**, and Release **0.02–0.05 s**, then tap keys in polyphonic mode. Noise decays
to silence even if the key stays held. These are simple noise-based percussion
sounds, not full drum models. Stabilize has no periodic reference for noise voices.
The filter uses the unity-peak band-pass design from the
[W3C Audio EQ Cookbook](https://www.w3.org/TR/audio-eq-cookbook/).

## Record synchronized loop tracks

1. While stopped, choose **BPM** (30–300, including decimals) and **Beats** (1–64).
2. Leave **Click** enabled (the default), or mute it. It ticks during recording and playback, with an accent
   at the beginning of each loop. Muting it does not move the loop clock.
3. Press **Play** to start the loop clock and click. Any existing take also plays.
4. Press **Record** on a track to arm its take. Recording starts at the **next loop boundary**,
   so you can use the remaining beats as a count-in.
5. After one full recorded loop, recording automatically becomes repeating playback.
6. Press **Stop** to stop and rewind, canceling any armed recording. To replace a take,
   press Play and then that track’s Record button again. Changing BPM or beat count while stopped clears
   the old take, since its event times belong to the previous loop length.

Use **Add track** for another independent loop. New tracks join the current sample
clock without restarting it. Each row has **Record**, **Mute**, **Volume**, and
**Delete** controls. Muted tracks keep their place, and recording one track leaves
other takes playing. If multiple tracks are armed, they record the same live input
on the next boundary. Delete removes only that track and lets its release tails
fade out. Track numbers are stable and are not reused after deletion. The track
list scrolls; there is no fixed track-count limit, subject to available CPU/memory.
New track storage is prepared on the UI thread, and deleted storage is reclaimed
there after its release tails finish.

Tempo and length stay fixed while running. The progress bar follows the audio
sample clock. The first version records notes only, with your actual timing;
there is no quantization or overdubbing within a track. The click is never recorded. You can play
live over the loop; releasing a live key does not release its loop counterpart.
Notes held at the recording boundary get recorded releases, and the monitored
live take releases as playback begins. Release tails can overlap loop passes.
Takes have a bounded capacity of 16,384 incoming events plus space to finish held
notes. Additional presses beyond that limit are omitted from the recording.
Closing the window stops both instruments and the click.

`src/transport.rs` owns the shared sample clock. `src/live_engine.rs` routes live
notes, manages tracks, and mixes playback. `src/looper.rs` stores each take and
follows the transport position. `src/metronome.rs` generates the shared click.

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
