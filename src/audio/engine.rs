use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

use super::filters::AudioFilter;
use super::oscillator::Oscillator;
use super::therapy::{
    AmplitudeModulator, BinauralBeat, FractalToneGenerator, FrequencySweep, NotchFilter,
    ResidualInhibition,
};

/// Fixed-size ring buffer for waveform visualization.
/// The audio thread writes into it; the GUI reads snapshots.
pub struct WaveformBuffer {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub write_pos: usize,
    pub capacity: usize,
}

impl WaveformBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            left: vec![0.0; capacity],
            right: vec![0.0; capacity],
            write_pos: 0,
            capacity,
        }
    }

    /// Push a stereo frame into the ring buffer.
    pub fn push(&mut self, l: f32, r: f32) {
        self.left[self.write_pos] = l;
        self.right[self.write_pos] = r;
        self.write_pos = (self.write_pos + 1) % self.capacity;
    }

    /// Read the most recent `count` samples in chronological order.
    pub fn read_recent(&self, count: usize) -> (Vec<f32>, Vec<f32>) {
        let count = count.min(self.capacity);
        let mut left = Vec::with_capacity(count);
        let mut right = Vec::with_capacity(count);
        let start = (self.write_pos + self.capacity - count) % self.capacity;
        for i in 0..count {
            let idx = (start + i) % self.capacity;
            left.push(self.left[idx]);
            right.push(self.right[idx]);
        }
        (left, right)
    }
}

/// Shared audio state that the GUI reads/writes and the audio thread consumes.
pub struct AudioState {
    pub oscillators: Vec<Oscillator>,
    pub sweep: FrequencySweep,
    pub binaural: BinauralBeat,
    pub notch_filters: Vec<NotchFilter>,
    pub filters: Vec<AudioFilter>,
    pub amp_mod: AmplitudeModulator,
    pub residual_inhibition: ResidualInhibition,
    pub fractal_tones: FractalToneGenerator,
    pub master_volume: f32,
    pub playing: bool,
    pub sample_rate: f32,
    pub waveform_buf: WaveformBuffer,
    /// Decimation counter — only write every N-th sample to the viz buffer
    /// to keep the scope at a readable time scale.
    waveform_decimation: u32,
    waveform_decimation_counter: u32,
    /// Undecimated mono ring buffer for spectrum analysis (full sample rate).
    pub spectrum_buf: Vec<f32>,
    pub spectrum_write_pos: usize,
    pub spectrum_buf_capacity: usize,
}

impl AudioState {
    pub fn new() -> Self {
        let spectrum_capacity = 8192; // enough for good FFT resolution
        let mut amp_mod = AmplitudeModulator::new(4.0, 0.5);
        amp_mod.enabled = false;
        let mut ri = ResidualInhibition::new(6000.0, 0.5, 1.0);
        ri.enabled = false;
        let mut ft = FractalToneGenerator::new(440.0, 2.0);
        ft.enabled = false;

        Self {
            oscillators: Vec::new(),
            sweep: FrequencySweep::new(200.0, 8000.0, 10.0),
            binaural: BinauralBeat::new(440.0, 10.0),
            notch_filters: Vec::new(),
            filters: Vec::new(),
            amp_mod,
            residual_inhibition: ri,
            fractal_tones: ft,
            master_volume: 0.5,
            playing: false,
            sample_rate: 44100.0,
            // 4096 samples visible in the scope at any time
            waveform_buf: WaveformBuffer::new(4096),
            waveform_decimation: 4,  // write every 4th sample (~11 kHz effective)
            waveform_decimation_counter: 0,
            spectrum_buf: vec![0.0; spectrum_capacity],
            spectrum_write_pos: 0,
            spectrum_buf_capacity: spectrum_capacity,
        }
    }

    /// Read the most recent `count` mono samples from the spectrum buffer.
    pub fn read_spectrum_samples(&self, count: usize) -> Vec<f32> {
        let count = count.min(self.spectrum_buf_capacity);
        let mut out = Vec::with_capacity(count);
        let start = (self.spectrum_write_pos + self.spectrum_buf_capacity - count)
            % self.spectrum_buf_capacity;
        for i in 0..count {
            let idx = (start + i) % self.spectrum_buf_capacity;
            out.push(self.spectrum_buf[idx]);
        }
        out
    }
}

/// The audio engine manages the cpal output stream and shared state.
pub struct AudioEngine {
    pub state: Arc<Mutex<AudioState>>,
    _stream: Option<Stream>,
    pub device_name: String,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let state = Arc::new(Mutex::new(AudioState::new()));

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No audio output device found")?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());

        let supported_config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get default output config: {}", e))?;

        let sample_rate = supported_config.sample_rate().0 as f32;
        let channels = supported_config.channels() as usize;

