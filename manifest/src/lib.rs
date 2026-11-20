// SPDX-License-Identifier: MIT OR Apache-2.0
//! A device's manifest, `device.toml`, declared once: every table, every field, its doc beside it.
//! devlab, host and bringup read these types; a table or field not declared here is refused everywhere,
//! never ignored (decision 2026-11-18-device-manifest-schema-from-rust). The JSON Schema of the manifest is
//! projected from these types (`schema()`), committed as `device.schema.json` and checked against them.

pub mod boot;
pub mod images;
pub mod layer;

use schemars::JsonSchema;
use serde::Deserialize;

pub use boot::{Boot, BootGadget, Console, ShellWord};
pub use images::{Image, Images, ProcValue};
pub use layer::{
    BootMethod, FileKind, FirmwareMethod, Layer, LayerBoot, LayerFile, LayerFirmware,
    LayerInitramfs, LayerKernel,
};

/// The whole manifest. A program asks for the tables it needs; a table nobody declared is an error.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub device: Option<Identity>,
    pub connection: Option<Connection>,
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
    /// What must never be touched on this phone, whoever asks
    #[serde(default)]
    pub hazard: Vec<Hazard>,
    /// Config files the phone keeps outside any git, which the lab names when they drift (lpoco host check)
    #[serde(default)]
    pub user_config: Vec<UserConfig>,
    /// What the lab host needs for this device
    #[serde(default)]
    pub host: HostTable,
    /// The header facts of the phone's boot image
    pub boot: Option<Boot>,
    /// The device layer: what each package format is generated from
    pub layer: Option<Layer>,
    /// The tools this device's lab pins
    pub tools: Option<Tools>,
    /// The kernels `lpoco lab kernel` keeps on the phone beside stock
    pub kernels: Option<Kernels>,
    /// The images a procedure puts on the phone, by name
    pub images: Option<Images>,
    /// Values only one procedure uses, per procedure (a free table of numbers and words)
    #[serde(default)]
    pub procedures:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, ProcValue>>,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let mut m: Manifest = toml::from_str(text).map_err(|e| e.to_string())?;
        for (i, g) in m.host.gadget.iter_mut().enumerate() {
            if let Some(id) = &mut g.usb_id {
                let ok = id.split_once(':').is_some_and(|(v, p)| {
                    [v, p]
                        .iter()
                        .all(|h| h.len() == 4 && h.chars().all(|c| c.is_ascii_hexdigit()))
                });
                if !ok {
                    return Err(format!(
                        "host.gadget #{} (serial {:?}) usb_id {id:?} is not vendor:product in four hex digits each",
                        i + 1,
                        g.serial
                    ));
                }
                id.make_ascii_lowercase();
            }
        }
        if let Some(c) = &m.connection {
            c.validate()?;
        }
        if let Some(i) = &m.images {
            i.validate()?;
        }
        images::validate_procedures(&m.procedures)?;
        m.health.validate()?;
        if let Some(k) = &m.kernels {
            k.validate()?;
            k.validate_boot(&m)?;
        }
        for (i, c) in m.user_config.iter().enumerate() {
            if m.user_config[..i].iter().any(|o| o.path == c.path) {
                return Err(format!("user_config {} is declared twice", c.path));
            }
            c.validate()?;
        }
        if let Some(l) = &m.layer {
            l.validate()?;
            if l.boot
                .as_ref()
                .is_some_and(|b| b.method == BootMethod::AndroidBootimg)
                && m.boot.is_none()
            {
                return Err("layer boot method android-bootimg needs a [boot] table".into());
            }
        }
        Ok(m)
    }
}

/// The JSON Schema (draft-07) of `device.toml`, as pretty JSON with a closing newline.
pub fn schema() -> String {
    let s = schemars::generate::SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<Manifest>();
    let mut text = serde_json::to_string_pretty(&s).expect("a schema is JSON");
    text.push('\n');
    text
}

/// `[kernels]`: the test kernels kept on the phone as boot entries beside stock (`lpoco lab kernel try NAME`), and
/// the one kept as the default. A procedure reads the names from here instead of carrying them.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Kernels {
    /// the test kernels by name (lowercase letters, digits and `-`: devlab's rule; `+` is systemd-boot's try counter)
    #[serde(default)]
    pub test: Vec<String>,
    /// the test kernel kept as the default (it must be listed in `test`); none means stock is the default
    pub kept: Option<String>,
    /// how a test kernel is booted: `entry` (a boot entry on the phone, tried once: `lpoco lab kernel try`) or
    /// `fastboot` (the image of `[images.NAME]` booted from memory, nothing written: `lpoco lab fastboot boot now`)
    #[serde(default)]
    pub boot: KernelBoot,
}

/// How a test kernel gets booted (`[kernels] boot`).
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum KernelBoot {
    /// a boot entry kept on the phone beside stock
    #[default]
    Entry,
    /// an Android boot image booted from memory over fastboot, nothing written to the phone
    Fastboot,
}

