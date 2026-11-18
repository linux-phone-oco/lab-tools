//! A device's manifest, `device.toml`, kept in its lab repository: what the tools need to know about
//! the phone. Secrets and what is learned at run time stay outside the repository.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "device.toml";

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Device {
    pub device: Identity,
    pub connection: Connection,
    #[serde(default)]
    pub tests: Tests,
    #[serde(default)]
    pub provision: Provision,
    #[serde(default)]
    pub flash: Flash,
    /// How the lab reaches the phone over fastboot, and whether it may write to it there
    #[serde(default)]
    pub fastboot: Fastboot,
    /// what the lab's ear listens for
    #[serde(default)]
    pub hear: Hear,
    /// what the phone's own health suite expects of it
    #[serde(default)]
    pub health: Health,
    /// What must never be touched on this phone, whoever asks: a command devlab carries to the phone
    /// (phone sh|root, the MCP door) that names one is refused, saying why
    #[serde(default)]
    pub hazard: Vec<Hazard>,
}

/// A path on the phone that harms it to touch - each with what it did, so the refusal says why.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Hazard {
    pub path: String,
    pub why: String,
}

impl Device {
    /// The first hazard a script names, if any: a refusal, in words.
    pub fn hazard_in(&self, script: &str) -> Option<String> {
        self.hazard
            .iter()
            .find(|h| script.contains(&h.path))
            .map(|h| {
                format!(
                    "refused: the commands touch {} on {} - {}",
                    h.path, self.device.codename, h.why
                )
            })
    }
}

/// The phone as fastboot knows it. Every fastboot call devlab makes names this serial, so a lab with two
/// phones in fastboot never reaches the wrong one; and nothing is written over fastboot (flash) unless the
/// manifest says `writes = true` - a phone whose slots must stay as they are says nothing, or false
/// (google-walleye: decision 2026-11-17-walleye-fastboot-first).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Fastboot {
    /// As `fastboot devices` lists it
    #[serde(default)]
    pub serial: String,
    /// Whether devlab may write partitions over fastboot (devlab flash); false unless said
    #[serde(default)]
    pub writes: bool,
    /// Whether devlab may change the active slot (devlab fastboot set-active: the bootloader's metadata, no
    /// partition written); false unless a recorded decision says so
    #[serde(default)]
    pub slot_switch: bool,
}

/// How an image goes onto the phone, and what is baked into it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Flash {
    #[serde(default = "default_sector")]
    pub sector_size: u32,
    #[serde(default = "one")]
    pub esp_partition: u32,
    #[serde(default = "two")]
    pub root_partition: u32,
    #[serde(default = "boot_part")]
    pub bootloader_partition: String,
    #[serde(default = "userdata_part")]
    pub image_partition: String,
    /// Where the device tree lives in the image (esp:/... or root:/...), for --dtb and the fingerprint
    #[serde(default)]
    pub dtb: String,
    #[serde(default)]
    pub overlay: Vec<Overlay>,
    /// Services masked in the image
    #[serde(default)]
    pub mask: Vec<String>,
    /// Services enabled on first boot - always, even --as-is (the lab must reach the phone)
    #[serde(default)]
    pub enable: Vec<String>,
}

impl Default for Flash {
    fn default() -> Self {
        Flash {
            sector_size: default_sector(),
            esp_partition: 1,
            root_partition: 2,
            bootloader_partition: boot_part(),
            image_partition: userdata_part(),
            dtb: String::new(),
            overlay: Vec::new(),
            mask: Vec::new(),
            enable: Vec::new(),
        }
    }
}

fn default_sector() -> u32 {
    512
}
fn one() -> u32 {
    1
}
fn two() -> u32 {
    2
}
fn boot_part() -> String {
    "boot".into()
}
fn userdata_part() -> String {
    "userdata".into()
}

/// A file baked into the image: from the lab repository, to esp:/... or root:/...; skipped unless
/// `if_exists` (also esp:/... or root:/...) exists in the image.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Overlay {
    pub from: String,
    pub to: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub if_exists: Option<String>,
}

