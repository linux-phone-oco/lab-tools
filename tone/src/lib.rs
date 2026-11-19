// SPDX-License-Identifier: MIT OR Apache-2.0
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

/// The samples from `from` to `to` seconds (either open), at `rate`: the part of a recording
/// where the tone sounds, not the pop of a switch before it.
pub fn window(samples: &[f64], rate: u32, from: Option<f64>, to: Option<f64>) -> &[f64] {
    let at = |s: f64| ((s.max(0.0) * f64::from(rate)).round() as usize).min(samples.len());
    let a = from.map_or(0, at);
    let b = to.map_or(samples.len(), at).max(a);
    &samples[a..b]
}

/// Raw signed 16-bit little-endian samples (one channel, as `parecord --raw` writes) in -1..1.
pub fn from_s16le(bytes: &[u8]) -> Vec<f64> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f64::from(i16::from_le_bytes(*b)) / 32768.0)
        .collect()
}

/// Samples in -1..1 as raw signed 16-bit little-endian, each repeated on `channels` channels.
pub fn to_s16le(samples: &[f64], channels: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2 * usize::from(channels));
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * f64::from(i16::MAX)).round() as i16;
        for _ in 0..channels {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// Recordings as files: WAV by hound, and raw s16le beside it.
#[cfg(feature = "wav")]
pub mod wav {
    use std::path::Path;

    /// A WAV file as one channel (the channels averaged), samples in -1..1, and its rate.
    pub fn read_mono(path: &Path) -> Result<(Vec<f64>, u32), hound::Error> {
        let mut r = hound::WavReader::open(path)?;
        let spec = r.spec();
        let ch = usize::from(spec.channels.max(1));
        let flat: Vec<f64> = match spec.sample_format {
            hound::SampleFormat::Float => r
                .samples::<f32>()
                .map(|s| s.map(f64::from))
                .collect::<Result<_, _>>()?,
            hound::SampleFormat::Int => {
                let full = f64::from(1u32 << (spec.bits_per_sample - 1));
                r.samples::<i32>()
                    .map(|s| s.map(|v| f64::from(v) / full))
                    .collect::<Result<_, _>>()?
            }
        };
        Ok((
            flat.chunks(ch)
                .map(|c| c.iter().sum::<f64>() / c.len() as f64)
                .collect(),
            spec.sample_rate,
        ))
    }

    /// A recording, whichever it is: WAV when it starts as one (its own rate), else raw s16le
    /// mono at `raw_rate`.
    pub fn read_any(path: &Path, raw_rate: u32) -> Result<(Vec<f64>, u32), hound::Error> {
        let bytes = std::fs::read(path)?;
        if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
            read_mono(path)
        } else {
            Ok((super::from_s16le(&bytes), raw_rate))
        }
    }

    /// Samples in -1..1 as a 16-bit WAV file, each repeated on `channels` channels.
    pub fn write(path: &Path, samples: &[f64], rate: u32, channels: u16) -> hound::Result<()> {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec)?;
        for s in samples {
            let v = (s.clamp(-1.0, 1.0) * f64::from(i16::MAX)).round() as i16;
            for _ in 0..channels {
                w.write_sample(v)?;
            }
        }
        w.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_s16le_goes_and_comes_back() {
        let t = tone(1000.0, 0.1, -6.0, RATE);
        let back = from_s16le(&to_s16le(&t, 1));
        assert_eq!(back.len(), t.len());
        assert!(t.iter().zip(&back).all(|(a, b)| (a - b).abs() < 1e-4));
        assert_eq!(to_s16le(&t, 2).len(), t.len() * 4);
    }

    #[test]
    fn a_window_is_the_seconds_asked_and_never_past_the_end() {
        let s = vec![0.0; RATE as usize * 2];
        assert_eq!(window(&s, RATE, Some(0.9), Some(1.8)).len(), 43_200);
        assert_eq!(window(&s, RATE, Some(1.5), None).len(), 24_000);
        assert_eq!(window(&s, RATE, Some(3.0), Some(4.0)).len(), 0);
        assert_eq!(window(&s, RATE, Some(1.0), Some(0.5)).len(), 0);
    }

    #[cfg(feature = "wav")]
    #[test]
    fn a_wav_and_a_raw_file_read_alike() {
        let dir = std::env::temp_dir().join(format!("tone-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let t = tone(1000.0, 0.5, -20.0, RATE);
        let (w, r) = (dir.join("t.wav"), dir.join("t.raw"));
        wav::write(&w, &t, RATE, 2).unwrap();
        std::fs::write(&r, to_s16le(&t, 1)).unwrap();
        let (a, ra) = wav::read_any(&w, 8000).unwrap();
        let (b, rb) = wav::read_any(&r, RATE).unwrap();
        assert_eq!((ra, rb), (RATE, RATE), "a WAV keeps its own rate");
        assert_eq!(a.len(), b.len());
        assert!(find(&a, ra, 1000.0, 20.0).found && find(&b, rb, 1000.0, 20.0).found);
        std::fs::remove_dir_all(dir).unwrap();
    }

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
