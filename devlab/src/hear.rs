//! The lab's ear: a tone played on each of the phone's speakers, heard through a microphone in the
//! lab and measured - so "the speaker works" is something a machine heard, not only a driver that
//! loaded. The microphone belongs to the lab, not the device: it is named in the lab's private
//! configuration ($DEVLAB_MICROPHONE, or ~/.config/devlab/microphone - a PipeWire source name).
//!
//! One microphone cannot tell which speaker made a sound: a tone on one channel proves that channel
//! alone produces sound, not that left is on the left.

use crate::device::Device;
use crate::health::Check;
use crate::phone::Phone;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tone::Finding;

const RATE: u32 = 48_000;
const SECONDS: f64 = 2.0;
/// The tone's level in the file, dBFS; the phone's volume is set to a known value while it plays.
const LEVEL: f64 = -12.0;
/// How far a tone must stand above the noise around it, and above the silent room, to count as heard.
const MIN_DB: f64 = 10.0;

/// A listening test counts only when the microphone has been put by the phone: its keeper is told
/// first, places it, and gives the word. Without that word nothing is played and nothing is judged.
pub fn placed_or_refuse(placed: bool) -> Result<()> {
    if placed {
        return Ok(());
    }
    bail!("the microphone must be placed by the phone first: tell its keeper, wait for their word, then run this again with --placed")
}

/// The lab's microphone, as PipeWire names it.
pub fn microphone() -> Result<String> {
    if let Ok(m) = std::env::var("DEVLAB_MICROPHONE") {
        if !m.trim().is_empty() {
            return Ok(m.trim().to_string());
        }
    }
    let path = std::env::var("HOME")
        .map(|h| Path::new(&h).join(".config/devlab/microphone"))
        .context("no HOME")?;
    let name = std::fs::read_to_string(&path)
        .with_context(|| format!("the lab has no microphone named: put a PipeWire source name (pactl list sources short) in {}", path.display()))?;
    Ok(name.trim().to_string())
}

/// A stereo file with the tone on the named channel (left, right or both) and silence on the other.
pub fn write_tone(path: &Path, freq: f64, channel: &str) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)
        .with_context(|| format!("writing {}", path.display()))?;
    for x in tone::tone(freq, SECONDS, LEVEL, RATE) {
        let v = (x * f64::from(i16::MAX)) as i16;
        w.write_sample(if channel != "right" { v } else { 0 })?;
        w.write_sample(if channel != "left" { v } else { 0 })?;
    }
    w.finalize()?;
    Ok(())
}