/// A kernel entry's name: lowercase letters, digits and `-` (devlab's rule; `+` is systemd-boot's try counter).
pub fn is_kernel_name(n: &str) -> bool {
    !n.is_empty()
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl Kernels {
    /// A fastboot boot reads, for each test kernel, its image, and for the phone its fastboot serial and the gadget it
    /// then waits for: each is named when missing.
    fn validate_boot(&self, m: &Manifest) -> Result<(), String> {
        if self.boot != KernelBoot::Fastboot {
            return Ok(());
        }
        if m.fastboot.serial.trim().is_empty() {
            return Err("kernels: boot = \"fastboot\" needs [fastboot] serial".into());
        }
        if !m.host.gadget.iter().any(|g| g.usb_id.is_some()) {
            return Err("kernels: boot = \"fastboot\" needs a [[host.gadget]] usb_id (the gadget it waits for)".into());
        }
        for n in &self.test {
            if !m.images.as_ref().is_some_and(|i| i.named.contains_key(n)) {
                return Err(format!("kernels: boot = \"fastboot\" needs [images.{n}] (the image of test kernel {n:?})"));
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        for (i, n) in self.test.iter().chain(self.kept.iter()).enumerate() {
            if !is_kernel_name(n) {
                return Err(format!(
                    "kernels: {n:?} is not a kernel name (lowercase letters, digits and -)"
                ));
            }
            if i < self.test.len() && self.test[..i].contains(n) {
                return Err(format!("kernels: test lists {n:?} twice"));
            }
        }
        if let Some(k) = &self.kept {
            if !self.test.contains(k) {
                return Err(format!("kernels: kept {k:?} is not listed in test"));
            }
        }
        Ok(())
    }
}

/// `[tools]`: the pins of this device's lab.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tools {
    /// The release of the lpoco family the phone's pieces come from
    pub lpoco: Option<String>,
    /// The adb program of this lab (adb on the path when none is given)
    pub adb: Option<String>,
    /// The fastboot program of this lab (fastboot on the path when none is given)
    pub fastboot: Option<String>,
}

/// `devices = "glob"` is the lab's; these are the device's own `[host]` words.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Block,
    Tty,
}

impl Access {
    pub fn subsystem(self) -> &'static str {
        match self {
            Access::Block => "block",
            Access::Tty => "tty",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gadget {
    pub serial: String,
    pub access: Vec<Access>,
    /// A stable name for the gadget's USB network interface (CDC-NCM and friends), set by udev at plug-in: the kernel's own name follows the port.
    #[serde(default)]
    pub net_name: Option<String>,
    /// The host's address on that network, as IPv4 CIDR (`10.55.0.1/24`): written as a link config for the host's own network manager, only with the verb. Needs `net_name`.
    #[serde(default)]
    pub net_address: Option<String>,
    /// What the phone shows on the host's USB bus when this gadget is up, as `vendor:product` in four hex digits each (`18d1:4ee0`); kept lowercase.
    #[serde(default)]
    pub usb_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostTable {
    /// The device asks to be watched: prepare enables its `devlab-watch@` instance. Watching is the device's own choice, like its gadgets.
    #[serde(default)]
    pub watch: bool,
    #[serde(default)]
    pub modules: Vec<String>,
    /// Programs the device needs on the host's PATH: named by plan, judged by check, never installed (installing is a root pacman the author grants).
    #[serde(default)]
    pub commands: Vec<String>,
    /// Rust targets the device's tools are built for: named by plan, judged by check (a standard library on the host), never installed.
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub gadget: Vec<Gadget>,
}

/// A path on the phone that harms it to touch - each with what it did, so the refusal says why.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hazard {
    pub path: String,
    pub why: String,
}

/// The phone as fastboot knows it. Every fastboot call devlab makes names this serial, so a lab with two
/// phones in fastboot never reaches the wrong one; and nothing is written over fastboot (flash) unless the
/// manifest says `writes = true` - a phone whose slots must stay as they are says nothing, or false
/// (google-walleye: decision 2026-11-17-walleye-fastboot-first).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
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
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
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
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    pub from: String,
    pub to: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub if_exists: Option<String>,
}

/// What a freshly flashed phone gets.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
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
#[derive(Debug, Clone, Default, Deserialize, PartialEq, JsonSchema)]
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
    /// The battery charge, in mV, below which a procedure that changes the phone does not start
    #[serde(default)]
    pub battery_floor_mv: Option<u32>,
    /// How the battery is read for that floor (needed with it, and only with it)
    #[serde(default)]
    pub battery_source: Option<BatterySource>,
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
    /// Systemd services stopped before the controlled suspend cycle (lpoco-health suspend cycle) and started
    /// again after it, last stopped first: the daemons a suspend would crash or leave stopped (surya: sensorspd)
    #[serde(default)]
    pub suspend_stop: Vec<String>,
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
             ABSENT={}\nGAPS={}\nDATA_BUDGET_MB={}\nSUSPEND_IRQ_MAX={}\nSUSPEND_GUARD={}\nSUSPEND_QUIET={}\nSUSPEND_STOP={}\nAWAKE_GATES={}\n",
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
            q(&self.suspend_stop.join(" ")),
            q(&self.awake_gates.join(" ")),
        )
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The postmarketOS-style codename, e.g. "xiaomi-surya"
    pub codename: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// How the phone is reached: ssh (as it always was), telnet on `shell_address`:`shell_port`, or adb by the
    /// phone's serial ([fastboot] or the first [[host.gadget]])
    #[serde(default)]
    pub transport: Transport,
    /// Whether the phone has a login: password (ssh, the default) or none (telnet, adb: the default there)
    pub login: Option<Login>,
    pub user: String,
    /// Addresses to try, in order (USB networking first, as a rule)
    pub addresses: Vec<String>,
    /// The port the phone's early shell answers on (its telnet port), where it has one
    pub shell_port: Option<u16>,
    /// The address that shell answers on, where it is not the first of `addresses`
    pub shell_address: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    #[default]
    Ssh,
    Telnet,
    Adb,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Login {
    Password,
    None,
}

impl Connection {
    /// The login this phone has: what is declared, else what the transport implies (ssh asks a password;
    /// telnet and adb reach a shell that asks none).
    pub fn login(&self) -> Login {
        self.login.unwrap_or(match self.transport {
            Transport::Ssh => Login::Password,
            Transport::Telnet | Transport::Adb => Login::None,
        })
    }

    fn validate(&self) -> Result<(), String> {
        match (self.transport, self.login()) {
            (Transport::Ssh, Login::None) => Err("connection: transport ssh has a password login; login = none is for telnet and adb".into()),
            (Transport::Adb, Login::Password) => Err("connection: transport adb has no login; say login = none or leave it out".into()),
            (Transport::Telnet, Login::Password) => Err("connection: transport telnet has no login; say login = none or leave it out".into()),
            (Transport::Telnet, Login::None) if self.shell_port.is_none() => {
                Err("connection: transport telnet needs shell_port, the port its shell answers on".into())
            }
            _ => Ok(()),
        }
    }
}

/// The tones the lab's ear listens for (`devlab hear`); none named means one on each side. How loud
/// any test sound may be is said here too, quiet by default: a phone in a home must never shout - a
/// full-scale tone at night once woke the author. Every check that plays sound reads the same two
/// values (TEST_TONE_DBFS, TEST_VOLUME in the phone's health.env).
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
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

/// A config file on the phone, outside any git, that the lab compares at each check: declared either against
/// the default its package ships (`expect`, a phone path) or against a copy kept in the base (`from`, a path
/// in the device repository) - exactly one. A deliberate change then lives in the base or in the package.
#[derive(Debug, Clone, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    /// The file's absolute path on the phone
    pub path: String,
    /// The package default's absolute path on the phone, e.g. /usr/share/sxmo/...
    #[serde(default)]
    pub expect: Option<String>,
    /// The copy kept in the device repository, relative to the manifest
    #[serde(default)]
    pub from: Option<String>,
}

