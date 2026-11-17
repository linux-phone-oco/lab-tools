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
    devlab flash now IMAGE BOOT    the manifest's fixes baked in, the device tree fingerprinted
    devlab provision now           a freshly flashed phone: packages, files, Wi-Fi
    devlab test | health | watch   its checks; regressions against what worked; after every boot

## The phone checks itself

`devlab suite put now --runner FILE` puts the health suite on the phone: `devlab-health` (a small
static Rust program, built by `health/build.sh aarch64-unknown-linux-musl`), the generic checks in
`health/suite/checks/`, the phone's own checks from its lab repository, and what the manifest's
`[health]` expects of this phone. A timer runs it 120 s after every boot; `devlab suite run` runs it
now and passes on what the phone says; `devlab suite latest` reads its latest report.

Each check is a small shell file in postmarketOS's test shape that prints `pass|fail|skip TEST
COMPONENT detail`, base layer first (the hardware and the system, no shell in the way), then the
shell layer. Each run is one report on the phone, `/var/lib/devlab/health/RUN.jsonl`, a line per test
in the shape of linux-phone-oco/docs' `test_results`; an unhealthy run keeps its evidence beside it
(kernel log, errors, failed units) and says what is wrong, in plain words, in the journal. A part the
phone never had working is a `gap` in the manifest: still measured, not ill health; a gap that starts
to pass is said. Until the package conveyor publishes Alpine packages, devlab puts these files on the
phone; then it is a package.

`devlab kernel install NAME VMLINUZ DTB MODULES.tgz | try NAME | log | remove NAME` - the safe test-kernel boot (one try by boot
counting, a stalled boot saves its log and falls back to stock), proven on surya.

## The phone for any agent

`devlab mcp` is devlab's door for agents: an MCP server on standard input and output, with the tools
`run` (commands as the user or root, no quoting layer), `put` and `get` (files), `health_report` and
`health_run` (the phone's own suite, with its verdict), `kernel_install`, `kernel_try` (once, falling
back by itself, then the health of the boot it came to), `kernel_status`, and `reboot` (and wait, and
the new boot's health). Each calls what the command line calls.

    claude mcp add --scope user phone -- devlab --device /path/to/LAB/device.toml mcp

## tone

Make a test tone, and find it in a recording - a device's speaker tests its microphone:

    tone make --freq 1000 --level -20 tone.wav    # gentle: -20 dBFS by default
    tone find --freq 1000 recording.wav           # JSON; exit 0 when found, 1 when not

`find` measures the frequency by the Goertzel algorithm (Hann window) and compares it with
neighbouring frequencies, off the tone and its harmonics: found means it stands `--min-snr` dB
(20 by default) above them.

Development: `nix-shell` (nixpkgs pinned, as in lab-agent), `cargo test`. The command line uses clap
for now; moving it onto argenv, as lab-agent uses, is a contained change.

## The ways, held for agents

`devlab guard` is what Claude Code's hooks on the lab call, so that a way an agent must keep is
held when it is tried, not remembered. It needs no manifest.

    devlab guard tool     PreToolUse: refuses (exit 2, saying why) a Bash command that pushes to main,
                          runs python or pip, or reaches a phone by ssh, scp, sftp, rsync or sshpass
    devlab guard start    SessionStart: lab-agent guide and lab-agent inbox, for the session to read

A phone's addresses are the ones devlab has met: every manifest it loads leaves its addresses in
`~/.local/state/devlab/CODENAME/addresses`, beside the Wi-Fi address it learned. In the lab's
`~/.claude/settings.json`:

    "hooks": {
      "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "~/.local/bin/devlab guard tool", "timeout": 10}]}],
      "SessionStart": [{"hooks": [{"type": "command", "command": "~/.local/bin/devlab guard start", "timeout": 30}]}]
    }