/// A recording as one channel of samples in -1..1, and its rate.
pub fn read_mono(path: &Path) -> Result<(Vec<f64>, u32)> {
    let mut r =
        hound::WavReader::open(path).with_context(|| format!("reading {}", path.display()))?;
    let spec = r.spec();
    let all: Vec<f64> = match spec.sample_format {
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
    let ch = usize::from(spec.channels.max(1));
    Ok((
        all.chunks(ch)
            .map(|c| c.iter().sum::<f64>() / ch as f64)
            .collect(),
        spec.sample_rate,
    ))
}

/// Whether the tone was heard: it stands out of the noise around it, and above the silent room.
pub fn verdict(heard: &Finding, room: &Finding) -> (bool, String) {
    let above_room = heard.level_dbfs - room.level_dbfs;
    let ok = heard.found && heard.snr_db >= MIN_DB && above_room >= MIN_DB;
    let detail = format!(
        "{:.0} Hz at {:.1} dBFS: {:.0} dB above the noise around it, {:.0} dB above the silent room ({:.1} dBFS)",
        heard.freq, heard.level_dbfs, heard.snr_db, above_room, room.level_dbfs
    );
    (
        ok,
        if ok {
            detail
        } else {
            format!("NOT HEARD - {detail}")
        },
    )
}

/// Records from the lab's microphone for `secs`, doing `during` meanwhile.
fn record(source: &str, path: &Path, secs: f64, during: impl FnOnce() -> Result<()>) -> Result<()> {
    let _ = std::fs::remove_file(path);
    let mut child = Command::new("timeout")
        .args([
            "--signal=INT",
            &format!("{secs:.1}"),
            "pw-record",
            "--target",
            source,
            "--rate",
            &RATE.to_string(),
            "--channels",
            "1",
        ])
        .arg(path)
        .spawn()
        .context("starting pw-record")?;
    std::thread::sleep(Duration::from_millis(700));
    let done = during();
    let _ = child.wait();
    done?;
    if !path.exists() {
        bail!("nothing was recorded from {source}");
    }
    Ok(())
}

/// Plays each of the device's tones on the phone and hears it through the lab's microphone.
pub fn hear(device: &Device, phone: &Phone) -> Result<Vec<Check>> {
    let mic = microphone()?;
    let work = std::env::temp_dir().join(format!("devlab-hear-{}", std::process::id()));
    std::fs::create_dir_all(&work)?;
    let tones = device.hear.tones();
    let mut checks = Vec::new();
    let result = (|| -> Result<()> {
        // the room first, in silence: a hum at the tone's pitch must not pass for the tone
        let silent = work.join("room.wav");
        record(&mic, &silent, 2.0, || Ok(()))?;
        let (room_samples, room_rate) = read_mono(&silent)?;
        for t in &tones {
            let local = work.join("tone.wav");
            write_tone(&local, t.freq, &t.channel)?;
            phone.put(local.to_str().context("path")?, "/tmp/devlab-tone.wav")?;
            let heard_file = work.join(format!("{}.wav", t.name));
            // a known volume while it plays, and the phone's own volume back afterwards
            let play = "export XDG_RUNTIME_DIR=/run/user/$(id -u); v=$(pactl get-sink-volume @DEFAULT_SINK@ | grep -o '[0-9]*%' | head -n1); pactl set-sink-volume @DEFAULT_SINK@ 60%; paplay /tmp/devlab-tone.wav; pactl set-sink-volume @DEFAULT_SINK@ \"${v:-50%}\"\n";
            record(&mic, &heard_file, SECONDS + 2.5, || {
                let (ok, out) = phone.sh(play)?;
                if !ok {
                    bail!("the phone could not play the tone: {}", out.trim());
                }
                Ok(())
            })?;
            let (samples, rate) = read_mono(&heard_file)?;
            let (ok, detail) = verdict(
                &tone::find(&samples, rate, t.freq, MIN_DB),
                &tone::find(&room_samples, room_rate, t.freq, MIN_DB),
            );
            checks.push(Check {
                key: format!("{}/{}", t.component, t.name),
                outcome: if ok { "pass" } else { "fail" }.into(),
                detail,
            });
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result?;
    Ok(checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(level: f64, snr: f64, found: bool) -> Finding {
        Finding {
            freq: 1000.0,
            level_dbfs: level,
            noise_dbfs: level - snr,
            snr_db: snr,
            found,
        }
    }

    #[test]
    fn heard_means_out_of_the_noise_and_above_the_room() {
        assert!(verdict(&finding(-30.0, 25.0, true), &finding(-60.0, 2.0, false)).0);
        assert!(
            !verdict(&finding(-58.0, 3.0, false), &finding(-60.0, 2.0, false)).0,
            "lost in the noise"
        );
        let (ok, detail) = verdict(&finding(-30.0, 25.0, true), &finding(-33.0, 24.0, true));
        assert!(
            !ok,
            "a hum already in the room at that pitch is not the phone's tone"
        );
        assert!(detail.starts_with("NOT HEARD"));
    }

    #[test]
    fn a_written_tone_is_found_again_on_its_channel() {
        let dir = std::env::temp_dir().join(format!("devlab-hear-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("left.wav");
        write_tone(&f, 1000.0, "left").unwrap();
        let (samples, rate) = read_mono(&f).unwrap();
        assert_eq!(rate, RATE);
        let found = tone::find(&samples, rate, 1000.0, MIN_DB);
        assert!(found.found && found.snr_db > 40.0, "{found:?}");
        assert!(
            !tone::find(&samples, rate, 3000.0, MIN_DB).found,
            "and no other pitch"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_is_heard_without_the_word_that_the_microphone_is_placed() {
        let refused = placed_or_refuse(false).unwrap_err().to_string();
        assert!(
            refused.contains("--placed") && refused.contains("wait for their word"),
            "{refused}"
        );
        assert!(placed_or_refuse(true).is_ok());
    }
}
