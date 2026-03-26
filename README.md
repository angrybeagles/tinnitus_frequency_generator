# Tinnitus Frequency Generator

A native Windows 11 desktop application for crafting personalized tinnitus therapy soundscapes. Built in Rust with a real-time GUI, this tool lets you layer oscillators, filters, and evidence-based therapy techniques to find relief from tinnitus.

## Features

- 11 waveform types including colored noise variants
- 9 parametric filter types based on the Audio EQ Cookbook
- 6 therapy techniques (frequency sweeps, binaural beats, notch filtering, amplitude modulation, residual inhibition, fractal tones)
- Real-time waveform and spectrum visualization
- Preset save/load system
- WAV export for offline listening (iPhone-compatible)

## Building

Requires [Rust](https://www.rust-lang.org/tools/install) (stable toolchain).

```
cargo build --release
```

The compiled executable will be at `target/release/tinnitus_freq_generator.exe`.

To run directly:

```
cargo run --release
```

## Oscillators

Each oscillator has independent frequency, volume, pan, and enable controls. Multiple oscillators can be layered simultaneously.

### Standard Waveforms

**Sine** — A pure tone at a single frequency. The most fundamental waveform, useful as a baseline for matching your tinnitus frequency.

**Square** — A waveform that alternates between two levels, producing a hollow, buzzy tone rich in odd harmonics. Can be useful for masking broader frequency ranges than a pure sine.

**Sawtooth** — A waveform that ramps linearly then drops sharply, containing both odd and even harmonics. Produces a bright, buzzy sound.

**Triangle** — Similar to a sine but with a slightly brighter character due to odd harmonics that fall off faster than a square wave. A gentler alternative to square waves.

### Noise Types

**White Noise** — Equal energy at every frequency. A flat "hiss" that masks across the entire audible spectrum.

**Pink Noise** — Energy decreases at 3 dB per octave, giving equal energy per octave. Sounds more balanced and natural than white noise. Often described as rainfall or a waterfall.

**Brown Noise** — Energy decreases at 6 dB per octave, emphasizing low frequencies. A deep, rumbling sound like thunder or heavy surf. Also called Brownian or red noise.

**Blue Noise** — Energy increases at 3 dB per octave, emphasizing high frequencies. The spectral inverse of pink noise.

**Violet Noise** — Energy increases at 6 dB per octave, strongly emphasizing high frequencies. The spectral inverse of brown noise.

**Grey Noise** — Psychoacoustically flat noise shaped by an inverse equal-loudness contour (roughly ISO 226). Sounds equally loud at all frequencies to human hearing, unlike white noise which sounds bright.

**Green Noise** — Bandpass-filtered noise centered around 500 Hz, simulating natural ambient sounds. A calming middle-frequency noise sometimes compared to the background sound of a forest or meadow.

## Filters

All filters are implemented as biquad IIR filters using coefficients from Robert Bristow-Johnson's Audio EQ Cookbook. Each filter has frequency, Q (resonance), gain (where applicable), mix (dry/wet blend), and enable controls. Multiple filters can be stacked in series.

**Low Pass** — Passes frequencies below the cutoff and attenuates those above. Useful for removing harsh high-frequency content.

**High Pass** — Passes frequencies above the cutoff and attenuates those below. Useful for removing low-frequency rumble.

**Band Pass** — Passes a band of frequencies around the center frequency and attenuates everything else. The Q parameter controls bandwidth.

**Notch** — Removes a narrow band of frequencies around the center frequency. Essentially the inverse of a band pass filter.

**Peaking EQ** — Boosts or cuts a band of frequencies by a specified gain in dB. The standard parametric EQ building block.

**Low Shelf** — Boosts or cuts all frequencies below the specified frequency by a given gain in dB.

**High Shelf** — Boosts or cuts all frequencies above the specified frequency by a given gain in dB.

**All Pass** — Passes all frequencies at equal amplitude but shifts the phase. Can be combined with the dry signal (via the mix control) to create phaser-like effects.

**Comb** — A delay-based filter that creates a series of evenly-spaced notches in the frequency spectrum, producing a resonant, metallic quality. The frequency parameter controls the delay length.

## Therapy Techniques

### Frequency Sweep

Continuously sweeps a sine tone between a start and end frequency over a configurable duration. Supports three sweep modes and an optional loop. Useful for scanning across frequency ranges to identify your tinnitus pitch or for broadband stimulation.

### Binaural Beats

Plays two slightly different frequencies in the left and right ears. The brain perceives a "beat" at the difference frequency. For example, a 400 Hz base with a 10 Hz beat frequency plays 400 Hz in the left ear and 410 Hz in the right. Binaural beats at specific frequencies are associated with different brainwave states (delta, theta, alpha, beta). Headphones are required for the binaural effect.

### Notch Filters

Specialized notch filters designed for tinnitus therapy. The theory (tailor-made notched music therapy) involves removing a narrow band of frequencies centered on your tinnitus pitch from the sound you listen to, which may reduce cortical overrepresentation of that frequency over time. Multiple notch filters can be added with independent center frequency, bandwidth, and depth controls.

### Amplitude Modulation

Modulates the overall signal amplitude at a configurable rate and depth. At low rates (1–10 Hz), this creates a pulsing or tremolo effect. Some research suggests amplitude-modulated tones can be more effective for tinnitus masking than continuous tones, as they may reduce habituation and maintain the brain's attention to the masking signal.

### Residual Inhibition

Alternates between a burst of a pure tone and a period of silence. After hearing a tone near the tinnitus frequency for a brief period, many people experience temporary suppression of their tinnitus during the silent interval. This technique automates that cycle with configurable burst frequency, burst duration, silence duration, and burst volume.

### Fractal Tones

Generates melodic, nature-inspired tone sequences using a random walk on the pentatonic scale with occasional octave jumps. Inspired by the approach used in commercial tinnitus devices (such as Widex Zen), fractal tones provide unpredictable but pleasant stimulation that avoids the monotony of static tones. Each note has a soft envelope to prevent clicks. Configurable base frequency, tempo, and volume.

## Signal Chain

The audio signal flows through the following stages in order:

1. **Oscillators** — All enabled oscillators are summed (stereo, with pan)
2. **Frequency Sweep** — Sweep tone added to the mix
3. **Binaural Beats** — Binaural tone added (left/right differ)
4. **Residual Inhibition** — Burst/silence tone added
5. **Fractal Tones** — Melodic tones added
6. **Notch Filters** — Therapeutic notch filters applied in series
7. **General Filters** — Parametric EQ/filter chain applied in series
8. **Amplitude Modulation** — Envelope applied to the combined signal
9. **Master Volume** — Final gain stage
10. **Soft Clip** — `tanh` saturation to prevent harsh digital clipping

## Visualization

**Waveform Scope** — Real-time display of the output waveform, showing the combined left/right signal after all processing.

**Spectrum Analyzer** — Real-time FFT-based spectrum display with 1 kHz bandwidth buckets from 0–20 kHz. Uses a Hann window and radix-2 Cooley-Tukey FFT. Useful for verifying that notch filters and EQ are working as expected.

## Presets

Save and load your therapy configurations as named presets. Presets are stored as JSON files in your local app data directory (`%LOCALAPPDATA%`). All oscillator, filter, therapy, and volume settings are preserved. The preset format is backward-compatible — older presets will load with new features at their default (disabled) values.

## WAV Export

Export your current configuration as a 1-minute WAV file (16-bit PCM, stereo, lossless). The exported file is compatible with iPhones and any standard audio player. The export renders audio offline at the current sample rate, so what you hear in the app is exactly what gets saved. A native file dialog lets you choose where to save.

## Running Tests

```
cargo test
```

This runs unit tests for all filter types, the WAV export, oscillators, and therapy modules, as well as integration tests that verify audio device availability and playback.

## License

This project is provided as-is for personal tinnitus therapy use.

## Disclaimer

This software is not a medical device and is not intended to diagnose, treat, cure, or prevent any medical condition. Tinnitus therapy should be discussed with a qualified audiologist or healthcare provider. Use at your own discretion and at comfortable listening volumes to protect your hearing.
