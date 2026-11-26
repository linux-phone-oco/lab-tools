// SPDX-License-Identifier: MIT OR Apache-2.0
//! A device's manifest, `device.toml`, kept in its lab repository: what the tools need to know about
//! the phone. Secrets and what is learned at run time stay outside the repository.

use anyhow::{bail, Context, Result};
use lpoco_manifest::Manifest;
pub use lpoco_manifest::{
    Connection, Fastboot, Flash, Hazard, Health, Hear, Identity, Provision, Tests,
};
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "device.toml";

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub device: Identity,
    pub connection: Connection,
    /// The other ways in, by name ([connections.NAME])
    pub connections: std::collections::BTreeMap<String, Connection>,
    /// The name of the connection `connection` is, when it was chosen by name; none for the default
    pub connection_name: Option<String>,
    pub tests: Tests,
    pub provision: Provision,
    pub flash: Flash,
    /// How the lab reaches the phone over fastboot, and whether it may write to it there
    pub fastboot: Fastboot,
    /// what the lab's ear listens for
    pub hear: Hear,
    /// what the phone's own health suite expects of it
    pub health: Health,
    /// What must never be touched on this phone, whoever asks: a command devlab carries to the phone
    /// (phone sh|root, the MCP door) that names one is refused, saying why
    pub hazard: Vec<Hazard>,
    /// How the phone shows on the lab machine's USB bus ([[host.gadget]]): the try reads the host's log for it
    pub gadgets: Vec<lpoco_manifest::Gadget>,
    /// Config files the phone keeps outside any git: the health suite reports their blobs, the lab compares
    pub user_config: Vec<lpoco_manifest::UserConfig>,
}

