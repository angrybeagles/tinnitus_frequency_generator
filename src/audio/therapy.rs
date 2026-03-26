use std::f32::consts::PI;

/// Frequency sweep mode: linear or logarithmic.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SweepMode {
    Linear,
    Logarithmic,
}

/// A frequency sweep generator that moves between two frequencies over time.
#[derive(Clone)]
pub struct FrequencySweep {
    pub start_freq: f32,
    pub end_freq: f32,
    pub duration_secs: f32,
    pub mode: SweepMode,
    pub volume: f32,
    pub enabled: bool,
    pub looping: bool,
    phase: f32,
    elapsed: f32,
}

impl FrequencySweep {
    pub fn new(start_freq: f32, end_freq: f32, duration_secs: f32) -> Self {
        Self {
            start_freq,
            end_freq,
            duration_secs,
            mode: SweepMode::Linear,
            volume: 0.3,
            enabled: false,
            looping: true,
            phase: 0.0,
            elapsed: 0.0,
        }
    }

    pub fn current_frequency(&self) -> f32 {
        let t = if self.duration_secs > 0.0 {
            (self.elapsed / self.duration_secs).min(1.0)
        } else {
            0.0
        };

        match self.mode {
            SweepMode::Linear => self.start_freq + (self.end_freq - self.start_freq) * t,
            SweepMode::Logarithmic => {
                let log_start = self.start_freq.ln();
                let log_end = self.end_freq.ln();
                (log_start + (log_end - log_start) * t).exp()
            }
        }
    }

    pub fn next_sample(&mut self, sample_rate: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        let freq = self.current_frequency();
        let sample = (2.0 * PI * self.phase).sin();

        self.phase += freq / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        self.elapsed += 1.0 / sample_rate;
        if self.elapsed >= self.duration_secs {
            if self.looping {
                self.elapsed = 0.0;
            } else {
                self.enabled = false;
            }
        }

        sample * self.volume
    }

    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.phase = 0.0;
    }

    pub fn progress(&self) -> f32 {
        if self.duration_secs > 0.0 {
            (self.elapsed / self.duration_secs).min(1.0)
        } else {
            0.0
        }
    }
}

/// Binaural beat generator: plays slightly different frequencies in each ear
/// to create a perceived beat frequency.
#[derive(Clone)]
pub struct BinauralBeat {
    pub base_freq: f32,
    pub beat_freq: f32,  // Difference between left and right
    pub volume: f32,
    pub enabled: bool,
    phase_left: f32,
    phase_right: f32,
}

impl BinauralBeat {
    pub fn new(base_freq: f32, beat_freq: f32) -> Self {
        Self {
            base_freq,
            beat_freq,
            volume: 0.3,
            enabled: false,
            phase_left: 0.0,
            phase_right: 0.0,
        }
    }

    /// Returns (left_sample, right_sample).
    pub fn next_stereo_sample(&mut self, sample_rate: f32) -> (f32, f32) {
        if !self.enabled {
            return (0.0, 0.0);
        }

        let freq_left = self.base_freq - self.beat_freq / 2.0;
        let freq_right = self.base_freq + self.beat_freq / 2.0;

        let left = (2.0 * PI * self.phase_left).sin() * self.volume;
        let right = (2.0 * PI * self.phase_right).sin() * self.volume;

        self.phase_left += freq_left / sample_rate;
        if self.phase_left >= 1.0 {
            self.phase_left -= 1.0;
        }

        self.phase_right += freq_right / sample_rate;
        if self.phase_right >= 1.0 {
            self.phase_right -= 1.0;
        }

        (left, right)
    }

    pub fn left_freq(&self) -> f32 {
        self.base_freq - self.beat_freq / 2.0
    }

    pub fn right_freq(&self) -> f32 {
        self.base_freq + self.beat_freq / 2.0
    }

    pub fn reset(&mut self) {
        self.phase_left = 0.0;
        self.phase_right = 0.0;
    }
}

/// Notched audio filter: attenuates frequencies around the tinnitus frequency.
/// Uses a simple notch filter approach.
#[derive(Clone)]
pub struct NotchFilter {
    pub center_freq: f32,
    pub bandwidth: f32,  // Width of the notch in Hz
    pub enabled: bool,
    pub depth: f32,      // 0.0 = full notch, 1.0 = no notch
    // Biquad filter coefficients
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    // Filter state
    x1: f32, x2: f32,
    y1: f32, y2: f32,
    x1_r: f32, x2_r: f32,
    y1_r: f32, y2_r: f32,
    last_sample_rate: f32,
}

