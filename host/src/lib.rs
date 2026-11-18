//! What the lab host needs, worked out from declarations and changed only by `prepare now`.
//! Two declarations: the lab's own (`lab.toml`: its devices, its root prefix, its tools at pins) and each
//! device's `[host]` table in its `device.toml` (decision 2026-11-17-lpoco-host).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FAMILY_URL: &str = "https://github.com/linux-phone-oco/lab-tools";
/// The programs of the lpoco family a lab host carries; each is released under its own tag.
pub const FAMILY: &[&str] = &[
    "lpoco",
    "lpoco-lab",
    "lpoco-bringup",
    "lpoco-health",
    "lpoco-host",
    "lpoco-tone",
];
/// The old names, second binaries of lpoco-lab and lpoco-tone: no step of their own, but a stale one first on PATH is off the pin.
pub const OLD_NAMES: &[(&str, &str)] = &[("devlab", "lpoco-lab"), ("tone", "lpoco-tone")];
/// A wide rule the author keeps for now: named by plan and check, never removed or edited by prepare.
pub const KEPT_RULE: &str = "/etc/udev/rules.d/51-android.rules";
pub const DEFAULT_ROOT: &str = "sudo -n lab-agent root-exec --";
pub const DEFAULT_DEVICES: &str = "~/Projects/linux-phone-oco/*/device.toml";
pub const UDEV_DIR: &str = "/etc/udev/rules.d";
pub const MODULES_FILE: &str = "/etc/modules-load.d/lpoco.conf";

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ToolPin {
    /// `lpoco = "0.2.0"`: the family, installed with cargo from its release tags.
    Version(String),
    /// A script of another repository at a commit.
    Script {
        git: String,
        path: String,
        rev: String,
        version: String,
    },
}

/// `devices = "glob"` or `devices = ["glob", ...]`: where the device manifests are.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Devices {
    One(String),
    Many(Vec<String>),
}

impl Devices {
    pub fn patterns(&self) -> Vec<String> {
        match self {
            Devices::One(s) => vec![s.clone()],
            Devices::Many(v) => v.clone(),
        }
    }
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Lab {
    pub devices: Option<Devices>,
    pub root: Option<String>,
    #[serde(default)]
    pub tools: BTreeMap<String, ToolPin>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Gadget {
    pub serial: String,
    pub access: Vec<Access>,
}

#[derive(Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostTable {
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub gadget: Vec<Gadget>,
}

/// What the host needs from one device's manifest.
#[derive(Debug, Clone)]
pub struct Device {
    pub path: PathBuf,
    pub codename: String,
    pub host: HostTable,
    pub lpoco_pin: Option<String>,
    pub fastboot_serial: Option<String>,
}

pub fn read_lab(path: &Path) -> Result<Lab, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let lab: Lab = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    validate_lab(&lab).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(lab)
}

pub fn read_device(path: &Path) -> Result<Device, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let v: toml::Value = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let str_at = |t: &str, k: &str| {
        v.get(t)
            .and_then(|t| t.get(k))
            .and_then(|x| x.as_str())
            .map(str::to_string)
    };
    let codename = str_at("device", "codename")
        .ok_or_else(|| format!("{}: [device] codename is missing", path.display()))?;
    let host = match v.get("host") {
        Some(h) => h
            .clone()
            .try_into::<HostTable>()
            .map_err(|e| format!("{}: [host]: {e}", path.display()))?,
        None => HostTable::default(),
    };
    let at = |e: String| format!("{}: {e}", path.display());
    safe_word("codename", &codename).map_err(at)?;
    for g in &host.gadget {
        safe_word("gadget serial", &g.serial).map_err(at)?;
    }
    for m in &host.modules {
        safe_word("module", m).map_err(at)?;
    }
    Ok(Device {
        path: path.to_path_buf(),
        codename,
        host,
        lpoco_pin: str_at("tools", "lpoco"),
        fastboot_serial: str_at("fastboot", "serial"),
    })
}

pub fn expand_home(s: &str, home: &str) -> String {
    match s.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => s.to_string(),
    }
}

pub fn find_devices(patterns: &[String], home: &str) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = Vec::new();
    for pattern in patterns {
        let found = glob::glob(&expand_home(pattern, home))
            .map_err(|e| format!("devices {pattern}: {e}"))?
            .flatten();
        v.extend(found);
    }
    v.sort();
    v.dedup();
    Ok(v)
}

