use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Filter type enum supporting various audio filter topologies
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    PeakingEQ,
    LowShelf,
    HighShelf,
    AllPass,
    Comb,
}

impl FilterType {
    /// Get the human-readable name of this filter type
    pub fn name(&self) -> &'static str {
        match self {
            FilterType::LowPass => "LowPass",
            FilterType::HighPass => "HighPass",
            FilterType::BandPass => "BandPass",
            FilterType::Notch => "Notch",
            FilterType::PeakingEQ => "PeakingEQ",
            FilterType::LowShelf => "LowShelf",
            FilterType::HighShelf => "HighShelf",
            FilterType::AllPass => "AllPass",
            FilterType::Comb => "Comb",
        }
    }

    /// Array of all available filter types
    pub const ALL: &'static [FilterType] = &[
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::BandPass,
        FilterType::Notch,
        FilterType::PeakingEQ,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::AllPass,
        FilterType::Comb,
    ];
}

/// Unified audio filter implementing biquad IIR filters and comb filters
/// Based on Audio EQ Cookbook by Robert Bristow-Johnson
#[derive(Clone)]
pub struct AudioFilter {
    /// Type of filter to apply
    pub filter_type: FilterType,
    /// Center or cutoff frequency in Hz
    pub frequency: f32,
    /// Quality factor (Q). Default 0.707 for Butterworth response
    pub q: f32,
    /// Gain in dB (used for PeakingEQ and shelf filters)
    pub gain_db: f32,
    /// Whether the filter is enabled
    pub enabled: bool,
    /// Mix amount: 0.0 = fully dry, 1.0 = fully wet
    pub mix: f32,

    // Biquad coefficients
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,

    // Left channel filter state
    x1_l: f32,
    x2_l: f32,
    y1_l: f32,
    y2_l: f32,

    // Right channel filter state
    x1_r: f32,
    x2_r: f32,
    y1_r: f32,
    y2_r: f32,

    // Comb filter state
    delay_line: Vec<f32>,
    delay_write_pos: usize,
    comb_feedback: f32,

    // Track sample rate to know when to recompute coefficients
    last_sample_rate: f32,
}

impl AudioFilter {
    /// Create a new audio filter with sensible defaults
    pub fn new(filter_type: FilterType, frequency: f32) -> Self {
        AudioFilter {
            filter_type,
            frequency,
            q: 0.707, // Butterworth response
            gain_db: 0.0,
            enabled: true,
            mix: 1.0,

            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,

            x1_l: 0.0,
            x2_l: 0.0,
            y1_l: 0.0,
            y2_l: 0.0,

            x1_r: 0.0,
            x2_r: 0.0,
            y1_r: 0.0,
            y2_r: 0.0,

            delay_line: Vec::new(),
            delay_write_pos: 0,
            comb_feedback: 0.5,

            last_sample_rate: 0.0,
        }
    }