/// What a freshly flashed phone gets.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Provision {
    /// The package list, relative to the manifest
    #[serde(default = "default_packages")]
    pub packages: String,
    /// Files to install - `{compatible}` is the phone's device-tree name
    #[serde(default)]
    pub install: Vec<Install>,
}

fn default_packages() -> String {
    "provision/packages.txt".into()
}

impl Default for Provision {
    fn default() -> Self {
        Provision {
            packages: default_packages(),
            install: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Install {
    pub from: String,
    pub to: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    /// A command on the phone that must succeed for the file to go on (empty: always)
    #[serde(default)]
    pub when: String,
}

fn default_mode() -> String {
    "644".into()
}

/// What the phone's own health suite expects of this phone (lpoco-health, on the phone). Every
/// field is a fact about the hardware; what it says becomes /etc/devlab/health.env there.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Health {
    /// Sound cards that must be registered: names as /proc/asound/cards shows them
    #[serde(default)]
    pub sound_cards: Vec<String>,
    /// Signal processors that must be running: remoteproc name -> the component that rests on it
    #[serde(default)]
    pub remoteprocs: std::collections::BTreeMap<String, String>,
    /// Kernel warnings known at boot (file:line), each with its reason beside it in the manifest
    #[serde(default)]
    pub known_warnings: Vec<String>,
    /// Units known to fail on this phone, each with its reason beside it in the manifest
    #[serde(default)]
    pub known_failed_units: Vec<String>,
    /// Parts this phone does not have at all (wifi, modem, bluetooth, nfc, fingerprint, camera, jack,
    /// display, touchscreen, haptics, sensors, usb-otg): skipped, never failing
    #[serde(default)]
    pub absent: Vec<String>,
    /// Components this phone has never had working (docs component ids), each with its reason beside it:
    /// still measured and reported, but a failure there is a known gap, not ill health
    #[serde(default)]
    pub gaps: Vec<String>,
    /// This phone's own checks, relative to the manifest: base/*.sh and shell/*.sh beneath it
    #[serde(default)]
    pub checks: Option<String>,
    /// Mobile data the health checks may spend in a month, in MB (250 when unsaid): the telephony
    /// layer's mobile-data check skips once the month has spent it (lpoco-health data)
    #[serde(default)]
    pub data_budget_mb: Option<u64>,
    /// Interrupts one 15 s suspended cycle may count in all (the suite check suspend-irq-quiet); unsaid, the
    /// check says not measured: the number comes from a reading of this phone, never a guess
    #[serde(default)]
    pub suspend_irq_max: Option<u64>,
    /// A command on the phone that succeeds while its watchdog is armed (the way-back check): the suite
    /// suspends only if it does; unsaid, it never suspends
    #[serde(default)]
    pub suspend_guard: Option<String>,
    /// Wakeup files (sysfs, e.g. a UART's power/wakeup) put to "disabled" for the controlled suspend cycle
    /// (lpoco-health suspend cycle) and put back after: the wake sources that would end s2idle at once
    #[serde(default)]
    pub suspend_quiet: Vec<String>,
    /// Gates that hold the keep-awake lock (lpoco-health awake) while autosleep is allowed, named by token:
    /// bluetooth-not-up (hci0 not UP, or an rfkill stuck in D state). The package knows the tokens; an unknown one is refused
    #[serde(default)]
    pub awake_gates: Vec<String>,
}

impl Health {
    /// The env file the suite sources on the phone. Values are written as single-quoted shell words.
    pub fn env(&self, codename: &str) -> String {
        let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let procs: Vec<String> = self
            .remoteprocs
            .iter()
            .map(|(n, c)| format!("{n}:{c}"))
            .collect();
        format!(
            "# written by devlab from the device's manifest ([health]) - do not edit here\n\
             DEVICE={}\nSOUND_CARDS={}\nREMOTEPROCS={}\nKNOWN_WARNINGS={}\nKNOWN_FAILED_UNITS={}\n\
             ABSENT={}\nGAPS={}\nDATA_BUDGET_MB={}\nSUSPEND_IRQ_MAX={}\nSUSPEND_GUARD={}\nSUSPEND_QUIET={}\nAWAKE_GATES={}\n",
            q(codename),
            q(&self.sound_cards.join(";")),
            q(&procs.join(" ")),
            q(&self.known_warnings.join(" ")),
            q(&self.known_failed_units.join(" ")),
            q(&self.absent.join(" ")),
            q(&self.gaps.join(" ")),
            q(&self.data_budget_mb.unwrap_or(250).to_string()),
            q(&self
                .suspend_irq_max
                .map(|n| n.to_string())
                .unwrap_or_default()),
            q(self.suspend_guard.as_deref().unwrap_or("")),
            q(&self.suspend_quiet.join(" ")),
            q(&self.awake_gates.join(" ")),
        )
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Tests {
    /// The checks script, relative to the manifest; run on the phone as root, one JSON line per check
    pub checks: String,
}

impl Default for Tests {
    fn default() -> Self {
        Tests {
            checks: "tests/automatic/checks.sh".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Identity {
    /// The postmarketOS-style codename, e.g. "xiaomi-surya"
    pub codename: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Connection {
    pub user: String,
    /// Addresses to try, in order (USB networking first, as a rule)
    pub addresses: Vec<String>,
}

impl Device {
    pub fn parse(text: &str) -> Result<Device> {
        let d: Device = toml::from_str(text).context("reading device.toml")?;
        if d.connection.addresses.is_empty() {
            bail!("device.toml: connection.addresses is empty");
        }
        Ok(d)
    }

    /// The manifest at `path`, or found by walking up from `start` as cargo finds Cargo.toml.
    pub fn load(path: Option<&Path>, start: &Path) -> Result<(Device, PathBuf)> {
        let file = match path {
            Some(p) => p.to_path_buf(),
            None => start
                .ancestors()
                .map(|d| d.join(MANIFEST))
                .find(|f| f.is_file())
                .with_context(|| {
                    format!(
                        "no {MANIFEST} here or above {} (or pass --device)",
                        start.display()
                    )
                })?,
        };
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("reading {}", file.display()))?;
        Ok((Device::parse(&text)?, file))
    }

    /// Leave the manifest's addresses in the state, so that the lab's guard knows this phone when an agent
    /// reaches for it outside devlab (devlab guard). Best effort: a guard that knows less refuses less.
    pub fn remember_addresses(&self) {
        let file = self.state_dir().join("addresses");
        let text: String = self
            .connection
            .addresses
            .iter()
            .map(|a| format!("{a}\n"))
            .collect();
        if std::fs::read_to_string(&file).ok().as_deref() != Some(text.as_str()) {
            let _ = std::fs::create_dir_all(self.state_dir());
            let _ = std::fs::write(file, text);
        }
    }

    /// Private per-device configuration (the password): ~/.config/devlab/CODENAME/
    pub fn config_dir(&self) -> PathBuf {
        home().join(".config/devlab").join(&self.device.codename)
    }

    /// What is learned at run time (the phone's Wi-Fi address): ~/.local/state/devlab/CODENAME/
    pub fn state_dir(&self) -> PathBuf {
        home()
            .join(".local/state/devlab")
            .join(&self.device.codename)
    }
}

fn home() -> PathBuf {
    crate::cli::env()
        .home
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The addresses to try: the manifest's, then a remembered one not among them.
pub fn candidates(manifest: &[String], remembered: Option<&str>) -> Vec<String> {
    let mut all = manifest.to_vec();
    if let Some(r) = remembered.map(str::trim).filter(|r| !r.is_empty()) {
        if !all.iter().any(|a| a == r) {
            all.push(r.to_string());
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_that_touches_a_hazard_is_refused_saying_why() {
        let d = Device::parse(
            "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
             [[hazard]]\npath = \"/sys/kernel/debug/gpio\"\nwhy = \"stalls the bus; the phone hangs\"\n",
        )
        .unwrap();
        let no = d
            .hazard_in("uname -r\ncat /sys/kernel/debug/gpio | head")
            .unwrap();
        assert!(
            no.contains("/sys/kernel/debug/gpio on p - stalls the bus"),
            "{no}"
        );
        assert_eq!(d.hazard_in("cat /sys/kernel/debug/clk/clk_summary"), None);
        let none = Device::parse(
            "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]",
        )
        .unwrap();
        assert_eq!(
            none.hazard_in("cat /sys/kernel/debug/gpio"),
            None,
            "no hazard named, none refused"
        );
    }

    #[test]
    fn test_sounds_are_quiet_unless_the_manifest_says_otherwise() {
        let h = Hear::default();
        assert_eq!((h.level(), h.volume()), (-40.0, 25), "quiet by default");
        assert_eq!(h.env(), "TEST_TONE_DBFS='-40'\nTEST_VOLUME='25%'\n");
        let loud: Hear = toml::from_str("level_dbfs = 6.0\nvolume = 250").unwrap();
        assert_eq!(
            (loud.level(), loud.volume()),
            (0.0, 100),
            "never above full scale or 100%"
        );
        let said: Hear = toml::from_str("level_dbfs = -30.0\nvolume = 40").unwrap();
        assert_eq!((said.level(), said.volume()), (-30.0, 40));
    }

    #[test]
    fn health_expectations_become_the_suites_env_file() {
        let h: Health = toml::from_str(
            "sound_cards = [\"POCO X3\"]\nremoteprocs = { adsp = \"audio-speaker\", modem = \"modem-data\" }\nknown_failed_units = [\"qbootctl.service\"]\nabsent = [\"modem\"]",
        )
        .unwrap();
        let e = h.env("xiaomi-surya");
        assert!(e.contains("DEVICE='xiaomi-surya'\n"));
        assert!(e.contains("SOUND_CARDS='POCO X3'\n"));
        assert!(e.contains("REMOTEPROCS='adsp:audio-speaker modem:modem-data'\n"));
        assert!(e.contains("ABSENT='modem'\n"));
        assert!(
            e.contains("DATA_BUDGET_MB='250'\n"),
            "250 MB a month when unsaid"
        );
        assert!(
            e.contains("SUSPEND_IRQ_MAX=''\n") && e.contains("SUSPEND_GUARD=''\n"),
            "unsaid: no threshold and no guard, never a guessed number"
        );
        let q: Health =
            toml::from_str("suspend_irq_max = 40\nsuspend_guard = \"way-back.sh check\"").unwrap();
        let qe = q.env("p");
        assert!(qe.contains("SUSPEND_IRQ_MAX='40'\n"));
        assert!(qe.contains("SUSPEND_GUARD='way-back.sh check'\n"));
        let sq: Health =
            toml::from_str("suspend_quiet = [\"/a/power/wakeup\", \"/b/power/wakeup\"]").unwrap();
        assert!(sq
            .env("p")
            .contains("SUSPEND_QUIET='/a/power/wakeup /b/power/wakeup'\n"));
        assert!(e.contains("SUSPEND_QUIET=''\n"));
        assert!(e.contains("AWAKE_GATES=''\n"), "no gate unless named");
        let ag: Health = toml::from_str("awake_gates = [\"bluetooth-not-up\"]").unwrap();
        assert!(ag.env("p").contains("AWAKE_GATES='bluetooth-not-up'\n"));
        let tight: Health = toml::from_str("data_budget_mb = 1").unwrap();
        assert!(tight.env("p").contains("DATA_BUDGET_MB='1'\n"));
        assert!(
            toml::from_str::<Health>("sound_card = []").is_err(),
            "a misspelt field is refused"
        );
    }

    const SURYA: &str = "[device]\ncodename = \"xiaomi-surya\"\n[connection]\nuser = \"user\"\naddresses = [\"172.16.42.1\"]\n";

    #[test]
    fn a_manifest_reads() {
        let d = Device::parse(SURYA).unwrap();
        assert_eq!(d.device.codename, "xiaomi-surya");
        assert_eq!(d.connection.addresses, ["172.16.42.1"]);
    }

    #[test]
    fn a_manifest_without_addresses_is_refused() {
        assert!(Device::parse(
            "[device]\ncodename = \"x\"\n[connection]\nuser = \"u\"\naddresses = []\n"
        )
        .is_err());
    }

    #[test]
    fn the_remembered_address_comes_last_and_once() {
        let m = vec!["172.16.42.1".to_string()];
        assert_eq!(
            candidates(&m, Some("192.168.1.5\n")),
            ["172.16.42.1", "192.168.1.5"]
        );
        assert_eq!(
            candidates(&m, Some("172.16.42.1")),
            ["172.16.42.1"],
            "not twice"
        );
        assert_eq!(candidates(&m, None), ["172.16.42.1"]);
    }

    #[test]
    fn the_manifest_is_found_by_walking_up() {
        let root = std::env::temp_dir().join(format!("devlab-test-{}", std::process::id()));
        let deep = root.join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(root.join(MANIFEST), SURYA).unwrap();
        let (d, f) = Device::load(None, &deep).unwrap();
        assert_eq!(d.device.codename, "xiaomi-surya");
        assert_eq!(f, root.join(MANIFEST));
        std::fs::remove_dir_all(root).unwrap();
    }
}

/// The tones the lab's ear listens for (`devlab hear`); none named means one on each side. How loud
/// any test sound may be is said here too, quiet by default: a phone in a home must never shout - a
/// full-scale tone at night once woke the author. Every check that plays sound reads the same two
/// values (TEST_TONE_DBFS, TEST_VOLUME in the phone's health.env).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Hear {
    #[serde(default)]
    pub tone: Vec<HearTone>,
    /// the tone's level in the file, dBFS (at most 0)
    #[serde(default = "quiet_dbfs")]
    pub level_dbfs: f64,
    /// the phone's output volume while a test sound plays, percent (at most 100)
    #[serde(default = "quiet_volume")]
    pub volume: u32,
}

fn quiet_dbfs() -> f64 {
    -40.0
}

fn quiet_volume() -> u32 {
    25
}

impl Default for Hear {
    fn default() -> Self {
        Hear {
            tone: Vec::new(),
            level_dbfs: quiet_dbfs(),
            volume: quiet_volume(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct HearTone {
    /// the check's name
    pub name: String,
    /// left, right or both
    pub channel: String,
    #[serde(default = "default_freq")]
    pub freq: f64,
    /// the component it speaks for
    #[serde(default = "default_component")]
    pub component: String,
}

fn default_freq() -> f64 {
    1000.0
}

fn default_component() -> String {
    "audio-speaker".into()
}

impl Hear {
    /// The level, never above full scale.
    pub fn level(&self) -> f64 {
        self.level_dbfs.min(0.0)
    }

    /// The volume, never above 100%.
    pub fn volume(&self) -> u32 {
        self.volume.min(100)
    }

    /// The lines every check that plays sound reads from the phone's health.env.
    pub fn env(&self) -> String {
        format!(
            "TEST_TONE_DBFS='{}'\nTEST_VOLUME='{}%'\n",
            self.level(),
            self.volume()
        )
    }

    pub fn tones(&self) -> Vec<HearTone> {
        if !self.tone.is_empty() {
            return self.tone.clone();
        }
        ["left", "right"]
            .iter()
            .map(|c| HearTone {
                name: format!("heard-{c}"),
                channel: (*c).into(),
                freq: default_freq(),
                component: default_component(),
            })
            .collect()
    }
}
