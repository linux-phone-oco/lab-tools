---
what: the lpoco tools - bringing up and testing Linux phones, not tied to one device
kept by: orchestration; its lab-tools builder lands here, one backlog item per run
records in: linux-phone-oco/docs (tad q backlog --project lab-tools)
branches: builder/WHAT, one change each, deleted when landed; release PRs are release-plz's
project map: linux-phone-oco/docs MAP.md
---

# lab-tools

Tools for bringing up and testing Linux phones, not tied to one device: each takes what any phone
gives (a WAV recording, a log) and does the work on the lab machine, so a new device needs no
toolchain of its own. Surya (Xiaomi POCO X3 NFC) is the first device they serve; the flows that use
them live in its lab (linux-phone-oco/surya-lab).
## Map

| folder | what it is |
|---|---|
| `lpoco/` | `lpoco`, the one front door: `lpoco NAME` runs `lpoco-NAME` |
| `devlab/` | `lpoco lab`: reach a phone, flash, try a kernel once, its keeper's lease, long jobs |
| `manifest/` | `device.toml`'s types and JSON Schema, read by every program |
| `health/` | `lpoco-health`, the suite the phone runs on itself, and the generic checks |
| `bringup/` | `lpoco bringup`: boot images made, read and checked; device-tree checks |
| `attended/` | `lpoco attended`: a procedure as named steps that each stop the run |
| `host/` | `lpoco host`: the lab machine prepared from declarations |
| `layer/` | `lpoco layer`: a phone's device layer packed as distribution packages |
| `tone/` | `lpoco tone`: a test tone made, and found in a recording |
| `ffs-acm/` | phone-side helpers: a serial console through FunctionFS (`ffs-acm`), and `restart2`, which restarts the phone into its bootloader |
| `packaging/` | the Alpine package and signed repository key; the release conveyor |

One crate per folder, each with its README and CHANGELOG. Never at the top: run outputs, recordings, worktrees.

## devlab (lpoco lab)

One program under two names: `lpoco lab` (the program `lpoco-lab`) and `devlab` run the same code; likewise `lpoco tone` (`lpoco-tone`) and `tone`. The family has one version (`lpoco list`). The old names stay.

