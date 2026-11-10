//! A test tone, and finding it again: a device's speaker plays a tone, its microphone records,
//! and the recording tells whether the tone came through - no listener needed.

use std::f64::consts::PI;

/// A sine tone: `secs` seconds at `freq` Hz and `level` dBFS (0 dBFS is full scale), one channel.
pub fn tone(freq: f64, secs: f64, level: f64, rate: u32) -> Vec<f64> {
    let amplitude = 10f64.powf(level / 20.0);
    let n = (secs * f64::from(rate)).round() as usize;
    (0..n)
        .map(|i| amplitude * (2.0 * PI * freq * i as f64 / f64::from(rate)).sin())
        .collect()
}

/// The amplitude (0..1 of full scale) of `freq` in `samples`, by the Goertzel algorithm over a
/// Hann window - one frequency, measured without a whole spectrum.
pub fn amplitude_at(samples: &[f64], rate: u32, freq: f64) -> f64 {
    let n = samples.len();
    if n < 2 {
        return 0.0;
    }
    let coeff = 2.0 * (2.0 * PI * freq / f64::from(rate)).cos();
    let (mut s1, mut s2, mut weight) = (0.0, 0.0, 0.0);
    for (i, x) in samples.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / (n - 1) as f64).cos();
        weight += w;
        let s0 = x * w + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let power = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0);
    2.0 * power.sqrt() / weight
}

/// What a recording says about a tone.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub freq: f64,
    /// the tone's level, dBFS
    pub level_dbfs: f64,
    /// the level around it (frequencies off the tone and its harmonics), dBFS
    pub noise_dbfs: f64,
    pub snr_db: f64,
    pub found: bool,
}

/// Frequencies near the tone, but off it and off its harmonics, whose levels stand for the noise.
const NEIGHBOURS: [f64; 6] = [0.71, 0.79, 0.89, 1.12, 1.26, 1.41];

/// Whether `freq` stands at least `min_snr` dB above the noise around it in `samples`.
pub fn find(samples: &[f64], rate: u32, freq: f64, min_snr: f64) -> Finding {
    let db = |a: f64| 20.0 * a.max(1e-12).log10();
    let level = db(amplitude_at(samples, rate, freq));
    let noise_amp = NEIGHBOURS
        .iter()
        .map(|k| amplitude_at(samples, rate, freq * k))
        .sum::<f64>()
        / NEIGHBOURS.len() as f64;
    let noise = db(noise_amp);
    let snr = level - noise;
    Finding {
        freq,
        level_dbfs: level,
        noise_dbfs: noise,
        snr_db: snr,
        found: snr >= min_snr && level > -100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// Deterministic white noise at roughly `level` dBFS RMS.
    fn noise(n: usize, level: f64) -> Vec<f64> {
        let a = 10f64.powf(level / 20.0) * 3f64.sqrt();
        let mut x: u64 = 0x2545_f491_4f6c_dd1d;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                a * ((x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0)
            })
            .collect()
    }

    #[test]
    fn a_tone_measures_at_its_own_level() {
        let f = find(&tone(1000.0, 1.0, -20.0, RATE), RATE, 1000.0, 20.0);
        assert!((f.level_dbfs + 20.0).abs() < 0.5, "{f:?}");
        assert!(f.found, "{f:?}");
    }

    #[test]
    fn silence_is_not_a_tone() {
        assert!(!find(&vec![0.0; RATE as usize], RATE, 1000.0, 20.0).found);
    }

    #[test]
    fn another_frequency_is_not_the_tone() {
        assert!(!find(&tone(1500.0, 1.0, -20.0, RATE), RATE, 1000.0, 20.0).found);
    }

    #[test]
    fn a_quiet_tone_is_found_in_noise() {
        let t = tone(1000.0, 2.0, -45.0, RATE);
        let mixed: Vec<f64> = t
            .iter()
            .zip(noise(t.len(), -40.0))
            .map(|(a, b)| a + b)
            .collect();
        let f = find(&mixed, RATE, 1000.0, 20.0);
        assert!(
            f.found,
            "a tone 5 dB under broadband noise still stands out in its own band: {f:?}"
        );
        assert!(
            !find(&noise(t.len(), -40.0), RATE, 1000.0, 20.0).found,
            "noise alone is no tone"
        );
    }
}