impl UserConfig {
    fn validate(&self) -> Result<(), String> {
        let word = |p: &str| {
            p.starts_with('/') && !p.contains(char::is_whitespace) && !p.contains(['\'', '='])
        };
        if !word(&self.path) || !self.expect.as_deref().is_none_or(word) {
            return Err(format!(
                "user_config {}: a phone path is absolute, with no space, quote or '='",
                self.path
            ));
        }
        if self.expect.as_deref() == Some(self.path.as_str()) {
            return Err(format!(
                "user_config {}: expect is the file itself",
                self.path
            ));
        }
        if self
            .from
            .as_deref()
            .is_some_and(|f| f.starts_with('/') || f.split('/').any(|s| s == ".."))
        {
            return Err(format!(
                "user_config {}: from is relative to the manifest, with no ..",
                self.path
            ));
        }
        if self.expect.is_some() == self.from.is_some() {
            return Err(format!(
                "user_config {}: declare exactly one of expect (the package default) and from (the base's copy)",
                self.path
            ));
        }
        Ok(())
    }
}

/// How `[health] battery_floor_mv` is read.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, JsonSchema)]
pub enum BatterySource {
    /// `fastboot getvar battery-voltage`, while the phone is in the bootloader
    #[serde(rename = "fastboot-getvar")]
    FastbootGetvar,
}

impl Health {
    fn validate(&self) -> Result<(), String> {
        match (self.battery_floor_mv, self.battery_source) {
            (Some(_), None) => {
                Err("health: battery_floor_mv needs battery_source (how it is read)".into())
            }
            (None, Some(_)) => Err("health: battery_source needs battery_floor_mv".into()),
            _ => Ok(()),
        }
    }
}