    /// Compute biquad filter coefficients based on filter type and parameters
    /// Must be called before processing audio
    pub fn compute_coefficients(&mut self, sample_rate: f32) {
        self.last_sample_rate = sample_rate;

        // Clamp frequency to avoid numerical issues
        let freq = self.frequency.clamp(20.0, sample_rate / 2.5);

        // Pre-compute common values
        let w0 = 2.0 * PI * freq / sample_rate;
        let sin_w0 = w0.sin();
        let cos_w0 = w0.cos();
        let alpha = sin_w0 / (2.0 * self.q);

        match self.filter_type {
            FilterType::LowPass => {
                self.b0 = (1.0 - cos_w0) / 2.0;
                self.b1 = 1.0 - cos_w0;
                self.b2 = (1.0 - cos_w0) / 2.0;
                let a0 = 1.0 + alpha;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha) / a0;
                self.b0 /= a0;
                self.b1 /= a0;
                self.b2 /= a0;
            }
            FilterType::HighPass => {
                self.b0 = (1.0 + cos_w0) / 2.0;
                self.b1 = -(1.0 + cos_w0);
                self.b2 = (1.0 + cos_w0) / 2.0;
                let a0 = 1.0 + alpha;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha) / a0;
                self.b0 /= a0;
                self.b1 /= a0;
                self.b2 /= a0;
            }
            FilterType::BandPass => {
                self.b0 = alpha;
                self.b1 = 0.0;
                self.b2 = -alpha;
                let a0 = 1.0 + alpha;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha) / a0;
                self.b0 /= a0;
                self.b1 /= a0;
                self.b2 /= a0;
            }
            FilterType::Notch => {
                self.b0 = 1.0;
                self.b1 = -2.0 * cos_w0;
                self.b2 = 1.0;
                let a0 = 1.0 + alpha;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha) / a0;
                self.b0 /= a0;
                self.b1 /= a0;
                self.b2 /= a0;
            }
            FilterType::PeakingEQ => {
                let a = 10_f32.powf(self.gain_db / 40.0);
                self.b0 = 1.0 + alpha * a;
                self.b1 = -2.0 * cos_w0;
                self.b2 = 1.0 - alpha * a;
                let a0 = 1.0 + alpha / a;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha / a) / a0;
                self.b0 /= a0;
                self.b1 /= a0;
                self.b2 /= a0;
            }
            FilterType::LowShelf => {
                let a = 10_f32.powf(self.gain_db / 40.0);
                let s = 1.0;
                let sqrt_a = a.sqrt();
                let denom = (a + 1.0) / s - 1.0;
                let alpha_shelf = sin_w0 / 2.0 * (2.0 * sqrt_a * denom).sqrt();

                let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha_shelf;
                self.b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha_shelf) / a0;
                self.b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0) / a0;
                self.b2 =
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha_shelf) / a0;
                self.a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0) / a0;
                self.a2 = ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha_shelf) / a0;
            }
            FilterType::HighShelf => {
                let a = 10_f32.powf(self.gain_db / 40.0);
                let s = 1.0;
                let sqrt_a = a.sqrt();
                let denom = (a + 1.0) / s - 1.0;
                let alpha_shelf = sin_w0 / 2.0 * (2.0 * sqrt_a * denom).sqrt();

                let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha_shelf;
                self.b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha_shelf) / a0;
                self.b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0) / a0;
                self.b2 =
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha_shelf) / a0;
                self.a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0) / a0;
                self.a2 = ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha_shelf) / a0;
            }
            FilterType::AllPass => {
                self.b0 = 1.0 - alpha;
                self.b1 = -2.0 * cos_w0;
                self.b2 = 1.0 + alpha;
                let a0 = 1.0 + alpha;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha) / a0;
                self.b0 /= a0;
                self.b1 /= a0;
                self.b2 /= a0;
            }
            FilterType::Comb => {
                // Comb filter uses a delay line, not traditional biquad coefficients
                let delay_samples = (sample_rate / freq).max(1.0) as usize;
                self.delay_line = vec![0.0; delay_samples];
                self.delay_write_pos = 0;
                self.comb_feedback = 0.5;
                // No biquad coefficients needed
                self.b0 = 1.0;
                self.b1 = 0.0;
                self.b2 = 0.0;
                self.a1 = 0.0;
                self.a2 = 0.0;
            }
        }
    }

    /// Process a single sample through the left channel filter
    pub fn process_left(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        if matches!(self.filter_type, FilterType::Comb) {
            self.process_comb_left(input)
        } else {
            // Apply biquad filter: y[n] = b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]
            let output = self.b0 * input + self.b1 * self.x1_l + self.b2 * self.x2_l
                - self.a1 * self.y1_l
                - self.a2 * self.y2_l;

            // Update state
            self.x2_l = self.x1_l;
            self.x1_l = input;
            self.y2_l = self.y1_l;
            self.y1_l = output;

            output
        }
    }

    /// Process a single sample through the right channel filter
    pub fn process_right(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        if matches!(self.filter_type, FilterType::Comb) {
            self.process_comb_right(input)
        } else {
            // Apply biquad filter: y[n] = b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]
            let output = self.b0 * input + self.b1 * self.x1_r + self.b2 * self.x2_r
                - self.a1 * self.y1_r
                - self.a2 * self.y2_r;

            // Update state
            self.x2_r = self.x1_r;
            self.x1_r = input;
            self.y2_r = self.y1_r;
            self.y1_r = output;

            output
        }
    }

    /// Process stereo pair with mix blending
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let filtered_left = self.process_left(left);
        let filtered_right = self.process_right(right);

        // Blend dry and wet signals
        let out_left = left.lerp(filtered_left, self.mix);
        let out_right = right.lerp(filtered_right, self.mix);

        (out_left, out_right)
    }

    /// Update filter coefficients if sample rate changed
    pub fn update_if_needed(&mut self, sample_rate: f32) {
        if (self.last_sample_rate - sample_rate).abs() > 0.1 {
            self.compute_coefficients(sample_rate);
        }
    }

    /// Reset all filter state to zero
    pub fn reset_state(&mut self) {
        self.x1_l = 0.0;
        self.x2_l = 0.0;
        self.y1_l = 0.0;
        self.y2_l = 0.0;

        self.x1_r = 0.0;
        self.x2_r = 0.0;
        self.y1_r = 0.0;
        self.y2_r = 0.0;

        for sample in &mut self.delay_line {
            *sample = 0.0;
        }
    }

    // Private helper for comb filter processing on left channel
    fn process_comb_left(&mut self, input: f32) -> f32 {
        if self.delay_line.is_empty() {
            return input;
        }

        let read_pos = self.delay_write_pos;
        let delayed = self.delay_line[read_pos];

        // y[n] = x[n] + feedback * delay_line[read_pos]
        let output = input + self.comb_feedback * delayed;

        // Write input to delay line
        self.delay_line[self.delay_write_pos] = input;

        // Advance write position
        self.delay_write_pos = (self.delay_write_pos + 1) % self.delay_line.len();

        output
    }

    // Private helper for comb filter processing on right channel
    fn process_comb_right(&mut self, input: f32) -> f32 {
        if self.delay_line.is_empty() {
            return input;
        }

        let read_pos = self.delay_write_pos;
        let delayed = self.delay_line[read_pos];

        // y[n] = x[n] + feedback * delay_line[read_pos]
        let output = input + self.comb_feedback * delayed;

        // Write input to delay line
        self.delay_line[self.delay_write_pos] = input;

        // Advance write position
        self.delay_write_pos = (self.delay_write_pos + 1) % self.delay_line.len();

        output
    }
}

