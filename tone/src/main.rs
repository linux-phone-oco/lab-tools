//! tone: make a test tone, and find it in a recording.
//!
//!   tone make --freq 1000 --secs 3 --level -20 tone.wav
//!   tone find --freq 1000 recording.wav      # JSON on stdout; exit 0 when found, 1 when not

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Make a test tone, and find it in a recording")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write a sine tone as a 16-bit WAV file
    Make {
        #[arg(long, default_value_t = 1000.0)]
        freq: f64,
        #[arg(long, default_value_t = 3.0)]
        secs: f64,
        /// Level in dBFS (0 is full scale) - gentle by default
        #[arg(long, default_value_t = -20.0, allow_hyphen_values = true)]
        level: f64,
        #[arg(long, default_value_t = 48_000)]
        rate: u32,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        out: PathBuf,
    },
    /// Measure a frequency in a WAV recording against the noise around it
    Find {
        #[arg(long, default_value_t = 1000.0)]
        freq: f64,
        /// How many dB above the noise the tone must stand to count as found
        #[arg(long, default_value_t = 20.0)]
        min_snr: f64,
        input: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Make {
            freq,
            secs,
            level,
            rate,
            channels,
            out,
        } => {
            let spec = hound::WavSpec {
                channels,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut w = hound::WavWriter::create(&out, spec)
                .with_context(|| format!("writing {}", out.display()))?;
            for s in tone::tone(freq, secs, level, rate) {
                for _ in 0..channels {
                    w.write_sample((s * f64::from(i16::MAX)).round() as i16)?;
                }
            }
            w.finalize()?;
            Ok(())
        }
        Cmd::Find {
            freq,
            min_snr,
            input,
        } => {
            let (samples, rate) = read_mono(&input)?;
            let f = tone::find(&samples, rate, freq, min_snr);
            let r = |x: f64| (x * 10.0).round() / 10.0;
            println!(
                "{}",
                serde_json::json!({"freq": f.freq, "level_dbfs": r(f.level_dbfs), "noise_dbfs": r(f.noise_dbfs),
                    "snr_db": r(f.snr_db), "found": f.found, "seconds": r(samples.len() as f64 / f64::from(rate))})
            );
            std::process::exit(if f.found { 0 } else { 1 });
        }
    }
}

/// A WAV file as one channel (the channels averaged), samples in -1..1, and its rate.
fn read_mono(path: &PathBuf) -> Result<(Vec<f64>, u32)> {
    let mut r =
        hound::WavReader::open(path).with_context(|| format!("reading {}", path.display()))?;
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
