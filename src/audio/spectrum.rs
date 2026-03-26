use std::f32::consts::PI;

/// In-place radix-2 Cooley-Tukey FFT.
/// `re` and `im` must have length that is a power of 2.
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    assert_eq!(n, im.len());
    assert!(n.is_power_of_two(), "FFT length must be a power of 2");

    // Bit-reversal permutation
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    // Butterfly stages
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle = -2.0 * PI / len as f32;
        let wn_re = angle.cos();
        let wn_im = angle.sin();

        let mut start = 0;
        while start < n {
            let mut w_re = 1.0f32;
            let mut w_im = 0.0f32;
            for k in 0..half {
                let a = start + k;
                let b = start + k + half;
                let t_re = w_re * re[b] - w_im * im[b];
                let t_im = w_re * im[b] + w_im * re[b];
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let new_w_re = w_re * wn_re - w_im * wn_im;
                let new_w_im = w_re * wn_im + w_im * wn_re;
                w_re = new_w_re;
                w_im = new_w_im;
            }
            start += len;
        }
        len <<= 1;
    }
}

/// Apply a Hann window in-place.
pub fn hann_window(samples: &mut [f32]) {
    let n = samples.len() as f32;
    for (i, s) in samples.iter_mut().enumerate() {
        let w = 0.5 * (1.0 - (2.0 * PI * i as f32 / n).cos());
        *s *= w;
    }
}

