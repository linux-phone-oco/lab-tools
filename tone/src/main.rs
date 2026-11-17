//! tone: make a test tone, and find it in a recording.
//!
//!   tone make --freq 1000 --secs 3 --level -20 tone.wav
//!   tone find --freq 1000 recording.wav      # JSON on stdout; exit 0 when found, 1 when not

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "lpoco-tone",
    bin_name = "lpoco-tone",
    version,
    about = "Make a test tone, and find it in a recording"
)]
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
        } => tone::wav::write(&out, &tone::tone(freq, secs, level, rate), rate, channels)
            .with_context(|| format!("writing {}", out.display())),
        Cmd::Find {
            freq,
            min_snr,
            input,
        } => {
            let (samples, rate) = tone::wav::read_mono(&input)
                .with_context(|| format!("reading {}", input.display()))?;
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
