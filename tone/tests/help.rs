// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-tone's command line is declared once with argenv (backlog lpoco-tone-argenv, proof
//! lpoco-tone-help-from-argenv): `--help` prints the argenv usage under both names, and a usage mistake
//! exits 2 with the usage, as lpoco-lab does (devlab/tests/help.rs). What the tone commands do stays as
//! it was: make writes a WAV, find prints JSON and exits 0 when found, 1 when not.

use std::process::Command;

fn run(bin: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(bin).args(args).output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

const BINS: [&str; 2] = [env!("CARGO_BIN_EXE_lpoco-tone"), env!("CARGO_BIN_EXE_tone")];

/// (the path, words its usage must hold)
const PATHS: &[(&[&str], &[&str])] = &[
    (&[], &["make", "find", "--version"]),
    (
        &["make"],
        &["--freq", "--secs", "--level", "--rate", "--channels", "OUT"],
    ),
    (&["find"], &["--freq", "--min-snr", "INPUT"]),
];

#[test]
fn every_path_answers_help_with_its_arguments() {
    for bin in BINS {
        for (path, words) in PATHS {
            for flag in ["--help", "-h"] {
                let mut args: Vec<&str> = path.to_vec();
                args.push(flag);
                let (code, out, err) = run(bin, &args);
                assert_eq!(code, 0, "{args:?}: stderr {err}");
                assert!(out.starts_with("Usage: lpoco-tone"), "{args:?}: {out}");
                for w in *words {
                    assert!(out.contains(w), "{args:?} lacks {w:?}:\n{out}");
                }
            }
        }
    }
}

#[test]
fn a_usage_mistake_exits_2_with_the_usage() {
    for args in [
        &[][..],
        &["bogus"],
        &["make"],
        &["make", "a.wav", "b.wav"],
        &["make", "--no-such-flag", "a.wav"],
        &["make", "--min-snr", "3", "a.wav"],
        &["find", "--secs", "3", "a.wav"],
        &["find"],
        &["find", "--freq", "loud", "a.wav"],
    ] {
        let (code, _, err) = run(BINS[0], args);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(err.contains("Usage: lpoco-tone"), "{args:?}: {err}");
    }
    // help wins over a mistake in the words
    let (code, out, _) = run(BINS[0], &["bogus", "--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("Usage: lpoco-tone"));
}

#[test]
fn make_then_find_still_works_with_a_negative_level() {
    let dir = std::env::temp_dir().join(format!("lpoco-tone-help-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("t.wav");
    let wav = wav.to_str().unwrap();
    for bin in BINS {
        let (code, _, err) = run(
            bin,
            &[
                "make", "--freq", "1000", "--secs", "1", "--level", "-20", wav,
            ],
        );
        assert_eq!(code, 0, "{err}");
        let (code, out, err) = run(bin, &["find", "--freq", "1000", wav]);
        assert_eq!(code, 0, "{out} {err}");
        assert!(out.contains("\"found\":true"), "{out}");
        let (code, out, _) = run(bin, &["find", "--freq", "3000", wav]);
        assert_eq!(code, 1, "{out}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_value_out_of_range_or_zero_is_a_usage_mistake() {
    for args in [
        &["make", "--channels", "70000", "a.wav"][..],
        &["make", "--channels", "0", "a.wav"],
        &["make", "--rate", "5000000000", "a.wav"],
        &["make", "--rate", "0", "a.wav"],
        &["make", "--secs", "inf", "a.wav"],
        &["make", "--rate", "x", "a.wav"],
    ] {
        let (code, _, err) = run(BINS[0], args);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(err.contains("Usage: lpoco-tone"), "{args:?}: {err}");
    }
}

#[test]
fn the_version_is_printed_under_both_names() {
    for bin in BINS {
        for flag in ["--version", "-V"] {
            let (code, out, _) = run(bin, &[flag]);
            assert_eq!(code, 0);
            assert!(out.starts_with("lpoco-tone "), "{out}");
        }
    }
}
