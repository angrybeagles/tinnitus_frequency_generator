//! Integration tests for the tinnitus frequency generator audio engine.

use std::time::Duration;

/// Test that we can enumerate at least one audio output device.
#[test]
fn test_audio_device_available() {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("No audio output device found — cannot run audio tests");

    let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    println!("Audio device: {}", name);

    let config = device
        .default_output_config()
        .expect("Failed to get default output config");
    println!(
        "Sample rate: {} Hz, Channels: {}, Format: {:?}",
        config.sample_rate().0,
        config.channels(),
        config.sample_format()
    );
}

/// Test that the AudioEngine initializes successfully and can play/pause.
#[test]
fn test_engine_init_and_play_pause() {
    use tinnitus_freq_generator::audio::engine::AudioEngine;

    let engine = AudioEngine::new().expect("Failed to create AudioEngine");
    println!("Engine created on device: {}", engine.device_name);

    {
        let state = engine.state.lock().unwrap();
        assert!(!state.playing, "Engine should start paused");
    }

    {
        let mut state = engine.state.lock().unwrap();
        state.playing = true;
    }

    std::thread::sleep(Duration::from_millis(100));

    {
        let mut state = engine.state.lock().unwrap();
        state.playing = false;
    }
}

/// Open an audio device and play a 440 Hz sine wave for 2 seconds.
#[test]
fn test_play_sine_tone() {
    use tinnitus_freq_generator::audio::engine::AudioEngine;
    use tinnitus_freq_generator::audio::oscillator::{Oscillator, Waveform};

    let engine = AudioEngine::new().expect("Failed to create AudioEngine");

    {
        let mut state = engine.state.lock().unwrap();
        let mut osc = Oscillator::new(Waveform::Sine, 440.0);
        osc.volume = 0.3;
        osc.enabled = true;
        state.oscillators.push(osc);
        state.master_volume = 0.5;
        state.playing = true;
    }

    println!("Playing 440 Hz sine tone for 2 seconds...");
    std::thread::sleep(Duration::from_secs(2));

    {
        let mut state = engine.state.lock().unwrap();
        state.playing = false;
        assert_eq!(state.oscillators.len(), 1);
        assert_eq!(state.oscillators[0].frequency, 440.0);
    }

    println!("Sine tone test complete.");
}

/// Test that multiple oscillators mix together without panicking.
#[test]
fn test_multi_oscillator_mix() {
    use tinnitus_freq_generator::audio::engine::AudioEngine;
    use tinnitus_freq_generator::audio::oscillator::{Oscillator, Waveform};

    let engine = AudioEngine::new().expect("Failed to create AudioEngine");

    {
        let mut state = engine.state.lock().unwrap();
        for &(wf, freq) in &[
            (Waveform::Sine, 250.0),
            (Waveform::Square, 500.0),
            (Waveform::Sawtooth, 1000.0),
            (Waveform::Triangle, 2000.0),
        ] {
            let mut osc = Oscillator::new(wf, freq);
            osc.volume = 0.1;
            osc.enabled = true;
            state.oscillators.push(osc);
        }
        state.master_volume = 0.4;
        state.playing = true;
    }

    println!("Playing 4 mixed waveforms for 1 second...");
    std::thread::sleep(Duration::from_secs(1));

    {
        let mut state = engine.state.lock().unwrap();
        state.playing = false;
        assert_eq!(state.oscillators.len(), 4);
    }
}

/// Test binaural beats produce output without errors.
#[test]
fn test_binaural_beat_playback() {
    use tinnitus_freq_generator::audio::engine::AudioEngine;

    let engine = AudioEngine::new().expect("Failed to create AudioEngine");

    {
        let mut state = engine.state.lock().unwrap();
        state.binaural.base_freq = 300.0;
        state.binaural.beat_freq = 10.0;
        state.binaural.volume = 0.3;
        state.binaural.enabled = true;
        state.master_volume = 0.5;
        state.playing = true;
    }

    println!("Playing binaural beat (300 Hz base, 10 Hz beat) for 1 second...");
    std::thread::sleep(Duration::from_secs(1));

    {
        let mut state = engine.state.lock().unwrap();
        state.playing = false;
        assert!((state.binaural.left_freq() - 295.0).abs() < 0.01);
        assert!((state.binaural.right_freq() - 305.0).abs() < 0.01);
    }
}