// Helper trait for linear interpolation
trait Lerp {
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, other: f32, t: f32) -> f32 {
        self * (1.0 - t) + other * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lowpass_reduces_high_frequency() {
        let mut filter = AudioFilter::new(FilterType::LowPass, 1000.0);
        let sample_rate = 44100.0;
        filter.compute_coefficients(sample_rate);

        // Generate a high-frequency sine wave (10 kHz)
        let freq_hz = 10000.0;
        let mut high_freq_sum = 0.0;
        for i in 0..1000 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * freq_hz * t).sin();
            high_freq_sum += filter.process_left(sample).abs();
        }

        // Generate the same high-frequency wave without filter for comparison
        let mut unfiltered_sum = 0.0;
        for i in 0..1000 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * freq_hz * t).sin();
            unfiltered_sum += sample.abs();
        }

        // High-frequency energy should be reduced by the low-pass filter
        assert!(
            high_freq_sum < unfiltered_sum * 0.8,
            "LowPass filter should reduce high-frequency amplitude"
        );
    }

    #[test]
    fn test_highpass_reduces_low_frequency() {
        let mut filter = AudioFilter::new(FilterType::HighPass, 5000.0);
        let sample_rate = 44100.0;
        filter.compute_coefficients(sample_rate);

        // Generate a low-frequency sine wave (500 Hz)
        let freq_hz = 500.0;
        let mut low_freq_sum = 0.0;
        for i in 0..1000 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * freq_hz * t).sin();
            low_freq_sum += filter.process_left(sample).abs();
        }

        // Generate the same low-frequency wave without filter for comparison
        let mut unfiltered_sum = 0.0;
        for i in 0..1000 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * freq_hz * t).sin();
            unfiltered_sum += sample.abs();
        }

        // Low-frequency energy should be reduced by the high-pass filter
        assert!(
            low_freq_sum < unfiltered_sum * 0.8,
            "HighPass filter should reduce low-frequency amplitude"
        );
    }

    #[test]
    fn test_all_filter_types_process_without_panic() {
        let sample_rate = 44100.0;
        let test_freq = 1000.0;

        for filter_type in FilterType::ALL {
            let mut filter = AudioFilter::new(*filter_type, test_freq);
            filter.compute_coefficients(sample_rate);

            // Process some samples
            for i in 0..100 {
                let t = i as f32 / sample_rate;
                let sample = (2.0 * PI * test_freq * t).sin();
                let _output = filter.process_left(sample);
                // Just ensure it doesn't panic
            }
        }
    }

    #[test]
    fn test_filter_reset_state() {
        let mut filter = AudioFilter::new(FilterType::LowPass, 1000.0);
        let sample_rate = 44100.0;
        filter.compute_coefficients(sample_rate);

        // Process some samples to populate state
        for i in 0..10 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * 1000.0 * t).sin();
            let _ = filter.process_left(sample);
        }

        // Verify state is not all zeros
        assert!(filter.x1_l != 0.0 || filter.y1_l != 0.0);

        // Reset state
        filter.reset_state();

        // Verify state is cleared
        assert_eq!(filter.x1_l, 0.0);
        assert_eq!(filter.x2_l, 0.0);
        assert_eq!(filter.y1_l, 0.0);
        assert_eq!(filter.y2_l, 0.0);
    }

    #[test]
    fn test_stereo_processing_with_mix() {
        let mut filter = AudioFilter::new(FilterType::LowPass, 1000.0);
        filter.mix = 0.5; // 50% dry/wet blend
        let sample_rate = 44100.0;
        filter.compute_coefficients(sample_rate);

        let input_left = 0.5;
        let input_right = -0.3;

        let (out_left, out_right) = filter.process_stereo(input_left, input_right);

        // Output should be between input and fully filtered value
        assert!(out_left.abs() > 0.0);
        assert!(out_right.abs() > 0.0);
    }

    #[test]
    fn test_comb_filter_processing() {
        let mut filter = AudioFilter::new(FilterType::Comb, 200.0);
        let sample_rate = 44100.0;
        filter.compute_coefficients(sample_rate);

        // Process some samples through comb filter
        for i in 0..1000 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * 200.0 * t).sin();
            let _output = filter.process_left(sample);
        }

        // Verify delay line was initialized
        assert!(!filter.delay_line.is_empty());
    }

    #[test]
    fn test_disabled_filter_passes_through() {
        let mut filter = AudioFilter::new(FilterType::LowPass, 1000.0);
        filter.enabled = false;
        let sample_rate = 44100.0;
        filter.compute_coefficients(sample_rate);

        let input = 0.75;
        let output = filter.process_left(input);

        // Disabled filter should pass through unmodified
        assert_eq!(output, input);
    }

    #[test]
    fn test_filter_type_names() {
        assert_eq!(FilterType::LowPass.name(), "LowPass");
        assert_eq!(FilterType::HighPass.name(), "HighPass");
        assert_eq!(FilterType::BandPass.name(), "BandPass");
        assert_eq!(FilterType::Notch.name(), "Notch");
        assert_eq!(FilterType::PeakingEQ.name(), "PeakingEQ");
        assert_eq!(FilterType::LowShelf.name(), "LowShelf");
        assert_eq!(FilterType::HighShelf.name(), "HighShelf");
        assert_eq!(FilterType::AllPass.name(), "AllPass");
        assert_eq!(FilterType::Comb.name(), "Comb");
    }

    #[test]
    fn test_update_if_needed() {
        let mut filter = AudioFilter::new(FilterType::LowPass, 1000.0);
        let sample_rate_1 = 44100.0;
        let sample_rate_2 = 48000.0;

        filter.compute_coefficients(sample_rate_1);
        let initial_b0 = filter.b0;

        // Update with new sample rate
        filter.update_if_needed(sample_rate_2);
        let updated_b0 = filter.b0;

        // Coefficients should have changed
        assert!((initial_b0 - updated_b0).abs() > 0.0001);
    }
}
