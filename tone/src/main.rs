// SPDX-License-Identifier: MIT OR Apache-2.0
//! tone: make a test tone, and find it in a recording. See cli.rs for the invocation surface.
//!
//!   lpoco-tone make --freq 1000 --secs 3 --level -20 tone.wav
//!   lpoco-tone find --freq 1000 recording.wav      # JSON on stdout; exit 0 when found, 1 when not

mod cli;

use anyhow::{Context, Result};
use argenv_resolve::{InputResolveDefaultExt, InputResolveExt};
use cli::{Command, Contract as C};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A usage mistake: said with the usage of what was meant, exit 2.
fn usage_error(word: Option<&str>, said: &str) -> ! {
    eprintln!("lpoco-tone: {said}\n\n{}", cli::usage(word));
    std::process::exit(2);
}

fn main() -> Result<()> {
    let model = C::records();
    let r = argenv_resolve::parse(&model);
    let words = r.positionals().to_vec();
    let word = words.first().map(String::as_str);
    let known = word.filter(|w| Command::TOKENS.contains(w));
    if C::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-tone {VERSION}");
        return Ok(());
    }
    if C::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage(known));
        return Ok(());
    }
    // a mistake in the words is a usage mistake: exit 2 with the usage (exit 1 stays for "not found")
    let policy = argenv_resolve::LintPolicy::default();
    let errors: Vec<String> = argenv_resolve::lint(&model, &r)
        .iter()
        .filter(|f| policy.severity(f) == argenv_resolve::Severity::Error)
        .map(|f| f.to_string())
        .collect();
    if !errors.is_empty() {
        usage_error(known, &errors.join("\n"));
    }
    let Some(tree) = cli::TREE.iter().find(|t| Some(t.0) == word) else {
        match word {
            Some(w) => usage_error(None, &format!("unrecognized command '{w}'")),
            None => usage_error(None, "a command is needed"),
        }
    };
    // flags are scoped by argenv to the command; checked here against the tree, so a flag for the other one is refused
    for (key, v) in r.values() {
        let given = v.source == argenv_resolve::Source::Arg;
        let is_flag = model
            .iter()
            .any(|m| m.key == *key && m.arg.as_ref().is_some_and(|a| a.position.is_none()));
        if given
            && is_flag
            && !tree.2.contains(&key.as_str())
            && !["help", "version"].contains(&key.as_str())
        {
            usage_error(
                known,
                &format!("--{} does not apply to {}", key.replace('_', "-"), tree.0),
            );
        }
    }
    if words.len() > 1 + tree.1.len() {
        usage_error(
            known,
            &format!("unexpected argument '{}'", words[1 + tree.1.len()]),
        );
    }
    let need = |v: Option<String>, name: &str| {
        v.unwrap_or_else(|| usage_error(known, &format!("the argument {name} is needed")))
    };
    match C::COMMAND.get_from(&r) {
        Some(Command::Make) => {
            let out = std::path::PathBuf::from(need(C::OUT.get_from(&r), "OUT"));
            let (freq, secs, level) = (
                C::FREQ.get_from_or_default(&r).unwrap_or_default(),
                C::SECS.get_from_or_default(&r).unwrap_or_default(),
                C::LEVEL.get_from_or_default(&r).unwrap_or_default(),
            );
            let rate = u32::try_from(C::RATE.get_from_or_default(&r).unwrap_or_default())
                .ok()
                .filter(|n| *n > 0)
                .unwrap_or_else(|| usage_error(known, "--rate must be between 1 and 4294967295"));
            let channels = u16::try_from(C::CHANNELS.get_from_or_default(&r).unwrap_or_default())
                .ok()
                .filter(|n| *n > 0)
                .unwrap_or_else(|| usage_error(known, "--channels must be between 1 and 65535"));
            if ![freq, secs, level].iter().all(|x| x.is_finite()) {
                usage_error(known, "--freq, --secs and --level must be finite numbers");
            }
            tone::wav::write(&out, &tone::tone(freq, secs, level, rate), rate, channels)
                .with_context(|| format!("writing {}", out.display()))
        }
        Some(Command::Find) => {
            let input = std::path::PathBuf::from(need(C::INPUT.get_from(&r), "INPUT"));
            let freq = C::FREQ.get_from_or_default(&r).unwrap_or_default();
            let min_snr = C::MIN_SNR.get_from_or_default(&r).unwrap_or_default();
            let (samples, rate) = tone::wav::read_mono(&input)
                .with_context(|| format!("reading {}", input.display()))?;
            let f = tone::find(&samples, rate, freq, min_snr);
            let round = |x: f64| (x * 10.0).round() / 10.0;
            println!(
                "{}",
                serde_json::json!({"freq": f.freq, "level_dbfs": round(f.level_dbfs), "noise_dbfs": round(f.noise_dbfs),
                    "snr_db": round(f.snr_db), "found": f.found, "seconds": round(samples.len() as f64 / f64::from(rate))})
            );
            std::process::exit(if f.found { 0 } else { 1 });
        }
        None => usage_error(None, "a command is needed"),
    }
}