/// Test frequency sweep runs without errors and progresses.
#[test]
fn test_frequency_sweep() {
    use tinnitus_freq_generator::audio::engine::AudioEngine;

    let engine = AudioEngine::new().expect("Failed to create AudioEngine");

    {
        let mut state = engine.state.lock().unwrap();
        state.sweep.start_freq = 200.0;
        state.sweep.end_freq = 2000.0;
        state.sweep.duration_secs = 3.0;
        state.sweep.volume = 0.25;
        state.sweep.looping = false;
        state.sweep.enabled = true;
        state.master_volume = 0.5;
        state.playing = true;
    }

    println!("Playing frequency sweep 200-2000 Hz over 3 seconds...");
    std::thread::sleep(Duration::from_millis(1500));

    {
        let state = engine.state.lock().unwrap();
        let progress = state.sweep.progress();
        println!("Sweep progress at 1.5s: {:.1}%", progress * 100.0);
        assert!(
            progress > 0.3 && progress < 0.7,
            "Sweep should be roughly halfway, got {}",
            progress
        );
    }

    std::thread::sleep(Duration::from_millis(2000));

    {
        let mut state = engine.state.lock().unwrap();
        state.playing = false;
    }
}

/// Test multiple notch filters applied in series.
#[test]
fn test_multiple_notch_filters() {
    use tinnitus_freq_generator::audio::engine::AudioEngine;
    use tinnitus_freq_generator::audio::oscillator::{Oscillator, Waveform};
    use tinnitus_freq_generator::audio::therapy::NotchFilter;

    let engine = AudioEngine::new().expect("Failed to create AudioEngine");

    {
        let mut state = engine.state.lock().unwrap();
        let sr = state.sample_rate;

        // White noise source (broadband)
        let mut osc = Oscillator::new(Waveform::WhiteNoise, 1000.0);
        osc.volume = 0.3;
        osc.enabled = true;
        state.oscillators.push(osc);

        // Add two notch filters at different frequencies
        let mut nf1 = NotchFilter::new(2000.0, 400.0);
        nf1.enabled = true;
        nf1.compute_coefficients(sr);
        state.notch_filters.push(nf1);

        let mut nf2 = NotchFilter::new(6000.0, 600.0);
        nf2.enabled = true;
        nf2.compute_coefficients(sr);
        state.notch_filters.push(nf2);

        state.master_volume = 0.4;
        state.playing = true;
    }

    println!("Playing white noise with dual notch filters (2kHz + 6kHz) for 1 second...");
    std::thread::sleep(Duration::from_secs(1));

    {
        let mut state = engine.state.lock().unwrap();
        state.playing = false;
        assert_eq!(state.notch_filters.len(), 2);
        assert_eq!(state.notch_filters[0].center_freq, 2000.0);
        assert_eq!(state.notch_filters[1].center_freq, 6000.0);
    }

    println!("Dual notch filter test complete.");
}

/// Test that the waveform buffer captures data while playing.
#[test]
fn test_waveform_buffer_fills() {
    use tinnitus_freq_generator::audio::engine::AudioEngine;
    use tinnitus_freq_generator::audio::oscillator::{Oscillator, Waveform};

    let engine = AudioEngine::new().expect("Failed to create AudioEngine");

    {
        let mut state = engine.state.lock().unwrap();
        let mut osc = Oscillator::new(Waveform::Sine, 440.0);
        osc.volume = 0.5;
        osc.enabled = true;
        state.oscillators.push(osc);
        state.master_volume = 0.5;
        state.playing = true;
    }

    // Let it fill for a bit
    std::thread::sleep(Duration::from_millis(500));

    {
        let state = engine.state.lock().unwrap();
        let (left, right) = state.waveform_buf.read_recent(512);

        // After 500ms of a 440Hz sine at volume, there should be non-zero data
        let left_energy: f32 = left.iter().map(|s| s * s).sum();
        let right_energy: f32 = right.iter().map(|s| s * s).sum();

        println!(
            "Waveform buffer energy — Left: {:.4}, Right: {:.4}",
            left_energy, right_energy
        );
        assert!(left_energy > 0.1, "Left channel buffer should have signal energy");
        assert!(right_energy > 0.1, "Right channel buffer should have signal energy");
    }

    {
        let mut state = engine.state.lock().unwrap();
        state.playing = false;
    }

    println!("Waveform buffer test complete.");
}