/// Compute power spectrum in dB, binned into `num_buckets` buckets
/// spanning from 0 Hz to `max_freq` Hz.
///
/// Uses **average power** per bucket so that notch filters and other
/// narrow-band features are visible rather than being masked by the
/// loudest FFT bin in the bucket.
///
/// Returns a Vec of length `num_buckets`, each value in dB (clamped to `floor_db`..0).
pub fn compute_spectrum_buckets(
    samples: &[f32],
    sample_rate: f32,
    num_buckets: usize,
    max_freq: f32,
    floor_db: f32,
) -> Vec<f32> {
    // Find the next power-of-2 FFT size >= samples.len()
    let fft_size = samples.len().next_power_of_two();

    let mut re = vec![0.0f32; fft_size];
    let mut im = vec![0.0f32; fft_size];

    // Copy and window
    let copy_len = samples.len().min(fft_size);
    re[..copy_len].copy_from_slice(&samples[..copy_len]);
    hann_window(&mut re[..copy_len]);

    fft(&mut re, &mut im);

    // Frequency resolution
    let bin_hz = sample_rate / fft_size as f32;
    let bucket_width = max_freq / num_buckets as f32;

    // Accumulate power (magnitude²) and count per bucket
    let mut power_sum = vec![0.0f32; num_buckets];
    let mut bin_count = vec![0u32; num_buckets];

    // Only use first half of FFT (positive frequencies)
    let max_bin = ((max_freq / bin_hz) as usize).min(fft_size / 2);

    for bin_idx in 0..max_bin {
        let freq = bin_idx as f32 * bin_hz;
        let bucket_idx = (freq / bucket_width) as usize;
        if bucket_idx >= num_buckets {
            break;
        }

        let magnitude_sq = re[bin_idx] * re[bin_idx] + im[bin_idx] * im[bin_idx];
        // Normalize by FFT size squared
        let power = magnitude_sq / (fft_size as f32).powi(2);
        power_sum[bucket_idx] += power;
        bin_count[bucket_idx] += 1;
    }

    // Convert average power to dB
    let mut buckets = vec![floor_db; num_buckets];
    for i in 0..num_buckets {
        if bin_count[i] > 0 {
            let avg_power = power_sum[i] / bin_count[i] as f32;
            let rms = avg_power.sqrt();
            let db = if rms > 1e-10 {
                20.0 * rms.log10()
            } else {
                floor_db
            };
            buckets[i] = db.max(floor_db);
        }
    }

    buckets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_single_sine() {
        // Generate a 1000 Hz sine at 44100 Hz sample rate, 4096 samples
        let n = 4096;
        let sample_rate = 44100.0;
        let freq = 1000.0;

        let mut re: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin())
            .collect();
        let mut im = vec![0.0f32; n];

        hann_window(&mut re);
        fft(&mut re, &mut im);

        // Find the peak bin
        let magnitudes: Vec<f32> = re
            .iter()
            .zip(im.iter())
            .map(|(r, i)| (r * r + i * i).sqrt())
            .collect();

        let peak_bin = magnitudes[..n / 2]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;

        let peak_freq = peak_bin as f32 * sample_rate / n as f32;
        println!("Peak frequency: {:.1} Hz (expected ~1000 Hz)", peak_freq);
        assert!(
            (peak_freq - freq).abs() < 20.0,
            "Peak should be near 1000 Hz, got {}",
            peak_freq
        );
    }

    #[test]
    fn spectrum_buckets_detect_tone() {
        let n = 4096;
        let sample_rate = 48000.0;
        let freq = 3500.0; // Should land in bucket 3 (3000-4000 Hz)

        let samples: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin() * 0.5)
            .collect();

        let buckets = compute_spectrum_buckets(&samples, sample_rate, 20, 20000.0, -80.0);

        // Bucket 3 (3000-4000 Hz) should have the highest energy
        let peak_bucket = buckets
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;

        println!("Buckets: {:?}", buckets);
        println!("Peak bucket: {} (expected 3 for 3000-4000 Hz)", peak_bucket);
        assert_eq!(peak_bucket, 3, "3500 Hz tone should peak in bucket 3");
    }

    /// Verify that a notch filter creates a visible dip in the spectrum.
    /// Generates white noise, applies a notch at 10.5 kHz (centered within
    /// bucket 10) with a 2 kHz bandwidth, and checks the dip.
    #[test]
    fn spectrum_shows_notch_dip() {
        use crate::audio::therapy::NotchFilter;

        let n = 8192;
        let sample_rate = 48000.0;

        // Deterministic "white noise" via a simple LCG
        let mut rng: u32 = 42;
        let mut noise = Vec::with_capacity(n);
        for _ in 0..n {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let val = (rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
            noise.push(val);
        }

        // Notch at 10.5 kHz (centered in the 10-11 kHz bucket) with 2 kHz
        // bandwidth so the deep part of the notch covers most of the bucket.
        let mut notch = NotchFilter::new(10500.0, 2000.0);
        notch.enabled = true;
        notch.compute_coefficients(sample_rate);

        let filtered: Vec<f32> = noise.iter().map(|&s| notch.process(s)).collect();

        let buckets_raw = compute_spectrum_buckets(&noise, sample_rate, 20, 20000.0, -80.0);
        let buckets_notched = compute_spectrum_buckets(&filtered, sample_rate, 20, 20000.0, -80.0);

        println!("Raw buckets:     {:?}", buckets_raw);
        println!("Notched buckets: {:?}", buckets_notched);

        // Bucket 10 (10-11 kHz) should drop noticeably.
        let notch_bucket = 10;
        let raw_val = buckets_raw[notch_bucket];
        let notched_val = buckets_notched[notch_bucket];
        let drop = raw_val - notched_val;

        // Check against unaffected neighbors (buckets 6 and 14, far from notch)
        let neighbor_avg = (buckets_notched[6] + buckets_notched[14]) / 2.0;
        let dip = neighbor_avg - notched_val;

        println!(
            "Bucket {} — raw: {:.1} dB, notched: {:.1} dB, drop: {:.1} dB, dip vs neighbors: {:.1} dB",
            notch_bucket, raw_val, notched_val, drop, dip
        );

        // A 2 kHz notch centered in a 1 kHz bucket should produce a clearly
        // visible dip of at least 3 dB versus unaffected neighbors.
        assert!(
            dip > 3.0,
            "Notch filter should create at least a 3 dB dip vs neighbors, got {:.1} dB",
            dip
        );
    }
}