/// Does `version` ("0.1.7") satisfy `pin` ("0", "0.1" or "0.1.7")? A pin names a prefix of the version.
pub fn pin_matches(pin: &str, version: &str) -> bool {
    let (p, v): (Vec<&str>, Vec<&str>) = (
        pin.trim().split('.').collect(),
        version.split('.').collect(),
    );
    p.len() <= 3 && p.len() <= v.len() && p.iter().zip(&v).all(|(a, b)| !a.is_empty() && a == b)
}

/// The udev rule file of one device: one line per access kind of each gadget.
pub fn rules_text(d: &Device) -> String {
    let mut s = format!(
        "# lpoco host: {} - written by `lpoco host prepare now` from its device.toml; edit the manifest, not this file\n",
        d.codename
    );
    for g in &d.host.gadget {
        for a in &g.access {
            s += &format!(
                "ACTION!=\"remove\", SUBSYSTEM==\"{}\", ATTRS{{serial}}==\"{}\", TAG+=\"uaccess\"\n",
                a.subsystem(),
                g.serial
            );
        }
    }
    s
}

pub fn modules_text(modules: &[String]) -> String {
    let mut s = String::from("# lpoco host: modules the devices' gadget functions need - written by `lpoco host prepare now`\n");
    for m in modules {
        s += m;
        s.push('\n');
    }
    s
}

/// A file the host should hold, what it holds now, and what changes with it.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub dst: String,
    pub content: String,
    pub existing: Option<String>,
    /// gadget serials whose udev rule this file carries (to trigger them once written)
    pub serials: Vec<String>,
}

impl FileChange {
    pub fn changed(&self) -> bool {
        self.existing.as_deref() != Some(self.content.as_str())
    }

    /// A line diff: `+` a line the file will gain, `-` one it will lose.
    pub fn diff(&self) -> Vec<String> {
        let old: Vec<&str> = self.existing.as_deref().unwrap_or("").lines().collect();
        let new: Vec<&str> = self.content.lines().collect();
        let mut d: Vec<String> = old
            .iter()
            .filter(|l| !new.contains(l))
            .map(|l| format!("- {l}"))
            .collect();
        d.extend(
            new.iter()
                .filter(|l| !old.contains(l))
                .map(|l| format!("+ {l}")),
        );
        d
    }
}

pub fn files_for(devices: &[Device], read: &dyn Fn(&str) -> Option<String>) -> Vec<FileChange> {
    let mut files = Vec::new();
    let mut modules: Vec<String> = Vec::new();
    for d in devices {
        if !d.host.gadget.is_empty() {
            let dst = format!("{UDEV_DIR}/70-lpoco-{}.rules", d.codename);
            files.push(FileChange {
                existing: read(&dst),
                content: rules_text(d),
                serials: d.host.gadget.iter().map(|g| g.serial.clone()).collect(),
                dst,
            });
        }
        for m in &d.host.modules {
            if !modules.contains(m) {
                modules.push(m.clone());
            }
        }
    }
    if !modules.is_empty() {
        modules.sort();
        files.push(FileChange {
            dst: MODULES_FILE.into(),
            content: modules_text(&modules),
            existing: read(MODULES_FILE),
            serials: vec![],
        });
    }
    files
}

pub fn modules_of(devices: &[Device]) -> Vec<String> {
    let mut m: Vec<String> = devices
        .iter()
        .flat_map(|d| d.host.modules.clone())
        .collect();
    m.sort();
    m.dedup();
    m
}

/// One thing a tool's installation does.
#[derive(Debug, Clone)]
pub enum Step {
    Run(Vec<String>),
    /// install a file at a mode
    Copy {
        from: String,
        to: String,
    },
    /// record the rev a script tool was installed at, so a later plan judges it by that and not by `--version`
    Stamp {
        to: String,
        rev: String,
    },
}

