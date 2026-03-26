use std::f32::consts::PI;

/// Waveform types available for tone generation.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Waveform {
    Sine,
    Square,
    Sawtooth,
    Triangle,
    WhiteNoise,
    PinkNoise,
    BrownNoise,
    BlueNoise,
    VioletNoise,
    GreyNoise,
    GreenNoise,
}

impl Waveform {
    pub const ALL: &'static [Waveform] = &[
        Waveform::Sine,
        Waveform::Square,
        Waveform::Sawtooth,
        Waveform::Triangle,
        Waveform::WhiteNoise,
        Waveform::PinkNoise,
        Waveform::BrownNoise,
        Waveform::BlueNoise,
        Waveform::VioletNoise,
        Waveform::GreyNoise,
        Waveform::GreenNoise,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Waveform::Sine => "Sine",
            Waveform::Square => "Square",
            Waveform::Sawtooth => "Sawtooth",
            Waveform::Triangle => "Triangle",
            Waveform::WhiteNoise => "White Noise",
            Waveform::PinkNoise => "Pink Noise",
            Waveform::BrownNoise => "Brown Noise",
            Waveform::BlueNoise => "Blue Noise",
            Waveform::VioletNoise => "Violet Noise",
            Waveform::GreyNoise => "Grey Noise",
            Waveform::GreenNoise => "Green Noise",
        }
    }
}

/// A single oscillator that generates audio samples for a given waveform and frequency.
#[derive(Clone)]
pub struct Oscillator {
    pub waveform: Waveform,
    pub frequency: f32,
    pub volume: f32,    // 0.0 - 1.0
    pub enabled: bool,
    pub pan: f32,       // -1.0 (left) to 1.0 (right), 0.0 = center
    phase: f32,
    // Pink noise state (Voss-McCartney algorithm)
    pink_rows: [f32; 16],
    pink_running_sum: f32,
    pink_index: u32,
    rng_state: u32,
    // Brown noise state
    brown_state: f32,
    // Blue noise state
    prev_white: f32,
    // Violet noise state
    prev_blue: f32,
    // Grey noise IIR filter state
    grey_state: [f32; 4],
    // Green noise bandpass filter state
    green_state: [f32; 4],
}

impl Oscillator {
    pub fn new(waveform: Waveform, frequency: f32) -> Self {
        Self {
            waveform,
            frequency,
            volume: 0.3,
            enabled: true,
            pan: 0.0,
            phase: 0.0,
            pink_rows: [0.0; 16],
            pink_running_sum: 0.0,
            pink_index: 0,
            rng_state: 12345,
            brown_state: 0.0,
            prev_white: 0.0,
            prev_blue: 0.0,
            grey_state: [0.0; 4],
            green_state: [0.0; 4],
        }
    }

    /// Generate the next sample at the given sample rate.
    /// Returns a mono sample in [-1.0, 1.0].
    pub fn next_sample(&mut self, sample_rate: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        let sample = match self.waveform {
            Waveform::Sine => (2.0 * PI * self.phase).sin(),
            Waveform::Square => {
                if self.phase < 0.5 { 1.0 } else { -1.0 }
            }
            Waveform::Sawtooth => 2.0 * self.phase - 1.0,
            Waveform::Triangle => {
                if self.phase < 0.5 {
                    4.0 * self.phase - 1.0
                } else {
                    3.0 - 4.0 * self.phase
                }
            }
            Waveform::WhiteNoise => self.white_noise(),
            Waveform::PinkNoise => self.pink_noise(),
            Waveform::BrownNoise => self.brown_noise(),
            Waveform::BlueNoise => self.blue_noise(),
            Waveform::VioletNoise => self.violet_noise(),
            Waveform::GreyNoise => self.grey_noise(),
            Waveform::GreenNoise => self.green_noise(),
        };

        // Advance phase
        self.phase += self.frequency / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample * self.volume
    }

    /// Get stereo samples (left, right) applying pan.
    pub fn next_stereo_sample(&mut self, sample_rate: f32) -> (f32, f32) {
        let mono = self.next_sample(sample_rate);
        let left_gain = ((1.0 - self.pan) / 2.0).sqrt();
        let right_gain = ((1.0 + self.pan) / 2.0).sqrt();
        (mono * left_gain, mono * right_gain)
    }