        // Update state with actual sample rate
        {
            let mut s = state.lock().unwrap();
            s.sample_rate = sample_rate;
            for nf in s.notch_filters.iter_mut() {
                nf.compute_coefficients(sample_rate);
            }
            for af in s.filters.iter_mut() {
                af.compute_coefficients(sample_rate);
            }
        }

        let config = StreamConfig {
            channels: supported_config.channels(),
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let state_clone = Arc::clone(&state);
        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = match supported_config.sample_format() {
            SampleFormat::F32 => {
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        fill_buffer_f32(data, &state_clone, channels, sample_rate);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                device.build_output_stream(
                    &config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        fill_buffer_i16(data, &state_clone, channels, sample_rate);
                    },
                    err_fn,
                    None,
                )
            }
            sample_format => {
                return Err(format!("Unsupported sample format: {:?}", sample_format));
            }
        }
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;

        Ok(Self {
            state,
            _stream: Some(stream),
            device_name,
        })
    }
}

impl AudioState {
    /// Render one stereo frame of audio from the current configuration.
    /// This is the core DSP chain, usable for both real-time playback and offline export.
    pub fn render_frame(&mut self) -> (f32, f32) {
        let sample_rate = self.sample_rate;
        let mut left = 0.0f32;
        let mut right = 0.0f32;

        // Mix all oscillators
        for osc in self.oscillators.iter_mut() {
            let (l, r) = osc.next_stereo_sample(sample_rate);
            left += l;
            right += r;
        }

        // Add frequency sweep (mono, centered)
        let sweep_sample = self.sweep.next_sample(sample_rate);
        left += sweep_sample;
        right += sweep_sample;

        // Add binaural beat (stereo)
        let (bl, br) = self.binaural.next_stereo_sample(sample_rate);
        left += bl;
        right += br;

        // Add residual inhibition burst (mono, centered)
        let ri_sample = self.residual_inhibition.next_sample(sample_rate);
        left += ri_sample;
        right += ri_sample;

        // Add fractal tones (mono, centered)
        let ft_sample = self.fractal_tones.next_sample(sample_rate);
        left += ft_sample;
        right += ft_sample;

        // Apply all notch filters in series
        for nf in self.notch_filters.iter_mut() {
            nf.update_if_needed(sample_rate);
            left = nf.process(left);
            right = nf.process_right(right);
        }

        // Apply general audio filters in series
        for af in self.filters.iter_mut() {
            af.update_if_needed(sample_rate);
            let (l, r) = af.process_stereo(left, right);
            left = l;
            right = r;
        }

        // Apply amplitude modulation (compute envelope once, apply to both channels)
        let am_env = self.amp_mod.next_envelope(sample_rate);
        left *= am_env;
        right *= am_env;

        // Apply master volume and soft-clip
        left *= self.master_volume;
        right *= self.master_volume;

        // Soft clipping (tanh) to prevent harsh distortion
        left = left.tanh();
        right = right.tanh();

        (left, right)
    }
}

fn generate_frame(state: &mut AudioState) -> (f32, f32) {
    if !state.playing {
        return (0.0, 0.0);
    }

    let (left, right) = state.render_frame();

    // Write to waveform visualization buffer (decimated)
    state.waveform_decimation_counter += 1;
    if state.waveform_decimation_counter >= state.waveform_decimation {
        state.waveform_decimation_counter = 0;
        state.waveform_buf.push(left, right);
    }

    // Write undecimated mono to spectrum buffer
    let mono = (left + right) * 0.5;
    let pos = state.spectrum_write_pos;
    state.spectrum_buf[pos] = mono;
    state.spectrum_write_pos = (pos + 1) % state.spectrum_buf_capacity;

    (left, right)
}

fn fill_buffer_f32(
    data: &mut [f32],
    state: &Arc<Mutex<AudioState>>,
    channels: usize,
    _sample_rate: f32,
) {
    let mut state = state.lock().unwrap();

    for frame in data.chunks_mut(channels) {
        let (left, right) = generate_frame(&mut state);
        if channels >= 2 {
            frame[0] = left;
            frame[1] = right;
            for ch in frame.iter_mut().skip(2) {
                *ch = 0.0;
            }
        } else {
            frame[0] = (left + right) * 0.5;
        }
    }
}

fn fill_buffer_i16(
    data: &mut [i16],
    state: &Arc<Mutex<AudioState>>,
    channels: usize,
    _sample_rate: f32,
) {
    let mut state = state.lock().unwrap();

    for frame in data.chunks_mut(channels) {
        let (left, right) = generate_frame(&mut state);
        let to_i16 = |v: f32| (v * i16::MAX as f32) as i16;
        if channels >= 2 {
            frame[0] = to_i16(left);
            frame[1] = to_i16(right);
            for ch in frame.iter_mut().skip(2) {
                *ch = 0;
            }
        } else {
            frame[0] = to_i16((left + right) * 0.5);
        }
    }
}