impl NotchFilter {
    pub fn new(center_freq: f32, bandwidth: f32) -> Self {
        let mut filter = Self {
            center_freq,
            bandwidth,
            enabled: false,
            depth: 0.0,
            b0: 1.0, b1: 0.0, b2: 0.0,
            a1: 0.0, a2: 0.0,
            x1: 0.0, x2: 0.0,
            y1: 0.0, y2: 0.0,
            x1_r: 0.0, x2_r: 0.0,
            y1_r: 0.0, y2_r: 0.0,
            last_sample_rate: 0.0,
        };
        filter.compute_coefficients(44100.0);
        filter
    }

    pub fn compute_coefficients(&mut self, sample_rate: f32) {
        self.last_sample_rate = sample_rate;
        let w0 = 2.0 * PI * self.center_freq / sample_rate;
        let q = self.center_freq / self.bandwidth.max(1.0);
        let alpha = w0.sin() / (2.0 * q);

        let b0 = 1.0;
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    /// Apply notch filter to a mono sample.
    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
                   - self.a1 * self.y1 - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        // Blend between filtered and original based on depth
        input * self.depth + output * (1.0 - self.depth)
    }

    /// Apply notch filter to the right channel (independent state).
    pub fn process_right(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        let output = self.b0 * input + self.b1 * self.x1_r + self.b2 * self.x2_r
                   - self.a1 * self.y1_r - self.a2 * self.y2_r;

        self.x2_r = self.x1_r;
        self.x1_r = input;
        self.y2_r = self.y1_r;
        self.y1_r = output;

        input * self.depth + output * (1.0 - self.depth)
    }

    pub fn update_if_needed(&mut self, sample_rate: f32) {
        if (self.last_sample_rate - sample_rate).abs() > 1.0 {
            self.compute_coefficients(sample_rate);
        }
    }

    pub fn reset_state(&mut self) {
        self.x1 = 0.0; self.x2 = 0.0;
        self.y1 = 0.0; self.y2 = 0.0;
        self.x1_r = 0.0; self.x2_r = 0.0;
        self.y1_r = 0.0; self.y2_r = 0.0;
    }
}

/// Amplitude modulator: varies the volume of a sample using a modulation waveform.
#[derive(Clone)]
pub struct AmplitudeModulator {
    pub rate: f32,      // Modulation rate in Hz (e.g., 1-10)
    pub depth: f32,     // 0.0 = no modulation, 1.0 = full 0-to-1 modulation
    pub enabled: bool,
    phase: f32,
}

impl AmplitudeModulator {
    pub fn new(rate: f32, depth: f32) -> Self {
        Self {
            rate,
            depth,
            enabled: true,
            phase: 0.0,
        }
    }

    /// Compute the current envelope value and advance the phase.
    /// Returns a multiplier in 0.0..=1.0.
    pub fn next_envelope(&mut self, sample_rate: f32) -> f32 {
        if !self.enabled {
            return 1.0;
        }

        let modulation = (2.0 * PI * self.phase).sin();
        let envelope = 1.0 - self.depth * 0.5 * (1.0 - modulation);

        self.phase += self.rate / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        envelope
    }

    /// Process a sample with amplitude modulation.
    /// Multiplies sample by (1.0 - depth * 0.5 * (1.0 - sin(2*PI*phase))).
    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        sample * self.next_envelope(sample_rate)
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }
}

/// Residual inhibition generator: produces bursts of tone followed by silence.
/// Used to temporarily mask tinnitus.
#[derive(Clone)]
pub struct ResidualInhibition {
    pub burst_freq: f32,        // Frequency of the noise burst tone
    pub burst_duration: f32,    // Duration of burst in seconds (e.g., 0.5)
    pub silence_duration: f32,  // Duration of silence in seconds (e.g., 1.0)
    pub burst_volume: f32,
    pub enabled: bool,
    phase: f32,
    elapsed: f32,
    in_burst: bool,
}

impl ResidualInhibition {
    pub fn new(burst_freq: f32, burst_duration: f32, silence_duration: f32) -> Self {
        Self {
            burst_freq,
            burst_duration,
            silence_duration,
            burst_volume: 0.5,
            enabled: true,
            phase: 0.0,
            elapsed: 0.0,
            in_burst: true,
        }
    }

    /// Generate the next sample of the burst/silence cycle.
    pub fn next_sample(&mut self, sample_rate: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        let cycle_duration = self.burst_duration + self.silence_duration;
        let cycle_pos = self.elapsed % cycle_duration;

        let in_burst = cycle_pos < self.burst_duration;
        self.in_burst = in_burst;

        let sample = if in_burst {
            let sine = (2.0 * PI * self.phase).sin();
            self.phase += self.burst_freq / sample_rate;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            sine * self.burst_volume
        } else {
            0.0
        };

        self.elapsed += 1.0 / sample_rate;

        sample
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.elapsed = 0.0;
        self.in_burst = true;
    }

