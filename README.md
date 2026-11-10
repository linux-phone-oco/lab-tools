# lab-tools

Tools for bringing up and testing Linux phones, not tied to one device: each takes what any phone
gives (a WAV recording, a log) and does the work on the lab machine, so a new device needs no
toolchain of its own. Surya (Xiaomi POCO X3 NFC) is the first device they serve; the flows that use
them live in its lab (linux-phone-oco/surya-lab).

## devlab

The device lab, for any phone: everything it knows about a device comes from the device's
manifest, `device.toml`, in its lab repository (found like cargo finds Cargo.toml, or `--device`).
Secrets and what is learned at run time stay outside repositories: `~/.config/devlab/CODENAME/`
(the password, mode 600) and `~/.local/state/devlab/CODENAME/` (the Wi-Fi address).

    devlab phone find              which address answers (the manifest's, then the remembered one)
    devlab phone sh   < script     commands as the user - on standard input, so no quoting layer
    devlab phone root < script     commands as root
    devlab phone put LOCAL REMOTE
    devlab phone reboot            reboot, and wait until it answers again

Next: `devlab kernel install | try | log | remove` - the safe test-kernel boot (one try by boot
counting, a stalled boot saves its log and falls back to stock), proven on surya.

## tone

Make a test tone, and find it in a recording - a device's speaker tests its microphone:

    tone make --freq 1000 --level -20 tone.wav    # gentle: -20 dBFS by default
    tone find --freq 1000 recording.wav           # JSON; exit 0 when found, 1 when not

`find` measures the frequency by the Goertzel algorithm (Hann window) and compares it with
neighbouring frequencies, off the tone and its harmonics: found means it stands `--min-snr` dB
(20 by default) above them.

Development: `nix-shell` (nixpkgs pinned, as in lab-agent), `cargo test`. The command line uses clap
for now; moving it onto argenv, as lab-agent uses, is a contained change.