impl Step {
    pub fn show(&self) -> String {
        match self {
            Step::Run(a) => a.join(" "),
            Step::Copy { from, to } => format!("install -m 0755 {from} {to}"),
            Step::Stamp { to, rev } => format!("echo {rev} > {to}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolPlan {
    pub name: String,
    pub pinned: String,
    pub found: Option<String>,
    pub steps: Vec<Step>,
    /// the program that installs this one (an old name), when it has no step of its own
    pub installed_by: Option<String>,
    /// a script tool is pinned by its rev and judged by the rev its install stamped, never by `--version`
    pub by_rev: bool,
    /// a script tool is on PATH though no stamp says which rev
    pub on_path: bool,
}

pub const NOT_KNOWN: &str = "not known yet";

impl ToolPlan {
    /// A tool the lab names but has no source to pin yet: listed, nothing installed.
    pub fn unpinned(&self) -> bool {
        self.pinned == NOT_KNOWN
    }

    pub fn ok(&self) -> bool {
        if self.unpinned() {
            return true;
        }
        if self.by_rev {
            return self.found.as_deref() == Some(self.pinned.as_str());
        }
        self.found
            .as_deref()
            .is_some_and(|f| pin_matches(&self.pinned, f))
    }

    /// What was found, in words.
    pub fn found_text(&self) -> String {
        match (&self.found, self.by_rev && self.on_path) {
            (Some(f), _) => f.clone(),
            (None, true) => "on PATH, rev not known (no stamp)".into(),
            (None, false) => "not found".into(),
        }
    }
}

/// Where a script tool's install records its rev.
pub fn stamp_path(home: &str, name: &str) -> String {
    format!("{home}/.local/share/lpoco-host/rev-{name}")
}

/// The git tag a family program is released under: `lpoco` as `lpoco-vX`, the others as `NAME-vX`.
pub fn family_tag(program: &str, version: &str) -> String {
    format!("{program}-v{version}")
}

/// `found` is the version a program reports, `stamp` the rev a script tool's install recorded, `on_path` whether a program is on PATH.
pub fn tools_for(
    lab: &Lab,
    home: &str,
    found: &dyn Fn(&str) -> Option<String>,
    stamp: &dyn Fn(&str) -> Option<String>,
    on_path: &dyn Fn(&str) -> bool,
) -> Vec<ToolPlan> {
    let mut plans = Vec::new();
    for (name, pin) in &lab.tools {
        match pin {
            ToolPin::Version(v) if name == "lpoco" => {
                for p in FAMILY {
                    plans.push(ToolPlan {
                        name: (*p).to_string(),
                        pinned: v.clone(),
                        found: found(p),
                        installed_by: None,
                        by_rev: false,
                        on_path: false,
                        steps: vec![Step::Run(
                            [
                                "cargo",
                                "install",
                                "--locked",
                                "--git",
                                FAMILY_URL,
                                "--tag",
                                &family_tag(p, v),
                                p,
                                "--root",
                                &format!("{home}/.local"),
                            ]
                            .map(String::from)
                            .to_vec(),
                        )],
                    });
                }
                for (old, from) in OLD_NAMES {
                    plans.push(ToolPlan {
                        name: (*old).to_string(),
                        pinned: v.clone(),
                        found: found(old),
                        steps: vec![],
                        installed_by: Some((*from).to_string()),
                        by_rev: false,
                        on_path: false,
                    });
                }
            }
            ToolPin::Version(_) => plans.push(ToolPlan {
                name: name.clone(),
                pinned: NOT_KNOWN.to_string(),
                found: found(name),
                steps: vec![],
                installed_by: None,
                by_rev: false,
                on_path: false,
            }),
            ToolPin::Script {
                git,
                path,
                rev,
                version: _,
            } => {
                let dir = format!("{home}/.cache/lpoco-host/src-{name}");
                plans.push(ToolPlan {
                    name: name.clone(),
                    pinned: rev.clone(),
                    installed_by: None,
                    by_rev: true,
                    on_path: on_path(name),
                    found: if on_path(name) { stamp(name) } else { None },
                    steps: vec![
                        Step::Run(
                            ["git", "clone", "--quiet", git, &dir]
                                .map(String::from)
                                .to_vec(),
                        ),
                        Step::Run(
                            ["git", "-C", &dir, "checkout", "--quiet", rev]
                                .map(String::from)
                                .to_vec(),
                        ),
                        Step::Copy {
                            from: format!("{dir}/{path}"),
                            to: format!("{home}/.local/bin/{name}"),
                        },
                        Step::Stamp {
                            to: stamp_path(home, name),
                            rev: rev.clone(),
                        },
                    ],
                });
            }
        }
    }
    plans
}

/// Devices whose `[tools] lpoco` pin the lab's does not meet: nothing is prepared for them.
pub fn pin_misses<'a>(lab: &Lab, devices: &'a [Device]) -> Vec<(&'a Device, String)> {
    let Some(ToolPin::Version(have)) = lab.tools.get("lpoco") else {
        return vec![];
    };
    devices
        .iter()
        .filter_map(|d| match &d.lpoco_pin {
            Some(p) if !pin_matches(p, have) => Some((
                d,
                format!("{} pins lpoco {p}, the lab has {have}", d.path.display()),
            )),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_names_a_prefix() {
        assert!(pin_matches("0.2", "0.2.0"));
        assert!(!pin_matches("0.3", "0.2.0"));
    }

    #[test]
    fn a_file_that_is_the_same_has_no_diff() {
        let f = FileChange {
            dst: "x".into(),
            content: "a\nb\n".into(),
            existing: Some("a\nb\n".into()),
            serials: vec![],
        };
        assert!(!f.changed() && f.diff().is_empty());
    }
}

/// One manifest per codename: a lab keeps several clones and worktrees of a device's repository, and the host
/// is prepared once per device. A clone that declares no `[host]` is passed over; of those that do, the first
/// path wins and a later one that declares something else is named.
pub fn one_per_codename(devices: Vec<Device>) -> (Vec<Device>, Vec<String>) {
    let mut kept: Vec<Device> = Vec::new();
    let mut notes = Vec::new();
    for d in devices {
        let empty = d.host == HostTable::default();
        match kept.iter().position(|k| k.codename == d.codename) {
            None => kept.push(d),
            Some(i) => {
                let k_empty = kept[i].host == HostTable::default();
                if k_empty && !empty {
                    kept[i] = d;
                } else if !empty && kept[i].host != d.host {
                    notes.push(format!(
                        "{} declares another [host] than {} for {}: ignored",
                        d.path.display(),
                        kept[i].path.display(),
                        d.codename
                    ));
                }
            }
        }
    }
    (kept, notes)
}

#[cfg(test)]
mod one_per_codename_tests {
    use super::*;

    fn dev(path: &str, serial: Option<&str>) -> Device {
        let gadget = serial.map(|s| Gadget {
            serial: s.into(),
            access: vec![Access::Block],
        });
        Device {
            path: path.into(),
            codename: "w".into(),
            host: HostTable {
                modules: vec![],
                gadget: gadget.into_iter().collect(),
            },
            lpoco_pin: None,
            fastboot_serial: None,
        }
    }

    #[test]
    fn a_clone_without_host_gives_way_and_a_conflict_is_named() {
        let (k, n) = one_per_codename(vec![
            dev("a", None),
            dev("b", Some("s")),
            dev("c", Some("s")),
            dev("d", Some("t")),
        ]);
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].path, PathBuf::from("b"));
        assert_eq!(n.len(), 1, "{n:?}");
    }
}

/// A word that reaches a root command, a file name or a udev rule: letters, digits, `.`, `_`, `-`, not leading `-`.
pub fn safe_word(what: &str, s: &str) -> Result<(), String> {
    let ok = !s.is_empty()
        && s != ".."
        && !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{what} {s:?} must be letters, digits, '.', '_' or '-', and not start with '-'"
        ))
    }
}

