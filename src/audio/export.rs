use std::io::Write;
use std::path::Path;

use super::engine::AudioState;
use super::filters::AudioFilter;
use super::oscillator::Oscillator;
use super::therapy::{
    AmplitudeModulator, BinauralBeat, FractalToneGenerator, FrequencySweep, NotchFilter,
    ResidualInhibition,
};

/// Snapshot the current AudioState into a fresh AudioState suitable for offline rendering.
/// This clones all configuration but creates fresh filter state and no visualization buffers.
fn snapshot_for_export(src: &AudioState) -> AudioState {
    // Create fresh oscillators with same settings
    let oscillators: Vec<Oscillator> = src
        .oscillators
        .iter()
        .map(|o| {
            let mut osc = Oscillator::new(o.waveform, o.frequency);
            osc.volume = o.volume;
            osc.enabled = o.enabled;
            osc.pan = o.pan;
            osc
        })
        .collect();

    // Sweep
    let mut sweep = FrequencySweep::new(src.sweep.start_freq, src.sweep.end_freq, src.sweep.duration_secs);
    sweep.mode = src.sweep.mode;
    sweep.volume = src.sweep.volume;
    sweep.enabled = src.sweep.enabled;
    sweep.looping = src.sweep.looping;

    // Binaural
    let mut binaural = BinauralBeat::new(src.binaural.base_freq, src.binaural.beat_freq);
    binaural.volume = src.binaural.volume;
    binaural.enabled = src.binaural.enabled;

    // Notch filters
    let notch_filters: Vec<NotchFilter> = src
        .notch_filters
        .iter()
        .map(|nf| {
            let mut n = NotchFilter::new(nf.center_freq, nf.bandwidth);
            n.enabled = nf.enabled;
            n.depth = nf.depth;
            n.compute_coefficients(src.sample_rate);
            n
        })
        .collect();

    // General audio filters
    let filters: Vec<AudioFilter> = src
        .filters
        .iter()
        .map(|af| {
            let mut f = AudioFilter::new(af.filter_type, af.frequency);
            f.q = af.q;
            f.gain_db = af.gain_db;
            f.enabled = af.enabled;
            f.mix = af.mix;
            f.compute_coefficients(src.sample_rate);
            f
        })
        .collect();

    // Amplitude modulator
    let mut amp_mod = AmplitudeModulator::new(src.amp_mod.rate, src.amp_mod.depth);
    amp_mod.enabled = src.amp_mod.enabled;

    // Residual inhibition
    let mut ri = ResidualInhibition::new(
        src.residual_inhibition.burst_freq,
        src.residual_inhibition.burst_duration,
        src.residual_inhibition.silence_duration,
    );
    ri.burst_volume = src.residual_inhibition.burst_volume;
    ri.enabled = src.residual_inhibition.enabled;

    // Fractal tones
    let mut ft = FractalToneGenerator::new(src.fractal_tones.base_freq, src.fractal_tones.tempo);
    ft.volume = src.fractal_tones.volume;
    ft.enabled = src.fractal_tones.enabled;

    let mut state = AudioState::new();
    state.oscillators = oscillators;
    state.sweep = sweep;
    state.binaural = binaural;
    state.notch_filters = notch_filters;
    state.filters = filters;
    state.amp_mod = amp_mod;
    state.residual_inhibition = ri;
    state.fractal_tones = ft;
    state.master_volume = src.master_volume;
    state.sample_rate = src.sample_rate;

    state
}

/// Export the current audio configuration as a WAV file.
///
/// Renders `duration_secs` of audio offline at the current sample rate.
/// Output: 16-bit PCM stereo WAV (lossless, iPhone-compatible).
pub fn export_wav(
    src_state: &AudioState,
    path: &Path,
    duration_secs: f32,
) -> Result<(), String> {
    let sample_rate = src_state.sample_rate;
    let total_frames = (sample_rate * duration_secs) as usize;
    let channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate as u32 * channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = channels * (bits_per_sample / 8);
    let data_size = total_frames as u32 * block_align as u32;

    // Snapshot the state for offline rendering
    let mut render_state = snapshot_for_export(src_state);

    // Render all frames
    let mut pcm_data: Vec<u8> = Vec::with_capacity(data_size as usize);

    for _ in 0..total_frames {
        let (left, right) = render_state.render_frame();

        // Convert f32 [-1.0, 1.0] → i16
        let l_i16 = (left.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        let r_i16 = (right.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;

        pcm_data.extend_from_slice(&l_i16.to_le_bytes());
        pcm_data.extend_from_slice(&r_i16.to_le_bytes());
    }

    // Write WAV file
    let file_size = 36 + data_size; // RIFF chunk size (file size - 8)

    let mut file = std::fs::File::create(path)
        .map_err(|e| format!("Failed to create WAV file: {}", e))?;

    // RIFF header
    file.write_all(b"RIFF")
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&file_size.to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(b"WAVE")
        .map_err(|e| format!("Write error: {}", e))?;

    // fmt sub-chunk
    file.write_all(b"fmt ")
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&16u32.to_le_bytes()) // sub-chunk size (16 for PCM)
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&1u16.to_le_bytes()) // audio format (1 = PCM)
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&channels.to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&(sample_rate as u32).to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&byte_rate.to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&block_align.to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&bits_per_sample.to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;

    // data sub-chunk
    file.write_all(b"data")
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&data_size.to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.write_all(&pcm_data)
        .map_err(|e| format!("Write error: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::oscillator::Waveform;

    #[test]
    fn test_export_creates_valid_wav() {
        let mut state = AudioState::new();
        state.sample_rate = 44100.0;
        state.oscillators.push(Oscillator::new(Waveform::Sine, 440.0));
        state.oscillators[0].enabled = true;
        state.master_volume = 0.5;

        let tmp = std::env::temp_dir().join("test_export.wav");
        let result = export_wav(&state, &tmp, 1.0); // 1 second
        assert!(result.is_ok(), "Export failed: {:?}", result.err());

        // Read back and verify WAV header
        let data = std::fs::read(&tmp).unwrap();
        assert!(data.len() > 44, "WAV file too small");
        assert_eq!(&data[0..4], b"RIFF");
        assert_eq!(&data[8..12], b"WAVE");
        assert_eq!(&data[12..16], b"fmt ");
        assert_eq!(&data[36..40], b"data");

        // Verify expected size: 44100 frames * 2 channels * 2 bytes = 176400 + 44 header
        let expected_data_size = 44100u32 * 2 * 2;
        let actual_data_size = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        assert_eq!(actual_data_size, expected_data_size);

        // Verify PCM data is not all zeros (we have an enabled oscillator)
        let pcm_slice = &data[44..];
        let has_nonzero = pcm_slice.iter().any(|&b| b != 0);
        assert!(has_nonzero, "PCM data should contain non-zero samples");

        // Clean up
        std::fs::remove_file(&tmp).ok();
    }
}