    fn xorshift(&mut self) -> u32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        self.rng_state
    }

    fn white_noise(&mut self) -> f32 {
        let val = self.xorshift();
        (val as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn pink_noise(&mut self) -> f32 {
        let white = self.white_noise();
        self.pink_index += 1;
        let mut changed_bits = self.pink_index ^ (self.pink_index - 1);
        let mut row_index = 0;

        while changed_bits > 0 && row_index < 16 {
            if changed_bits & 1 != 0 {
                self.pink_running_sum -= self.pink_rows[row_index];
                let new_val = white * 0.5;
                self.pink_running_sum += new_val;
                self.pink_rows[row_index] = new_val;
            }
            changed_bits >>= 1;
            row_index += 1;
        }

        (self.pink_running_sum + white) / 5.0 // Normalize
    }

    fn brown_noise(&mut self) -> f32 {
        let white = self.white_noise();
        // Integrate white noise with leaky integrator
        self.brown_state += white * 0.02;
        self.brown_state *= 0.998;
        // Clamp and normalize
        self.brown_state.clamp(-8.0, 8.0) / 8.0
    }

    fn blue_noise(&mut self) -> f32 {
        let white = self.white_noise();
        let blue = white - self.prev_white;
        self.prev_white = white;
        blue.clamp(-1.0, 1.0)
    }

    fn violet_noise(&mut self) -> f32 {
        let white = self.white_noise();
        let blue = white - self.prev_white;
        let violet = blue - self.prev_blue;
        self.prev_blue = blue;
        violet.clamp(-1.0, 1.0)
    }

    fn grey_noise(&mut self) -> f32 {
        let white = self.white_noise();

        // Simple 2-pole shelf approximation: boost lows and highs, cut mids around 3kHz
        // This is a simplified IIR filter approach
        // state[0] and state[1] are low shelf, state[2] and state[3] are high shelf

        // Low shelf (boost lows)
        let low_coef = 0.3;
        self.grey_state[0] = white * low_coef + self.grey_state[0] * (1.0 - low_coef);
        self.grey_state[1] = self.grey_state[0] * 1.5 + self.grey_state[1] * 0.5;

        // High shelf (boost highs)
        let high_coef = 0.3;
        self.grey_state[2] = white * high_coef + self.grey_state[2] * (1.0 - high_coef);
        self.grey_state[3] = self.grey_state[2] * 1.3 + self.grey_state[3] * 0.7;

        // Mid-cut (approximate by reducing mid-frequency response)
        let filtered = (self.grey_state[1] + self.grey_state[3]) * 0.5 - white * 0.2;
        filtered.clamp(-1.0, 1.0)
    }

    fn green_noise(&mut self) -> f32 {
        let white = self.white_noise();

        // Simple state-variable bandpass filter approach for ~500-2000Hz range
        // Using simplified second-order sections

        let _low_center = 0.15;  // Normalized frequency for lower cutoff
        let _high_center = 0.45; // Normalized frequency for upper cutoff

        // First section: shelving for passband
        self.green_state[0] = white * 0.2 + self.green_state[0] * 0.8;
        let low_passed = self.green_state[0];

        // Second section: shelf for mid-range boost
        self.green_state[1] = low_passed * 0.25 + self.green_state[1] * 0.75;

        // High-pass characteristic
        self.green_state[2] = white * 0.1 + self.green_state[2] * 0.9;
        let high_contrib = white - self.green_state[2];

        // Combine: focus on middle frequencies
        self.green_state[3] = self.green_state[1] * 0.7 + high_contrib * 0.3;
        self.green_state[3].clamp(-1.0, 1.0)
    }

    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 44100.0;

    #[test]
    fn sine_oscillator_output_range() {
        let mut osc = Oscillator::new(Waveform::Sine, 440.0);
        osc.volume = 1.0;
        for _ in 0..44100 {
            let s = osc.next_sample(SAMPLE_RATE);
            assert!(s >= -1.0 && s <= 1.0, "Sample out of range: {}", s);
        }
    }

    #[test]
    fn square_wave_only_two_values() {
        let mut osc = Oscillator::new(Waveform::Square, 100.0);
        osc.volume = 1.0;
        for _ in 0..44100 {
            let s = osc.next_sample(SAMPLE_RATE);
            assert!(
                (s - 1.0).abs() < 0.001 || (s + 1.0).abs() < 0.001,
                "Square wave sample should be +/-1.0, got {}",
                s
            );
        }
    }

    #[test]
    fn disabled_oscillator_is_silent() {
        let mut osc = Oscillator::new(Waveform::Sine, 440.0);
        osc.enabled = false;
        for _ in 0..1000 {
            assert_eq!(osc.next_sample(SAMPLE_RATE), 0.0);
        }
    }

    #[test]
    fn volume_scales_output() {
        let mut loud = Oscillator::new(Waveform::Sine, 440.0);
        loud.volume = 1.0;
        let mut quiet = Oscillator::new(Waveform::Sine, 440.0);
        quiet.volume = 0.1;

        // Generate a batch and compare RMS
        let loud_rms: f32 = (0..4410)
            .map(|_| { let s = loud.next_sample(SAMPLE_RATE); s * s })
            .sum::<f32>() / 4410.0;
        let quiet_rms: f32 = (0..4410)
            .map(|_| { let s = quiet.next_sample(SAMPLE_RATE); s * s })
            .sum::<f32>() / 4410.0;

        assert!(loud_rms > quiet_rms * 50.0, "Loud should be much louder than quiet");
    }

    #[test]
    fn stereo_pan_left() {
        let mut osc = Oscillator::new(Waveform::Sine, 440.0);
        osc.volume = 1.0;
        osc.pan = -1.0; // Full left

        let mut left_sum = 0.0f32;
        let mut right_sum = 0.0f32;
        for _ in 0..4410 {
            let (l, r) = osc.next_stereo_sample(SAMPLE_RATE);
            left_sum += l.abs();
            right_sum += r.abs();
        }

        assert!(left_sum > 0.1, "Left channel should have signal");
        assert!(right_sum < 0.01, "Right channel should be near silent when panned full left");
    }

    #[test]
    fn white_noise_has_variance() {
        let mut osc = Oscillator::new(Waveform::WhiteNoise, 1000.0);
        osc.volume = 1.0;
        let samples: Vec<f32> = (0..4410).map(|_| osc.next_sample(SAMPLE_RATE)).collect();
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        let variance = samples.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / samples.len() as f32;
        assert!(variance > 0.01, "White noise should have significant variance, got {}", variance);
    }

    #[test]
    fn all_waveforms_produce_output() {
        for wf in Waveform::ALL {
            let mut osc = Oscillator::new(*wf, 440.0);
            osc.volume = 1.0;
            let mut any_nonzero = false;
            for _ in 0..4410 {
                if osc.next_sample(SAMPLE_RATE).abs() > 0.001 {
                    any_nonzero = true;
                    break;
                }
            }
            assert!(any_nonzero, "{:?} waveform produced no output", wf);
        }
    }
}