    pub fn is_in_burst(&self) -> bool {
        self.in_burst
    }

    /// Progress through the current burst/silence cycle (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        let cycle_duration = self.burst_duration + self.silence_duration;
        if cycle_duration > 0.0 {
            (self.elapsed % cycle_duration) / cycle_duration
        } else {
            0.0
        }
    }
}

/// Fractal tone generator: produces melodic tones using pentatonic scales
/// with a random walk through note space.
#[derive(Clone)]
pub struct FractalToneGenerator {
    pub base_freq: f32,     // Base frequency (e.g., 440.0)
    pub tempo: f32,         // Notes per second (e.g., 2.0)
    pub volume: f32,
    pub enabled: bool,
    phase: f32,
    current_freq: f32,
    note_elapsed: f32,
    note_duration: f32,
    rng_state: u32,
}

impl FractalToneGenerator {
    /// Pentatonic scale ratios: 1.0, 9/8, 5/4, 3/2, 5/3
    const PENTATONIC_RATIOS: [f32; 5] = [1.0, 9.0 / 8.0, 5.0 / 4.0, 3.0 / 2.0, 5.0 / 3.0];

    pub fn new(base_freq: f32, tempo: f32) -> Self {
        let note_duration = 1.0 / tempo;
        Self {
            base_freq,
            tempo,
            volume: 0.3,
            enabled: true,
            phase: 0.0,
            current_freq: base_freq,
            note_elapsed: 0.0,
            note_duration,
            rng_state: 12345,
        }
    }

    /// Simple 32-bit LCG random number generator
    fn next_random(&mut self) -> u32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.rng_state
    }

    /// Pick a random note via a random walk through the pentatonic scale
    fn pick_next_note(&mut self) {
        // Find the current note index in the pentatonic scale (modulo octave)
        let current_ratio = self.current_freq / self.base_freq;
        let octave = current_ratio.log2().floor();
        let ratio_in_octave = current_ratio / (2.0_f32).powf(octave);

        // Find closest ratio index
        let mut closest_idx = 0;
        let mut min_dist = (Self::PENTATONIC_RATIOS[0] - ratio_in_octave).abs();
        for (i, &ratio) in Self::PENTATONIC_RATIOS.iter().enumerate() {
            let dist = (ratio - ratio_in_octave).abs();
            if dist < min_dist {
                min_dist = dist;
                closest_idx = i;
            }
        }

        // Random walk: step up/down 1-2 scale degrees
        let rand_val = self.next_random() % 100;
        let step = if rand_val < 50 {
            -(1 + (self.next_random() % 2) as i32)
        } else {
            1 + (self.next_random() % 2) as i32
        };

        let new_idx = ((closest_idx as i32 + step) % 5) as usize;
        let mut new_ratio = Self::PENTATONIC_RATIOS[new_idx];

        // Occasionally jump octaves
        if self.next_random() % 10 < 2 {
            let octave_jump = if self.next_random() % 2 == 0 { -1 } else { 1 };
            new_ratio *= (2.0_f32).powf(octave_jump as f32);
        }

        self.current_freq = self.base_freq * new_ratio;

        // Vary note duration slightly (0.8x to 1.2x)
        let duration_variance = 0.8 + (self.next_random() % 100) as f32 / 500.0;
        self.note_duration = (1.0 / self.tempo) * duration_variance;
    }

    /// Generate the next sample of the fractal tone with envelope.
    pub fn next_sample(&mut self, sample_rate: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        // Check if we need to pick a new note
        if self.note_elapsed >= self.note_duration {
            self.pick_next_note();
            self.note_elapsed = 0.0;
        }

        // Calculate soft amplitude envelope (fade in/out at note boundaries)
        let envelope = if self.note_duration > 0.0 {
            let progress = self.note_elapsed / self.note_duration;
            let fade_time = 0.1; // 10% of note duration for fade in/out
            let fade_threshold = fade_time / self.note_duration;

            if progress < fade_threshold {
                // Fade in
                progress / fade_threshold
            } else if progress > (1.0 - fade_threshold) {
                // Fade out
                (1.0 - progress) / fade_threshold
            } else {
                // Full volume in the middle
                1.0
            }
        } else {
            1.0
        };

        let sine = (2.0 * PI * self.phase).sin();
        let sample = sine * self.volume * envelope;

        self.phase += self.current_freq / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        self.note_elapsed += 1.0 / sample_rate;

        sample
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.note_elapsed = 0.0;
        self.current_freq = self.base_freq;
    }
}