impl Device {
    /// This device reached by the connection NAME ([connections.NAME]), or as it is for none. A name the manifest
    /// does not declare is refused, saying so.
    pub fn on(&self, name: Option<&str>) -> Result<Device> {
        let Some(name) = name else {
            return Ok(self.clone());
        };
        let connection = self.connections.get(name).with_context(|| {
            format!(
                "{} declares no [connections.{name}] in device.toml (declared: {})",
                self.device.codename,
                self.connections
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        Ok(Device {
            connection: connection.clone(),
            connection_name: Some(name.to_string()),
            ..self.clone()
        })
    }

    /// The serial adb reaches this phone by: its fastboot serial, else the first gadget serial the manifest names.
    pub fn adb_serial(&self) -> Result<String> {
        let from_fastboot = self.fastboot.serial.trim();
        let serial = if from_fastboot.is_empty() {
            self.gadgets
                .iter()
                .map(|g| g.serial.trim())
                .find(|s| !s.is_empty())
                .unwrap_or("")
        } else {
            from_fastboot
        };
        if serial.is_empty() {
            bail!(
                "{} is reached by adb but device.toml names no serial: say [fastboot] serial (or a [[host.gadget]] serial)",
                self.device.codename
            );
        }
        Ok(serial.to_string())
    }

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

impl Device {
    /// The files the manifest puts on the phone, as phone paths: what the suite check lab-pieces reports a
    /// blob id for (PIECE_FILES in health.env). The flash overlays into the root, the provision installs,
    /// and `checks` (the phone's own checks, relative paths as `suite put` lays them). A target that names
    /// the device tree (`{compatible}`) is left out: the manifest does not say which file it becomes.
    pub fn pieces_env(&self, checks: &[String]) -> Result<String> {
        let mut files: Vec<String> = Vec::new();
        for o in &self.flash.overlay {
            if o.if_exists.is_some() {
                continue; // may never be on this phone: reporting it missing would be noise
            }
            if let Some(p) = o.to.strip_prefix("root:") {
                files.push(p.to_string());
            }
        }
        files.extend(self.provision.install.iter().map(|i| i.to.clone()));
        files.extend(checks.iter().map(|c| format!("/{c}")));
        files
            .retain(|f| f.starts_with('/') && !f.contains('{') && !f.contains(char::is_whitespace));
        if let Some(bad) = files.iter().find(|f| f.contains('\'')) {
            anyhow::bail!("a manifest path holds a quote, which health.env cannot carry: {bad}");
        }
        files.sort();
        files.dedup();
        let mut env = format!("PIECE_FILES='{}'\n", files.join(" "));
        if !self.user_config.is_empty() {
            // PATH, or PATH=DEFAULT when declared against the package default (the manifest refuses a space, quote or '=')
            let configs: Vec<String> = self
                .user_config
                .iter()
                .map(|c| match &c.expect {
                    Some(e) => format!("{}={e}", c.path),
                    None => c.path.clone(),
                })
                .collect();
            env.push_str(&format!("PIECE_CONFIGS='{}'\n", configs.join(" ")));
        }
        Ok(env)
    }

    pub fn parse(text: &str) -> Result<Device> {
        let m = Manifest::parse(text).map_err(|e| anyhow::anyhow!("reading device.toml: {e}"))?;
        let device = m.device.context("device.toml: [device] is missing")?;
        let connection = m
            .connection
            .context("device.toml: [connection] is missing")?;
        if connection.addresses.is_empty() && connection.transport == lpoco_manifest::Transport::Ssh
        {
            bail!("device.toml: connection.addresses is empty");
        }
        for (name, c) in &m.connections {
            if c.addresses.is_empty() && c.transport == lpoco_manifest::Transport::Ssh {
                bail!("device.toml: connections.{name}.addresses is empty");
            }
        }
        Ok(Device {
            device,
            connection,
            connections: m.connections,
            connection_name: None,
            tests: m.tests,
            provision: m.provision,
            flash: m.flash,
            fastboot: m.fastboot,
            hear: m.hear,
            health: m.health,
            hazard: m.hazard,
            gadgets: m.host.gadget,
            user_config: m.user_config,
        })
    }

    /// The manifest file: `path`, or found by walking up from `start` as cargo finds Cargo.toml.
    pub fn locate(path: Option<&Path>, start: &Path) -> Result<PathBuf> {
        Ok(match path {
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
        })
    }

    pub fn load(path: Option<&Path>, start: &Path) -> Result<(Device, PathBuf)> {
        let file = Device::locate(path, start)?;
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("reading {}", file.display()))?;
        Ok((Device::parse(&text)?, file))
    }

    /// `lpoco-lab manifest check`: whether this version reads the manifest as every command does (`Device::parse`) -
    /// parse and validate only, nothing written. An error names the file, the version and the reason.
    pub fn check(path: &Path, version: &str) -> Result<String> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Device::parse(&text).map_err(|e| {
            anyhow::anyhow!(
                "{} is not read by lpoco-lab {version}: {e:#}",
                path.display()
            )
        })?;
        Ok(format!("{}: read by lpoco-lab {version}", path.display()))
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
        state_root().join(&self.device.codename)
    }
}

/// Where every phone's run-time state is: ~/.local/state/devlab/ (one folder per codename).
pub fn state_root() -> PathBuf {
    home().join(".local/state/devlab")
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
    fn the_declared_files_become_the_pieces_the_phone_reports() {
        let d = Device::parse(
            "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
             [[flash.overlay]]\nfrom = \"a\"\nto = \"root:/etc/b.conf\"\n\
             [[flash.overlay]]\nfrom = \"a\"\nto = \"esp:/loader.conf\"\n\
             [[provision.install]]\nfrom = \"c\"\nto = \"/usr/bin/c\"\n\
             [[provision.install]]\nfrom = \"d\"\nto = \"/boot/{compatible}.dtb\"\n",
        )
        .unwrap();
        let e = d
            .pieces_env(&["usr/lib/lpoco-health/device/base/10-x.sh".to_string()])
            .unwrap();
        assert_eq!(
            e, "PIECE_FILES='/etc/b.conf /usr/bin/c /usr/lib/lpoco-health/device/base/10-x.sh'\n",
            "root overlays, installs and own checks; not the esp's, not a name the tree decides"
        );
    }

    #[test]
    fn a_declared_user_config_is_named_to_the_phone_with_its_package_default_when_it_has_one() {
        let d = Device::parse(
            "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n\
             [[user_config]]\npath = \"/home/u/hook.sh\"\nexpect = \"/usr/share/x/hook.sh\"\n\
             [[user_config]]\npath = \"/home/u/profile\"\nfrom = \"config/profile\"\n",
        )
        .unwrap();
        assert_eq!(
            d.pieces_env(&[]).unwrap(),
            "PIECE_FILES=''\nPIECE_CONFIGS='/home/u/hook.sh=/usr/share/x/hook.sh /home/u/profile'\n"
        );
    }
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
        let ss: Health = toml::from_str("suspend_stop = [\"sensorspd\", \"hexagonrpcd\"]").unwrap();
        assert!(ss
            .env("p")
            .contains("SUSPEND_STOP='sensorspd hexagonrpcd'\n"));
        assert!(e.contains("SUSPEND_STOP=''\n"));
        assert!(e.contains("SUSPEND_TRACE_DYNDBG=''\n") && e.contains("SUSPEND_TRACE_IRQ=''\n"));
        let tr: Health = toml::from_str(
            "suspend_trace_dyndbg = [\"file a.c\", \"module b\"]\nsuspend_trace_irq = [157, 12]",
        )
        .unwrap();
        let te = tr.env("p");
        assert!(
            te.contains("SUSPEND_TRACE_DYNDBG='file a.c;module b'\n"),
            "{te}"
        );
        assert!(te.contains("SUSPEND_TRACE_IRQ='157 12'\n"), "{te}");
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

#[cfg(test)]
mod one_schema {
    use super::*;

    const HEAD: &str =
        "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n";

    #[test]
    fn a_misspelled_table_is_refused_and_missing_ones_are_named() {
        let e = Device::parse(&format!(
            "{HEAD}[[flash.overlays]]\nfrom = \"a\"\nto = \"b\"\n"
        ))
        .unwrap_err();
        assert!(format!("{e:#}").contains("overlays"), "{e:#}");
        assert!(format!(
            "{:#}",
            Device::parse("[device]\ncodename = \"p\"\n").unwrap_err()
        )
        .contains("[connection] is missing"));
        assert!(format!(
            "{:#}",
            Device::parse("[connection]\nuser=\"u\"\naddresses=[\"a\"]\n").unwrap_err()
        )
        .contains("[device] is missing"));
    }
}