The device lab, for any phone: everything it knows about a device comes from the device's
manifest, `device.toml`, in its lab repository (found like cargo finds Cargo.toml, or `--device`).
Secrets and what is learned at run time stay outside repositories: `~/.config/devlab/CODENAME/`
(the password, mode 600) and `~/.local/state/devlab/CODENAME/` (the Wi-Fi address).

    lpoco lab phone find              which address answers (the manifest's, then the remembered one)
    lpoco lab phone sh   < script     commands as the user - on standard input, so no quoting layer
    lpoco lab phone root < script     commands as root
    lpoco lab phone put LOCAL REMOTE
    lpoco lab phone reboot            reboot, and wait until it answers again
    lpoco lab flash now IMAGE BOOT    the manifest's fixes baked in, the device tree fingerprinted -
                                   by [fastboot] serial, only where the manifest says writes = true
    lpoco lab fastboot state | getvar NAME | boot now BOOT.IMG | reboot now [--bootloader]
                                   a phone in fastboot, by its serial: read it, boot from memory
    lpoco lab provision now           a freshly flashed phone: packages, files, Wi-Fi
    lpoco lab test | health | watch   its checks; regressions against what worked; after every boot

## The phone checks itself: lpoco-health

`lpoco lab suite put now` puts the health suite on the phone - the lab's checks and manifest as its main has them (devlab's own clone; `--here` takes this clone's, to try a check before it lands): `lpoco-health` - the
runner (a small static Rust program) and the generic checks in `health/suite/checks/` - from
lab-tools' signed repository (its key and repository added to the phone once), then the phone's own
checks from its lab repository and what the manifest's `[health]` expects of this phone. With
`--built-runner` devlab builds the runner (`health/build.sh aarch64-unknown-linux-musl`) in the lab-tools it
was itself built from and copies it with that tree's checks instead, to try what is not released yet;
`--runner FILE` copies a runner built elsewhere. A timer runs the suite 120 s after
every boot; `lpoco lab suite run` runs it now and passes on what the phone says; `lpoco lab suite latest`
reads its latest report.

Each check is a small shell file in postmarketOS's test shape that prints `pass|fail|skip TEST
COMPONENT detail`, base layer first (the hardware and the system, no shell in the way), then the
shell layer, then the telephony layer: what costs something to run. A call or an SMS runs at most once a
day (`once_a_day NAME` in lib.sh, skipping with "ran today" after); mobile data (`mobile-data`) runs every
boot over the modem's own interface, about 150 kB, 5 MB once a week for throughput, each run's bytes kept
in the month's ledger (`lpoco-health data`) and skipped once the month has spent the manifest's
`data_budget_mb` (250 by default). Each run is one report on the phone, `/var/lib/lpoco-health/RUN.jsonl`, a line per test
in the shape of the device base's `test_results` (docs' when the lab has no base); an unhealthy run keeps its evidence beside it
(kernel log, errors, failed units) and says what is wrong, in plain words, in the journal. A part the
phone never had working is a `gap` in the manifest: still measured, not ill health; a gap that starts
to pass is said.

### Keeping a lab phone awake: `lpoco-health awake`

    lpoco-health awake [--once]           hold the kernel wake lock `lpoco-awake` (every 5 s; --once: one step)

The package's unit `lpoco-awake.service` runs it (the device enables it; the package only carries it). The lock is
held **always**, so sxmo's autosuspend, which blocks on it, never suspends a phone on USB (a suspend that completes
while the port blips leaves the idle phone silent). Deliberate suspends (`suspend cycle`) write `/sys/power/state`
themselves and are not held back. With `/etc/lpoco/allow-autosleep` (a battery test) the lock is held only while the
UDC is `configured` or while a gate the manifest names says unsafe: `[health] awake_gates = ["bluetooth-not-up"]`
(hci0 not UP, or an rfkill stuck in D state). A gate the package does not know is named on stderr; with autosleep allowed it is refused (exit 2, nothing held), otherwise the phone stays awake. The lock was `devlab-usb` and the allow file `/etc/devlab-allow-autosleep` before: a phone moving over drops the old file and script by hand.
The suite's check `awake-wakelock-held` reads the lock and the unit, not an installed script.

### One controlled suspend: `lpoco-health suspend`

    lpoco-health suspend plan             what a cycle would do, or why it would refuse - changes nothing
    lpoco-health suspend cycle [--seconds 15] [--settle 2]
                                          one suspend (s2idle) ended by an RTC wake alarm, counted

The device declares in its manifest's `[health]` its **way back** - `suspend_guard`, a command that succeeds while
the phone's watchdog is armed (surya: the kept-armed watchdog) - and the wake sources to quiet for the cycle,
`suspend_quiet` (wakeup files, e.g. a UART's `power/wakeup`, put to `disabled` and put back after, however the cycle
ends), and the services to stop for it, `suspend_stop` (systemd units - surya: `sensorspd` - stopped, in the order named, before the suspend and started again after it, last stopped first, however the cycle ends; the report names them in `stopped` and any that would not start in `not_started`; the plan names them in `stop`). **Without a way back, or with one that is not armed, the cycle is refused** (exit 2, nothing suspended). `--before CMD` and `--after CMD` run a one-off command (`sh -c`) around the suspend - `before` once the services are stopped and ahead of the RTC alarm (it failing refuses the cycle, exit 2), `after` once the counters are read and before the services start again, also when the cycle is refused after `before` ran; an `after` that fails is reported in `hooks` only and does not change the exit code; the report names both in `hooks` - so a probe (dyndbg, irq trace on and off) needs no wrapper. It
reads before and after the SoC's own counters (qcom_stats), the suspend counter, the cpu idle states (cluster states
among them, summed over the cpus) and the wakeup sources, prints one JSON report and keeps it as this boot's cycle
(`/var/lib/lpoco-health/suspend/cycle.json`); exit 0 when the SoC slept, 1 when it did not. The check `soc-sleeps`
`--seconds` is at least 5 (the RTC counts whole seconds); a wake long before the alarm is reported `woke_early`, not slept; a declared source that cannot be quieted is named in `not_quieted`; sources left quiet by a cut-off cycle are put back by the next. `plan` always exits 0: read `would_cycle`. The check `soc-sleeps`
reads that cycle - of this boot, no other - and says pass or fail where it used to skip. The cycle is never part of
a scheduled run: somebody (a keeper) asks for it. Every path lies under `LPOCO_HEALTH_ROOT` (empty on the phone), which
is how the fixture tests run it on a made-up phone.

### Detected conditions and chosen switches

What the phone finds out by itself is a **condition**, worked out at the start of every run by
`conditions/*.sh` (no SIM card, no modem, no network): a check says `skip_on no-sim TEST COMPONENT && exit 0`
and reports "not measured: no SIM card". Nothing is written down - put the SIM back and the next run measures
again. What somebody *chooses* is a **switch**: `lpoco-health off TEST --reason WORDS [--by WHO]`, `lpoco-health
on TEST`, `lpoco-health list` (tests, switches with who and when, conditions found now), kept in
`/etc/lpoco-health/switches` on the phone and reported as `skip: switched off: REASON (by WHO, WHEN)`. A test
is never off without a reason. From the lab: `lpoco lab suite off|on|list`, and the MCP door's
`health_switch_off`, `health_switch_on`, `health_switches`. The phone's state never goes into docs; docs keep
the hardware's facts and the results. The command line is declared once with argenv (`health/src/cli.rs`).
`lpoco lab suite put now` moves a phone from the old `devlab-health` (reports, month's data ledger, once-a-day
marks) to `lpoco-health` and leaves `devlab-health` as another name of the program.

The package is released by release-plz (a `lpoco-health-vX.Y.Z` tag) and published by the package
conveyor (`packaging/`, `.github/workflows/packages.yml`) at https://linux-phone-oco.github.io/lab-tools/,
for postmarketOS v26.06 (Alpine 3.24): any postmarketOS phone can take it with the lines on that page.
sxmo-utils is not built here any more: surya runs Alpine community's own (its `sxmo-utils-v*` tags stay in history).

`lpoco lab kernel install NAME VMLINUZ DTB MODULES.tgz | try NAME | log | remove NAME` - the safe test-kernel boot (one try by boot
counting, a stalled boot saves its log and falls back to stock), proven on surya.

## The phone for any agent

`lpoco lab mcp` is devlab's door for agents: an MCP server on standard input and output, with the tools
`run` (commands as the user or root, no quoting layer), `put` and `get` (files), `health_report` and
`health_run` (the phone's own suite, with its verdict), `kernel_install`, `kernel_try` (once, falling
back by itself, then the health of the boot it came to), `kernel_status`, and `reboot` (and wait, and
the new boot's health). Each calls what the command line calls.

`lpoco mcp` is the family's one door: the same server, every program's tools under its name - `lab_run`, `lab_put`,
`lab_get`, `lab_kernel_*`, `lab_reboot` (the bare names above still answer), `health_*`, and `bringup_image_info` and
`bringup_image_check` (boot images read and counted against the manifest).

    claude mcp add --scope user phone -- devlab --device /path/to/LAB/device.toml mcp
    claude mcp add --scope user lpoco -- lpoco mcp

## tone

Make a test tone, and find it in a recording - a device's speaker tests its microphone:

    lpoco tone make --freq 1000 --level -20 tone.wav    # gentle: -20 dBFS by default
    lpoco tone find --freq 1000 recording.wav           # JSON; exit 0 when found, 1 when not

`find` measures the frequency by the Goertzel algorithm (Hann window) and compares it with
neighbouring frequencies, off the tone and its harmonics: found means it stands `--min-snr` dB
(20 by default) above them.

On the phone the same crate answers as `lpoco-health tone make|find` (no clap: the runner stays small;
`--raw` makes or reads s16le mono as `pacat`/`parecord --raw` do, `--from`/`--to` keep the seconds where
the tone sounds): a check that plays a tone and listens hands the numbers to this tested code instead of
awk, and reads its JSON line with `json_field` from the suite's `lib.sh`. Analysis that counts, times or
keeps a state joins lpoco-health as a subcommand the same way (decision 2026-11-17-health-checks-shell-and-rust).

Development: `nix-shell` (nixpkgs pinned, as in lab-agent), `cargo test`. The command line uses clap
for now; moving it onto argenv, as lab-agent uses, is a contained change.

## The phone's way, held for agents

`lpoco lab guard tool` is what Claude Code's PreToolUse hook on the lab calls, so that changing the phone
outside devlab is refused when it is tried, not remembered: a Bash command that reaches a phone by ssh,
scp, sftp, rsync or sshpass exits 2, saying why. So does raw `fastboot` with any verb but `devices`, `getvar` and
`help` (and `-w` or `--set-active` with any): it would change whichever phone it reaches. It needs no manifest.

The ways every project shares - no git push to main, no Python, the practices and the inbox at every
session start - are the hooks of the ideal-agent plugin (https://github.com/grenudi/ideal-agent), installed
on the lab since 2026-11-17; devlab keeps only what is its phone's.

### How the phone is reached

`[connection]` says how (decision 2026-11-19-phone-reached-by-its-declared-transport): `transport = "ssh"` (the default: the phone user's password from `PHONE_PASS` or `~/.config/devlab/CODENAME/password`, sudo for root), `"telnet"` (the phone's own shell at `shell_address` (else the first address) and `shell_port`, no login, a script run as it is) or `"adb"` (`adb -s SERIAL shell`, the serial being `[fastboot] serial`, else the first `[[host.gadget]]` serial; `addresses = []` is fine). `login = "key"` (ssh by `identity = "/abs/path"`, a private file of mode 600: `ssh -i PATH -o IdentitiesOnly=yes -o BatchMode=yes`, no password; root is the login itself when `user = "root"`, else `sudo -n`; a missing or group/world-readable identity is refused naming it) may be declared instead of `"password"` for ssh. `login = "password"` (ssh) or `"none"` (telnet, adb) follows the transport and may be left out; a pair that cannot work is refused. `lpoco lab phone sh|root|put` and `lpoco lab suite` go through it: on a device with no login `suite` says `not applicable: device.toml declares no login` (exit 0 for `latest` and `list`, 2 for the rest) and asks for no password. Limits: over telnet and adb `root` is the same shell as `sh` (adb's shell is the phone's `shell` user unless adbd runs as root), a script must not exit the shell or read standard input over telnet, and a file is not carried over telnet. Not yet through the transport: `boot_once` by `fastboot boot`, the `lpoco-attended` library steps.

A phone's addresses are the ones devlab has met: every manifest it loads leaves its addresses in
`~/.local/state/devlab/CODENAME/addresses`, beside the Wi-Fi address it learned. In the lab's
`~/.claude/settings.json`:

    "hooks": {
      "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "~/.local/bin/devlab guard tool", "timeout": 10}]}]
    }

## What must never be touched

A phone's manifest names what harms it to touch, each with what it did:

    [[hazard]]
    path = "/sys/kernel/debug/gpio"
    why = "read while LPASS is idle it stalls the bus and the phone hangs until a person holds power"

devlab refuses any command it carries to the phone - `lpoco lab phone sh|root`, the MCP door - that names
one, saying why; the phone is never reached for it.

`lpoco lab kernel keep NAME now` makes an installed test kernel the phone's default: its entry sorted
ahead of stock with three tries and boot blessing on, so its first good boot is blessed (the default
from then on), and three bad boots in a row put stock back by themselves - the initramfs timeout still
guards each boot. `lpoco lab kernel unkeep NAME now` gives the default back to stock; the kernel stays
installed. One kernel is kept at a time.

`lpoco lab kernel try NAME --dtb FILE` boots installed kernel NAME - its kernel, initramfs and modules -
once with another device tree, as the entry NAME-dt: a device-tree change is one boot, even on the kept
kernel. A try is judged by the entry the bootloader says it booted (LoaderEntrySelected).

## A phone over fastboot

A phone that speaks only fastboot until Linux runs on it (google-walleye), or any phone sitting in its
bootloader, is reached by its manifest's serial - never "whichever phone is in fastboot", as a lab may
hold two:

    [fastboot]
    serial = "FA81N1A00790"   # as fastboot devices lists it
    writes = false            # the default: devlab flash refuses this phone; say true only by a decision
    slot_switch = false       # the default: devlab fastboot set-active refuses; true only by a decision

`lpoco lab fastboot state` says whether it is in fastboot and what its bootloader says of itself (product,
unlocked, slot-count, current-slot, versions); `getvar NAME` reads one. `boot now BOOT.IMG` boots an
Android boot image from memory - devlab writes nothing - reading the slots' flags before, then saying where
the phone went (back to fastboot, Android's adb, Linux on its USB network) and what the flags became: the
bootloader keeps A/B flags of its own and may change them on a failed boot (google-walleye, 2026-11-17: a RAM
boot ended on slot b with slot a unbootable). `set-active now SLOT` changes the active slot (metadata only)
where the manifest says `slot_switch = true`, by a recorded decision only. `reboot now` goes to the active slot, `--bootloader`
back to fastboot. None of them writes a partition or erases. The lease and the hold are the
phone's own (by codename), so one phone's keeper never blocks another's.

## The phone's keeper

One session owns a phone's flow at a time - its keeper, taking up the phone's queue (docs: `tad q
phone-queue`) one change at a time; devlab still holds the phone for every change. `lpoco lab keeper take
NAME` takes the lease (refused while another keeper is live), `renew NAME` as the keeper works, `release
NAME` as it ends, `status` says who keeps the phone. A lease left unrenewed for 90 minutes has lapsed: its
keeper ended or was cut off, and the next one may take it.

## A long job: `lpoco-lab job`

A build or a test that must outlive the session that starts it (a kernel build, an image, a sleep test) is a user
unit of the lab's own, `lpoco-job-NAME.service`, made with `systemd-run --user`. The unit's own process holds an
idle and sleep inhibitor for as long as the job runs, so the lab stays awake for the build and nothing else keeps it
awake. systemd is the runner: its journal is the log, a name is claimed once, no daemon, no queue.

    lpoco-lab job start NAME SCRIPT   run the executable SCRIPT (a name already claimed is refused)
    lpoco-lab job status NAME         running since T | finished ok at T | failed (exit N) at T | not found
    lpoco-lab job log NAME            the output of its current run
    lpoco-lab job stop NAME           end it if it runs, and free the name
    lpoco-lab job list                every job and its state

It needs no device manifest. The unit gets the caller's PATH and runs in the directory `start` was called from; a
script that needs more sources it. The user manager keeps its units after logout (lingering is on at the lab).

## lpoco: one front door

`lpoco NAME args...` finds `lpoco-NAME` beside itself or on PATH and runs it, returning its exit code
(`lpoco health latest`). The family is released together under one version (release-plz `version_group`):
a program whose `--version` differs from lpoco's is refused, naming both (`lpoco-attended` and `lpoco-manifest` are the exceptions: tagged only when they change, at their own version). `lpoco list` shows the
programs found with versions; `lpoco completions SHELL` and `lpoco man` come from the argenv declaration.
Lab-side only: the phone runs `lpoco-health` directly and gets no dispatcher.

## lpoco bringup: boot images on the host

`lpoco-bringup` (run as `lpoco bringup ...`) makes the files that bring a phone up, on the host; devlab puts them
on the phone. First piece, boot images with header v0, the header facts read from the `[boot]` table of the
device's `device.toml` (postmarketOS deviceinfo's words: `kernel_offset`, `ramdisk_offset`, `tags_offset`,
`pagesize`, `cmdline`, `partition_size`, `msm_id`, `os_version`, `os_patch_level`, ...):

    lpoco bringup image pack --kernel K [--dtb DTB] --ramdisk R --out IMG    byte for byte as mkbootimg writes it
    lpoco bringup image check IMG        count it against [boot]: magic, v0, size, offsets, os words, cmdline,
                                         the appended dtb and its qcom,msm-id; exit 1 on a miss
    lpoco bringup image info IMG         the header's facts as a [boot] table
    lpoco bringup image unpack IMG --out DIR

`--device FILE` (or `LPOCO_DEVICE`) names the manifest; `device.toml` in the current folder otherwise. The packer
is proven against mkbootimg by test (`bringup/tests/pack_v0.rs`; where android-tools is absent the test says
"not measured: no mkbootimg"). Swap, initramfs and the rest are their own items.

### lpoco bringup image ensure: an image by its sha256, from a store or a recipe

    lpoco bringup image ensure NAME [--device device.toml] [--cache DIR] [--seed DIR] [--source DIR]

Prints the Nix store path of `[images.NAME]` (its `sha256` and the file name of its `path` give it a fixed-output path:
`nix-store --print-fixed-path`). In the store: done. Else `nix copy --from` the file:// cache `--cache` (or
`LPOCO_IMAGE_CACHE`). Else a file under `--seed` (`LPOCO_IMAGE_SEED`) whose sha256 it is is taken into the store - how a
hand-built image enters once. Else it is built from `[images.NAME.recipe]` in Nix's sandbox as a fixed-output derivation
whose `outputHash` is the declared sha256, so Nix refuses a build that is not that image, and the result is copied to the
cache. A recipe is another image of the manifest with parts swapped in, each part named by its file's sha256:

    [images.z21]
    path = "images-z21/boot-z21.img"
    sha256 = "bdc0a891..."
    [images.z21.recipe]
    control = "z20"            # [images.z20] declares its sha256
    dtb = "f9f2ca5d..."        # also kernel, ramdisk (sha256 of the file), cmdline (text)

The derivation (`bringup/src/ensure.nix`) builds `lpoco-bringup` from `--source` (`LPOCO_SOURCE`; this version's release tag
from GitHub when none) with nixpkgs pinned in `nixpkgs.nix` (the pin `shell.nix` uses too) and runs `image swap`. The AVB footer
is still avbtool's, pinned to the AOSP revision the host's avbtool 1.4.0 is (nixpkgs' android-tools writes 1.3.0, whose
footer differs by a byte); `image-avb-footer-in-rust` removes it. Proof: `bringup/tests/ensure.rs` (where nix or avbtool is
absent it says "not measured"); measured on the lab 2026-11-23 for walleye's z21 from z20 and its dtb.

A recipe's `kernel` is a file by sha256; `[kernel_builds.NAME]` makes one from an `int/` tag, and `image ensure NAME` takes
the same ways (store, cache, seed) before building it:

    [kernel_builds.walleye]
    tag = "int/walleye/ufs-off/v7.3/1"
    rev = "41f3b5fd..."          # the commit the tag points at: Nix fetches this, a tag can move
    epoch = 1795421017           # the build's clock (KBUILD_BUILD_TIMESTAMP): the tag's own time
    configs = ["5424de9b..."]    # fragments merged after the defconfig, by sha256 (seeded or cached)
    patches = []                 # diffs applied with patch -p1, by sha256
    targets = ["Image.gz"]       # make targets; output = "arch/arm64/boot/Image.gz"; dtc_flags = "-@"
    sha256 = "..."               # what the build must be: Nix refuses any other bytes

`bringup/src/kernel.nix` builds the tag's commit with the pinned nixpkgs' clang and lld (`LLVM=1`), the user `lpoco`, the
host `lab`, the counter 1 and the clock from `epoch`: the knobs whose absence made hand builds differ (research
image-recipes-how-it-stands, measurement 4). A recipe whose `kernel` sha256 a `[kernel_builds]` row declares gets it built
when nothing holds it. Proof: `bringup/tests/ensure_kernel.rs` (a stand-in tree with the kernel's make interface; the real
tree is measured on the lab and recorded as research).

### lpoco bringup kernel: a phone's test kernel from its named list

    lpoco bringup kernel assemble LIST --lists kernel/topics.toml --repo CLONE --records DOCS [--reproduces COMMIT]
    lpoco bringup kernel publish TAG --repo CLONE [--device device.toml]

`assemble` runs the record checks lab-tools owns, then git-rhizoid's `rhizoid assemble` (which does the merging, rerere
and tagging: decision 2026-11-23-structure-git-flows-run-on-rhizoid) and prints its JSON report. A topic or backport
branch with no upstream-item row (`tad q carried`, run in `--records`), a commit with no `TAG(item):` subject, or one whose
tag disagrees with its item's state is named and the run ends 1 before rhizoid starts. A frozen list names `archive/NAME` branches (the series as an image was built on it): their name carries no item, so each commit's subject is checked against the carried items (a branch with no commit over the base is refused: nothing says what it carries). `--reproduces COMMIT` (or `LPOCO_KERNEL_REPRODUCES`) asks rhizoid that the tag's tree equal COMMIT's; the verdict (`reproduces COMMIT: equal`, or `differs in` and the paths) is printed after the report, and a difference ends 1, and a comparison asked for and not reported ends 2. `publish` pushes `TAG` to `origin` and
ends rhizoid's worktree for it (`--done TAG --force`) only when an `[images.NAME] kernel_tag` of the manifest names it; else it
ends 1 and pushes nothing. `--rhizoid` / `LPOCO_RHIZOID` and `--tad` / `LPOCO_TAD` name the programs. rhizoid is a host tool of
the lab (`lab.toml`); it is not installed by hand, and `lpoco host` cannot yet install a cargo-built tool at a pin.

## Install on a lab host

From the release tags, with cargo; the family has one version, so the tags of one release agree:

    cargo install --locked --git https://github.com/linux-phone-oco/lab-tools --tag lpoco-v0.2.0 lpoco
    cargo install --locked --git https://github.com/linux-phone-oco/lab-tools --tag lpoco-bringup-v0.2.0 lpoco-bringup

`lpoco --version` is the tag's version and `lpoco list` shows bringup. Proof: `lpoco/tests/install_from_tag.rs`
installs the newest release into a clean root and checks both, and that bringup refuses a wrong pin.

## lpoco layer: a phone's device layer as packages

`lpoco-layer` (run as `lpoco layer ...`) packs the `[layer]` of a device's `device.toml` (typed in lpoco-manifest) with
[nfpm](https://nfpm.goreleaser.com), which it needs on PATH (`LPOCO_NFPM` names another):

    lpoco layer pack --manifest device.toml --format apk|deb|rpm|archlinux --out DIR [--pkg-version V] [--kernel-build DIR]

Per format it writes `NAME-config` (the declared files and, for `efi-kernel-install`, `/usr/lib/kernel/devicetree` naming
the first dtb: the same files in every format; dependency tokens written the way the format writes them) and,
`NAME-boot` (a systemd preset enabling the layer's units, not for apk; the initramfs module list as an initramfs-tools hook,
a dracut drop-in or a mkinitcpio drop-in; for apk only the mkinitfs module list, as globs under `/lib/modules/KERNEL`, and a scriptlet adding the feature to `mkinitfs.conf` once, depending on `mkinitfs`). With `--kernel-build DIR` (a finished build: `Image`, `kernel.release`, `dtbs/`, optionally `modules/` = the tree for `/usr/lib/modules/RELEASE`) it also writes `linux-NAME` for arm64: `vmlinuz`, the declared dtbs under `dtb/` where kernel-install reads them, and the modules. It builds no kernel; a missing declared dtb, an `Image` or a release that is not one word is refused. Each package's nfpm config is kept beside it. Proof: `layer/tests/pack.rs`
from fixtures; with nfpm, research `test-layer-apk-and-deb.sh` (docs) passes on a fixture of surya's shape.
For apk, `NAME-config` also holds `/etc/deviceinfo` (Nura's override of the device package's deviceinfo, sourced by mkinitfs and boot-deploy) written from `[boot]` and `[device]`: codename, aarch64, header version, generate_bootimg, page size, the flash offsets, the cmdline; a codename or cmdline a shell would run (quote, `$`, backtick, backslash, newline) is refused. Open: current pmaports marks `deviceinfo_kernel_cmdline` obsolete for `/usr/lib/kernel-cmdline.d/*.conf` (not yet confirmed against boot-deploy), and flash_method/dtb are not written.
Not yet: building the kernel from `[layer.kernel]` tree and tag (the package takes a finished build),
`export`.

## lpoco host: the lab host prepared from declarations

`lpoco-host` (run as `lpoco host ...`) prepares the machine the lab runs on from two declarations: the lab's own
`lab.toml` (`--lab FILE` / `LPOCO_LAB`; default `~/Projects/lab-machine/lab.toml`: `devices` a glob or a list of globs, default
`~/Projects/linux-phone-oco/*/device.toml`; `root` a prefix; `[tools]` each host tool at its pin) and each device's
`device.toml` `[host]` (`modules`, and a `[[host.gadget]]` per `serial` with `access = ["block", "tty"]`).
A top-level `gadget_group = "users"` in `lab.toml` (a group the lab user already holds, so no re-login) adds `GROUP` and `MODE="0660"` to
every gadget rule line beside `TAG+="uaccess"`: access then does not wait on the seat's ACL, nor need the active seat session.
`check` says which group and whether the lab user is in it.
A gadget's `net_name = "lpoco-NAME"` gives its USB network a stable interface name (a udev rule by serial, at the next plug-in);
`net_address = "10.55.0.1/24"` (IPv4 CIDR, needs `net_name`) adds the host's address on it as a link config for the manager the host
runs: a NetworkManager keyfile (`/etc/NetworkManager/system-connections/NAME.nmconnection`, mode 0600, `nmcli connection reload`)
when `/run/NetworkManager` exists, else a systemd-networkd file (`/etc/systemd/network/70-NAME.network`, `networkctl reload`) when
`/run/systemd/netif` exists; with neither, plan says so and writes nothing. Planned in the diff, written only by `prepare now`.
The NetworkManager directory is root-only, so the lab user cannot read an existing keyfile: it is planned as new and rewritten at each `prepare now` (same content, one `nmcli connection reload`).
A gadget's `usb_id = "18d1:4ee0"` (vendor:product, four hex digits each) is what the phone shows on the host's USB bus when that gadget is up; `host` does nothing with it, the attended procedures (`wait_for_gadget`) read it.
A device that wants its health watched says `watch = true` in its `[host]` (`devlab-watch@NAME-lab` user unit, enabled and restarted by prepare);
without it no watcher is enabled for it, and an instance already enabled for a device that does not ask is named in the plan and left running.

    lpoco host [plan]        a diff per file and the exact root commands; changes nothing
    lpoco host prepare now   write /etc/udev/rules.d/70-lpoco-CODENAME.rules and /etc/modules-load.d/lpoco.conf,
                             reload and trigger udev, modprobe, install the tools - only where they differ
    lpoco host check         the lab user opens each declared node that is present; each tool is at its pin; the phone share is started and stopped
                             by the lab user with no password (the polkit rule's own test), its interfaces found from the device's address
    lpoco host revoke now    removes /etc/sudoers.d/lab-agent, the standing grant: refuses unless check passes, the share was measured and the
                             root prefix does not name lab-agent; `prepare now` ends with the same share check
    lpoco host follow [now]  a family release newer than lab.toml's pin: with no keeper lease held on any phone, install it (release assets, hash checked), run its own
                             `check` and `plan` against lab.toml with the pin moved, and land the pin (a clone, a branch, `land`, signed lab-host-follow);
                             a failing check or a plan that lists a root command puts the previous programs back and lands nothing; without `now` it says what it would do
                             with the family current, an installed `[tools.NAME]` (rhizoid) whose installed copy is not at its pin is installed at once, with no lease taken or waited for (it never drives a phone),
                             and checked (no pin landed: the pin is what moved); a failing check puts it and its stamp back; a tool never installed is prepare's

`[follow]` may also say `window = "02:00-05:00"` (local time, may cross midnight; `LPOCO_CLOCK` names the time for tests): outside it `follow now` only says it waits.

`lab.toml` `[follow]` with `every = "15min"` makes `plan` list, and `prepare now` write and enable, the user timer `lpoco-host-follow.timer` that runs `lpoco-host follow now`. A phone's keeper lease is `~/.local/state/devlab/CODENAME/keeper.lease/holder`, held while renewed in the last 90 minutes. `LPOCO_RELEASES` names the releases list (GitHub's API when none is given).

A tool pinned by release (`[tools.NAME]` with `repository`, `tag`, `asset`) may ship user timers: `timers = ["rhizoid-bump"]` makes `prepare now` write the tag's `packaging/systemd/NAME.service` and `NAME.timer` as user units and enable the timer, and `env_file = ".config/rhizoid/bump.env"` with `env = { RHIZOID_GRAPH_CLONE = "~/Projects/linux-phone-oco/docs" }` writes that file (mode 600; a leading `~/` in a value is the home). `plan` fetches the tag's units to compare: one that differs is named `hand-changed`; a tag without the unit stops `prepare` with nothing written (prepare therefore needs the network when timers are declared; the units are fetched by tag, with no hash check, unlike the binary). The env file never appears in `plan` output, its mode is set back to 600, and a changed timer that was already installed is restarted.

Root is one prefix, `LPOCO_ROOT`, then the lab declaration's `root`, then `sudo` (the author's typed password: no standing grant, decision of 2026-11-18); the commands are a fixed
list the plan prints. `lab.toml` tools: `lpoco = "0.2.0"` installs the family from its release tags with cargo into
`~/.local`; `[tools.NAME]` with `git`, `path`, `rev`, `version` installs a script of another repository at a commit (`version` is a label only; the pin is `rev`)
into `~/.local/bin`, and stamps the rev in `~/.local/share/lpoco-host/rev-NAME`: a script is judged by that stamp (never by `--version`; on PATH without a stamp it is "rev not known"). A device whose `[tools] lpoco` the lab's pin does not meet is named and nothing is prepared for it.
`[tools.NAME]` with `repository` (`owner/name`), `tag`, `asset` installs another project's release binary the same way: fetched from `LPOCO_GITHUB` (GitHub when none is given) with its `.sha256`, installed only when the hash matches, never built, the tag stamped (the hash comes from the same release: it catches a damaged download, not a replaced release).

`lpoco = "X.Y.Z"` must be a full version (it names the release tags); a tool with no source yet is `oco-work = "unknown"`
and is listed as "not known yet". Codenames, serials, module and tool names that reach a root command or a file name are
refused unless letters, digits, `.`, `_`, `-`. `/etc/udev/rules.d/51-android.rules` is named as a wide rule and kept:
prepare never edits or removes it. An old `devlab`/`tone` first on PATH that is off the pin is named by plan and check.

## Licence

Our tools are MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](LICENSE-MIT), [`LICENSE-APACHE`](LICENSE-APACHE). Source files say so in
an `SPDX-License-Identifier` header; the rest is annotated by path in [`REUSE.toml`](REUSE.toml), and `.github/licence-check` (run by CI)
keeps new files covered.
The device-tree fixture is GPL-2.0-only OR BSD-2-Clause, the kernel's dual licence for device trees. Texts of the other licences: `LICENSES/`.

## Attended procedures: lpoco-attended

A procedure a person attends (flash, power-cycle, look at the screen) is a short entry file of named steps,
not a program: `attended/examples/attended.rs` is the one to copy (a rust-script with a git dependency on the
tag `lpoco-attended-vX.Y.Z`). `use lpoco_attended::prelude::*;`, then `procedure(name, device)` once, and in order
A device that runs `boot_once(NAME)` or `back_to_known_good()` declares `[kernels]` in its device.toml (`test = ["calls4"]`, optional `kept = "calls4"`, the kernel entries `lpoco lab kernel try` boots and the one kept as default): a name it does not list, or no table, ends the run (exit 2, dry run too) naming the key; the kept name is declared, not checked against the phone. With `boot = "fastboot"`, `pre_boot = ["CMD ARG", ...]` lists gates `boot_once` runs first, in order, as argv (split on spaces, no shell) in the device folder: the first that exits non-zero stops the boot before the phone is touched and is named with its output; a dry run lists them and runs none. A gate has no timeout: one that hangs holds the procedure. An `[images.NAME]` that declares a `[images.NAME.recipe]` and whose file is not on disk is not refused: `boot_once` asks `lpoco-bringup image ensure NAME` (the store, the file:// cache `LPOCO_IMAGE_CACHE`, the seed `LPOCO_IMAGE_SEED`, else a build of the recipe) and boots the store path it prints, its sha256 checked as for any image; the dry run's step line says which (`image z21 not on disk, by ensure: from the cache (PATH)`), asked of `lpoco-bringup image ensure NAME --dry-run`, which reads the store, the cache and the seed and changes none. A file on disk is booted as before. `LPOCO_ATTENDED_BRINGUP` names another lpoco-bringup (a test's stand-in).
`step(what, || ..)`, `check_from(what, source, || ..)` (a `check(what, || ..)` names no source and fails the dry run), `ask_author("what to do / what to check")`; physical acts named for a bench to do (`press_power`, `hold_button`, `unplug_usb`, `replug_usb`, `touch`, `swipe`, `look_for`, `listen_for`, `confirm`; `bench(impl Bench)` puts a relay, servo or camera on the bench to claim acts, the person does the rest; a dry run lists each doer; `examples/physical.rs`); `phone().sh(..)` and
`phone().root(..)` run a script on the phone through `lpoco lab phone sh|root` (`LPOCO_ATTENDED_DEVLAB` names another
program). The first failing step stops the run, is named, and the run exits 1; `ask_author` waits for "ok" (anything
else stops the run and is logged); `--dry-run` walks the list and touches nothing; `--log FILE` is the log a lane
files (appended to). `--until FIRST_PROMPT` runs the steps before the first `ask_author` and stops there with exit 0, never asking, so a keeper runs everything that needs no author; `--call-limit SECONDS` (or `LPOCO_ATTENDED_CALL_LIMIT`; no limit unless given, since devlab itself waits ten minutes or more for a boot) cuts a call to the phone that hangs, with what it started, and fails its step naming the limit; `--part NAME` runs one named part of a procedure (`part()` in the entry file, `unknown_part(name, &[..])` refuses the rest, exit 2; `examples/parts.rs`) - the parts a keeper may run alone hold no prompt and no risky step. A `Step` is the one step type for this runner and the unattended one; library steps (`steps::`)
return a `Step`. Not covered: a step that hangs when no `--call-limit` is given (the log names the step that began).

A procedure reads its device's values by name, never from constants (decision 2026-11-19-attended-keys-in-the-manifest): `dev("health.battery_floor_mv")` is a key of the table that owns the fact, `proc("window")` a key of its own `[procedures.NAME]` (NAME is the procedure's name in lowercase words joined by `-`, `boot the pmicwdt kernel` is `boot-the-pmicwdt-kernel`); both give a `Val` (`.int()`, `.float()`, `.text()`). A missing key, or one of another kind, ends the run with exit 2 naming it, so `--dry-run` finds it before any step. The homes: `[connection]` `shell_port`, `shell_address`; `[tools]` `adb`, `fastboot` beside `lpoco`; `[health]` `battery_floor_mv` with `battery_source = "fastboot-getvar"`; `[boot]` `device_wait`, `back_wait` (seconds); `[images]` `dir` and `[images.NAME]` `path` (relative to the device repo) and `sha256`; `[procedures.NAME]` numbers and words only for what no other table owns (a second procedure that needs one lifts it to the owning table).
A device repo keeps its procedures as `procedures/NAME.rs` beside its `device.toml` (decision 2026-11-19-procedures-one-shape), and `lpoco attended` is their front door: `lpoco attended new NAME` writes `procedures/NAME.rs` for the repo's device (codename read from its device.toml; the repo's own `procedures/steps` crate, when there is one, is already in the cargo block; it refuses an existing file or a name that is not lowercase words, digits and hyphens), `lpoco attended run NAME [--dry-run] [--log FILE] [--device FILE] [--until FIRST_PROMPT] [--part NAME] [--call-limit SECONDS]` hands those options to the procedure and returns its exit code, and `lpoco attended check` dry-runs every procedure (each with its own throw-away log) and names each that fails, exit 1. `check --host` also runs the lab's installed lab program (`LPOCO_ATTENDED_DEVLAB`, devlab when none) on the repo's device.toml - `lpoco lab --version`, then `lpoco lab --device FILE keeper status`, each cut after 30 s - so a procedure is ready only when the host's own tools read its manifest; a refusal names the binary, its version and what it said. `LPOCO_ATTENDED_LIB=PATH` makes `new` depend on a checkout instead of this release's tag; `LPOCO_ATTENDED_RUST_SCRIPT` names the rust-script program.
A check names its source (decision 2026-11-23-a-stop-is-read-before-the-fix): `check_from(what, source, ..)` says where the value it expects came from - `research:ID` (a research row of the base; taken as named), a path ending `.md` (a doc), or a path under a `research` folder (text a phone printed, captured), read from the folder of the device's `device.toml`. The dry run, so `lpoco attended check`, exits 1 naming the check when the source is empty, the file is missing, holds nothing or is not text, is not under a `research` folder (a typed stand-in) or leaves the repository by `..` or an absolute path; `check(what, ..)` names none and is refused the same way, and `lpoco attended new` writes `research:ROW-ID`, which is refused until the row's id replaces it. A real run does not read the source. Not looked up: that a `research:ID` row exists in the base, and that a captured file holds what the check expects.

`lpoco-attended` has its own version, tagged only when `attended/` changes (it names `lpoco-manifest` by a released git tag, not by path). `lpoco-manifest` (`lpoco manifest schema`) is tagged only when `manifest/` changes, so that tag can be pinned with an edge; a manifest change still moves its path dependents, and the family moves together. The lab host does not install it: a procedure is run as a rust-script at that tag, and `lpoco attended` runs at whatever version is on PATH.

## Whether a device.toml is read: `lpoco-lab manifest check`

    lpoco-lab manifest check --device device.toml   read it as every command does; exit 1 naming file, version and reason

Parse and validate only: no state directory, no phone, no lease. A device repository's CI downloads the
`lpoco-lab` release asset the lab machine pins (`lab.toml`'s `lpoco`) and runs this on its `device.toml`, so a file the
host cannot read cannot land (device-template's `bin/check-device-toml.sh`).

### The shared clones, kept fresh: `lpoco-host clones`

`lab.toml` `[clones]` with `folders = ["~/Projects"]` and `every = "15min"` makes `plan` list, and `prepare now` write and
enable, the user timer `lpoco-host-clones.timer` that runs `lpoco-host clones now`. Each clone under a folder that is on
`main`, has an `AGENTS.md` at its top (a commons place: any other repository is named "not a commons place, left" and never touched, the author's own among them) and has no uncommitted tracked change is fast-forwarded to origin/main (`merge --ff-only`: nothing is reset or
removed, and git itself refuses a fast-forward that an untracked file would meet). Any other - dirty, detached, on another
branch, diverged - is left exactly as it is and named with why; a worktree is not a clone. `clones` without `now` only
fetches and says what it would do (a fresh `origin/main` is all it changes). The unit fails (exit 1) when a declared
folder cannot be read or when no clone could be fetched; a clone left alone is named, not a failure. The result is that a
clone the keeper starts from is at origin/main within the interval; a clone on another branch is not moved.