fn full_version(v: &str) -> bool {
    let p: Vec<&str> = v.split('.').collect();
    p.len() == 3
        && p.iter()
            .all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
}

fn validate_lab(lab: &Lab) -> Result<(), String> {
    for (name, pin) in &lab.tools {
        safe_word("tool name", name)?;
        match pin {
            ToolPin::Version(v) if name == "lpoco" => {
                if !full_version(v) {
                    return Err(format!(
                        "[tools] lpoco = {v:?} must be a full X.Y.Z: it names the release tags"
                    ));
                }
            }
            ToolPin::Version(v) if v == "unknown" => {}
            ToolPin::Version(v) => {
                return Err(format!("[tools] {name} = {v:?}: a bare version is for lpoco only; give git, path, rev and version, or \"unknown\" when it has no source yet"));
            }
            ToolPin::Script {
                git,
                path,
                rev,
                version,
            } => {
                for (w, s) in [("git", git), ("rev", rev), ("version", version)] {
                    if s.chars().any(|c| c.is_whitespace() || c.is_control())
                        || s.starts_with('-')
                        || s.is_empty()
                    {
                        return Err(format!("[tools.{name}] {w} {s:?} is not usable"));
                    }
                }
                if path.starts_with('/') || path.split('/').any(|c| c == "..") {
                    return Err(format!(
                        "[tools.{name}] path {path:?} must stay inside the repository"
                    ));
                }
            }
        }
    }
    Ok(())
}
