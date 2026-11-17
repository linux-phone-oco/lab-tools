//! `lpoco-health tone make|find`: the lab's tone crate on the phone, so a check that plays a tone
//! and listens for it hands the numbers to tested Rust instead of an awk Goertzel.
//!
//!   lpoco-health tone make [--freq 1000] [--secs 1] [--level -40] [--rate 48000] [--channels 1] [--raw] OUT
//!   lpoco-health tone find [--freq 1000] [--min-snr 20] [--rate 48000] [--from S] [--to S] IN
//!
//! make writes a sine tone: a 16-bit WAV, or raw s16le with --raw (as pacat --raw plays it).
//! find reads a WAV (its own rate) or raw s16le mono (at --rate, as parecord --raw records it),
//! keeps the seconds from --from to --to, and prints one JSON line - freq, level_dbfs, noise_dbfs,
//! snr_db, found, seconds, samples; exit 0 when the tone stands --min-snr dB above the noise around
//! it, 1 when it does not, 2 when it could not be read or was asked wrongly.

use serde_json::json;
use std::path::PathBuf;

/// The options of one subcommand: `--name value` pairs and flags, and the one path.
#[derive(Debug, Default, PartialEq)]
struct Asked {
    freq: Option<f64>,
    secs: Option<f64>,
    level: Option<f64>,
    rate: Option<u32>,
    channels: Option<u16>,
    min_snr: Option<f64>,
    from: Option<f64>,
    to: Option<f64>,
    raw: bool,
    path: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Asked, String> {
    let mut a = Asked::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--raw" {
            a.raw = true;
            continue;
        }
        if let Some(name) = arg.strip_prefix("--") {
            let value = it.next().ok_or(format!("--{name} needs a value"))?;
            let num = || {
                value
                    .parse::<f64>()
                    .map_err(|_| format!("--{name} {value}: not a number"))
            };
            let whole = || {
                value
                    .parse::<u32>()
                    .map_err(|_| format!("--{name} {value}: not a whole number"))
            };
            match name {
                "freq" => a.freq = Some(num()?),
                "secs" => a.secs = Some(num()?),
                "level" => a.level = Some(num()?),
                "min-snr" => a.min_snr = Some(num()?),
                "from" => a.from = Some(num()?),
                "to" => a.to = Some(num()?),
                "rate" => a.rate = Some(whole()?),
                "channels" => {
                    a.channels = Some(
                        u16::try_from(whole()?)
                            .map_err(|_| format!("--channels {value}: too many"))?,
                    )
                }
                _ => return Err(format!("--{name}: no such option")),
            }
        } else if a.path.replace(arg.into()).is_some() {
            return Err(format!("{arg}: one file only"));
        }
    }
    Ok(a)
}

/// One decimal, as the checks print it.
fn r(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// What `tone find` says of a recording, and whether the tone was found.
fn finding(samples: &[f64], rate: u32, a: &Asked) -> (serde_json::Value, bool) {
    let part = ::tone::window(samples, rate, a.from, a.to);
    let f = ::tone::find(
        part,
        rate,
        a.freq.unwrap_or(1000.0),
        a.min_snr.unwrap_or(20.0),
    );
    (
        json!({"freq": f.freq, "level_dbfs": r(f.level_dbfs), "noise_dbfs": r(f.noise_dbfs),
            "snr_db": r(f.snr_db), "found": f.found, "seconds": r(part.len() as f64 / f64::from(rate)),
            "samples": part.len()}),
        f.found,
    )
}

/// `lpoco-health tone ...`: the exit code.
pub fn main(args: &[String]) -> i32 {
    let usage = "usage: lpoco-health tone make|find [--freq HZ] ... FILE";
    let Some((verb, rest)) = args.split_first() else {
        eprintln!("{usage}");
        return 2;
    };
    let a = match parse(rest) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("lpoco-health tone: {e}");
            return 2;
        }
    };
    let Some(path) = a.path.clone() else {
        eprintln!("{usage}");
        return 2;
    };
    let rate = a.rate.unwrap_or(48_000);
    match verb.as_str() {
        "make" => {
            let t = ::tone::tone(
                a.freq.unwrap_or(1000.0),
                a.secs.unwrap_or(1.0),
                a.level.unwrap_or(-40.0).min(0.0),
                rate,
            );
            let channels = a.channels.unwrap_or(1);
            let written = if a.raw {
                std::fs::write(&path, ::tone::to_s16le(&t, channels)).map_err(|e| e.to_string())
            } else {
                ::tone::wav::write(&path, &t, rate, channels).map_err(|e| e.to_string())
            };
            match written {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("lpoco-health tone make: {}: {e}", path.display());
                    2
                }
            }
        }
        "find" => match ::tone::wav::read_any(&path, rate) {
            Ok((samples, rate)) => {
                let (said, found) = finding(&samples, rate, &a);
                println!("{said}");
                if found {
                    0
                } else {
                    1
                }
            }
            Err(e) => {
                eprintln!("lpoco-health tone find: {}: {e}", path.display());
                2
            }
        },
        _ => {
            eprintln!("{usage}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn options_and_the_file_are_read_in_any_order() {
        let a = parse(&words("--freq 1000 rec --level -40 --raw --to 1.8")).unwrap();
        assert_eq!(a.freq, Some(1000.0));
        assert_eq!(
            a.level,
            Some(-40.0),
            "a negative level is a value, not an option"
        );
        assert!(a.raw);
        assert_eq!(a.to, Some(1.8));
        assert_eq!(a.path, Some(PathBuf::from("rec")));
    }

    #[test]
    fn a_wrong_ask_is_said_not_guessed() {
        assert!(parse(&words("--freq")).is_err());
        assert!(parse(&words("--freq loud x")).is_err());
        assert!(parse(&words("--volume 3 x")).is_err());
        assert!(parse(&words("a b")).is_err());
    }

    #[test]
    fn a_tone_made_raw_is_found_in_its_window_and_silence_is_not() {
        let dir = std::env::temp_dir().join(format!("health-tone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("t.raw");
        let s = |x: &str| x.to_string();
        let made = main(&[
            s("make"),
            s("--raw"),
            s("--secs"),
            s("2"),
            s(f.to_str().unwrap()),
        ]);
        assert_eq!(made, 0);
        assert_eq!(std::fs::metadata(&f).unwrap().len(), 2 * 48_000 * 2);
        let mut bytes = std::fs::read(&f).unwrap();
        bytes.splice(0..0, vec![0u8; 48_000 * 2]); // a second of silence before it
        let samples = ::tone::from_s16le(&bytes);
        let at = |from: f64, to: f64| {
            finding(
                &samples,
                48_000,
                &Asked {
                    from: Some(from),
                    to: Some(to),
                    ..Asked::default()
                },
            )
        };
        let (said, found) = at(1.2, 2.8);
        assert!(found, "{said}");
        assert_eq!(said["samples"], 76_800);
        assert!(
            (said["level_dbfs"].as_f64().unwrap() + 40.0).abs() < 0.5,
            "{said}"
        );
        assert!(!at(0.0, 0.9).1, "the silence before it holds no tone");
        assert_eq!(main(&[s("find"), s(dir.join("none").to_str().unwrap())]), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
