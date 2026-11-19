//! What the lab host needs, worked out from declarations and changed only by `prepare now`.
//! Two declarations: the lab's own (`lab.toml`: its devices, its root prefix, its tools at pins) and each
//! device's `[host]` table in its `device.toml` (decision 2026-11-17-lpoco-host).

pub mod share;

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
/// Root is asked for only by `prepare now`, behind sudo with the author's typed password: no standing grant.
pub const DEFAULT_ROOT: &str = "sudo";
pub const DEFAULT_DEVICES: &str = "~/Projects/linux-phone-oco/*/device.toml";
pub const UDEV_DIR: &str = "/etc/udev/rules.d";
pub const MODULES_FILE: &str = "/etc/modules-load.d/lpoco.conf";

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ToolPin {
    /// `lpoco = "0.2.0"`: the family, installed with cargo from its release tags.
    Version(String),
    /// A script of another repository at a commit. `version` is a label for the reader only: a script is judged by its
    /// `rev` stamp, never by `--version`.
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
    /// A group the lab user already holds: each gadget node gets it and mode 0660, so access does not wait on the seat's uaccess ACL.
    pub gadget_group: Option<String>,
    #[serde(default)]
    pub tools: BTreeMap<String, ToolPin>,
}

pub use lpoco_manifest::{Access, Gadget, HostTable};

/// What the host needs from one device's manifest.
#[derive(Debug, Clone)]
pub struct Device {
    pub path: PathBuf,
    pub codename: String,
    pub host: HostTable,
    pub lpoco_pin: Option<String>,
    pub fastboot_serial: Option<String>,
    /// The phone's own address on its USB network (the manifest's first `[connection] addresses`): where the lab's side of it is found.
    pub usb_address: Option<String>,
    /// Files the manifest puts on the phone (`[[flash.overlay]]`, `[[provision.install]]`).
    pub pieces: Vec<Piece>,
    /// Config files the phone keeps outside any git (`[[user_config]]`), each against its package default or the base's copy.
    pub user_configs: Vec<lpoco_manifest::UserConfig>,
}

/// One declared phone-side file: `from` in the device repository, `to` on the phone (a `root:` prefix, the image's root, is dropped).
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub from: String,
    pub to: String,
}

fn pieces_of(v: &toml::Value) -> Vec<Piece> {
    let list = |path: &[&str]| {
        let mut at = Some(v);
        for k in path {
            at = at.and_then(|t| t.get(*k));
        }
        at.and_then(|a| a.as_array()).cloned().unwrap_or_default()
    };
    let mut out = Vec::new();
    for e in list(&["flash", "overlay"])
        .iter()
        .chain(list(&["provision", "install"]).iter())
    {
        let get = |k: &str| e.get(k).and_then(|x| x.as_str());
        if let (Some(from), Some(to)) = (get("from"), get("to")) {
            out.push(Piece {
                from: from.into(),
                to: to.strip_prefix("root:").unwrap_or(to).into(),
            });
        }
    }
    out
}

/// A phone-side piece found off what main declares.
#[derive(Debug, Clone, PartialEq)]
pub enum Stale {
    /// A file on the phone whose blob is not the one at origin/main.
    File {
        path: String,
        found: String,
        main: String,
    },
    /// A declared file or a lab-tools package the phone does not hold.
    Missing { what: String },
    /// A declared user config whose blob is not the one it is declared against: `expected` is the blob of the
    /// package default (or of the base's copy at origin/main), `against` says which, in words.
    Config {
        path: String,
        found: String,
        expected: String,
        against: String,
    },
    /// A lab-tools package on the phone off the `[tools] lpoco` pin.
    Package {
        name: String,
        found: String,
        pin: String,
    },
}

/// Git's blob id of `bytes`: sha1 of `blob LEN\0` and the bytes (the phone computes the same with busybox).
pub fn blob_id(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(format!("blob {}\0", bytes.len()).as_bytes());
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// What the phone's last gathered report (jsonl, one result a line; the rows of health's 94-lab-pieces.sh) says is
/// off main. A row `test = "piece-file:PATH"` has detail `PATH BLOBID|missing`; `test = "piece-package-NAME"` has
/// `NAME VERSION|missing`; `test = "config-file:PATH"` has `PATH BLOB DEFAULT` (a declared user config). `main` gives the bytes origin/main holds for a declared `from` (None: not judged).
/// Rows the report lacks are not judged: not measured is not stale. A line that is not a row is an error.
pub fn stale_pieces(
    d: &Device,
    report: &str,
    main: impl Fn(&str) -> Option<Vec<u8>>,
) -> Result<Vec<Stale>, String> {
    let mut out: Vec<Stale> = Vec::new();
    for line in report.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("report line {line:?}: {e}"))?;
        let (Some(test), Some(detail)) = (v["test"].as_str(), v["detail"].as_str()) else {
            return Err(format!("report line {line:?}: no test or detail"));
        };
        let mut w = detail.split_whitespace();
        let (Some(what), Some(found)) = (w.next(), w.next()) else {
            continue;
        };
        let s = if test.starts_with("config-file:") {
            // detail `PATH BLOB DEFAULT`: only a declared path is read. A file the phone lacks has no edit to lose
            // against a package default, but is drift against the base's copy; a default or copy that cannot be
            // found is named, never passed.
            let Some(c) = d.user_configs.iter().find(|c| c.path == what) else {
                continue;
            };
            let (expected, against) = match (&c.expect, &c.from) {
                (Some(_), _) if found == "missing" => continue,
                (Some(e), _) => match w.next() {
                    Some("-") | None => continue, // not reported: not measured is not stale
                    Some("missing") => {
                        let what = format!("{what}'s package default {e}");
                        let s = Stale::Missing { what };
                        if !out.contains(&s) {
                            out.push(s);
                        }
                        continue;
                    }
                    Some(blob) => (blob.to_string(), format!("the package default {e}")),
                },
                (None, Some(f)) => {
                    let Some(bytes) = main(f) else {
                        let s = Stale::Missing {
                            what: format!("{what}'s base copy {f} at origin/main"),
                        };
                        if !out.contains(&s) {
                            out.push(s);
                        }
                        continue;
                    };
                    if found == "missing" {
                        out.push(Stale::Missing { what: what.into() });
                        continue;
                    }
                    (blob_id(&bytes), format!("the base's copy {f}"))
                }
                (None, None) => continue,
            };
            if expected == found {
                continue;
            }
            Stale::Config {
                path: what.into(),
                found: found.into(),
                expected,
                against,
            }
        } else if test.starts_with("piece-file:") {
            let Some(p) = d.pieces.iter().find(|p| p.to == what) else {
                continue;
            };
            let Some(bytes) = main(&p.from) else { continue };
            let m = blob_id(&bytes);
            if found == "missing" {
                Stale::Missing { what: what.into() }
            } else if m != found {
                Stale::File {
                    path: what.into(),
                    found: found.into(),
                    main: m,
                }
            } else {
                continue;
            }
        } else if test.starts_with("piece-package-") {
            let Some(pin) = d.lpoco_pin.as_deref() else {
                continue;
            };
            if found == "missing" {
                Stale::Missing { what: what.into() }
            } else if !pin_matches(pin, found.split("-r").next().unwrap_or(found)) {
                let found = found.split("-r").next().unwrap_or(found);
                Stale::Package {
                    name: what.into(),
                    found: found.into(),
                    pin: pin.into(),
                }
            } else {
                continue;
            }
        } else {
            continue;
        };
        if !out.contains(&s) {
            out.push(s);
        }
    }
    Ok(out)
}

/// The lines that name what `report` (the phone's rows, read back from `per`) holds off origin/main of the device's
/// repository (the folder of its manifest): one line a stale piece, with the commit the phone's copy came from, or
/// one line saying none is. `Err` when the report is no report; the caller says "not measured". Never touches the phone.
pub fn stale_lines(d: &Device, report: &str, per: &str) -> Result<Vec<String>, String> {
    let git = |args: &[&str]| {
        let dir = d.path.parent().filter(|p| !p.as_os_str().is_empty());
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir.unwrap_or(Path::new(".")))
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
    };
    let main = |from: &str| git(&["show", &format!("origin/main:{from}")]).map(|o| o.stdout);
    let stale = stale_pieces(d, report, main)?;
    if stale.is_empty() {
        return Ok(vec![format!(
            "{} phone pieces: ok, as main per {per}",
            d.codename
        )]);
    }
    Ok(stale
        .into_iter()
        .map(|s| match s {
            Stale::File { path, found, main } => {
                let from = d
                    .pieces
                    .iter()
                    .find(|p| p.to == path)
                    .map(|p| p.from.clone())
                    .unwrap_or_default();
                let came = git(&[
                    "log",
                    "--format=%h %s",
                    &format!("--find-object={found}"),
                    "origin/main",
                    "--",
                    &from,
                ])
                // the oldest commit that holds the blob: where the phone's copy came from, not what replaced it
                .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().last().map(str::to_string))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "a commit not found".into());
                format!("{} phone piece {path}: STALE, runs {found} ({came}), main has {main} (per {per})", d.codename)
            }
            Stale::Missing { what } => format!(
                "{} phone piece {what}: STALE, missing on the phone (per {per})",
                d.codename
            ),
            Stale::Config {
                path,
                found,
                expected,
                against,
            } => format!(
                "{} phone config {path}: DRIFTED, runs {found}, {against} is {expected} (per {per})",
                d.codename
            ),
            Stale::Package { name, found, pin } => format!(
                "{} phone package {name}: STALE, {found} is off the pin {pin} (per {per})",
                d.codename
            ),
        })
        .collect())
}

/// A lab declaration already read: `path` names it, `text` is what it says (the working file, or origin/main's).
pub fn parse_lab(path: &Path, text: &str) -> Result<Lab, String> {
    let lab: Lab = toml::from_str(text).map_err(|e| format!("{}: {e}", path.display()))?;
    validate_lab(&lab).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(lab)
}

pub fn read_device(path: &Path) -> Result<Device, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    parse_device(path, &text)
}

/// A device manifest already read: `path` names it, `text` is what it says (the working file, or origin/main's).
pub fn parse_device(path: &Path, text: &str) -> Result<Device, String> {
    let v: toml::Value = toml::from_str(text).map_err(|e| format!("{}: {e}", path.display()))?;
    let m =
        lpoco_manifest::Manifest::parse(text).map_err(|e| format!("{}: {e}", path.display()))?;
    let str_at = |t: &str, k: &str| {
        v.get(t)
            .and_then(|t| t.get(k))
            .and_then(|x| x.as_str())
            .map(str::to_string)
    };
    let codename = str_at("device", "codename")
        .ok_or_else(|| format!("{}: [device] codename is missing", path.display()))?;
    let host = m.host.clone();
    let at = |e: String| format!("{}: {e}", path.display());
    safe_word("codename", &codename).map_err(at)?;
    for g in &host.gadget {
        safe_word("gadget serial", &g.serial).map_err(at)?;
        if let Some(n) = &g.net_name {
            safe_word("gadget net_name", n).map_err(at)?;
            if n.len() > 15 || n == "." {
                return Err(at(format!(
                    "gadget net_name {n:?} is longer than 15 characters, an interface name's limit"
                )));
            }
        }
    }
    for g in &host.gadget {
        if let Some(a) = &g.net_address {
            if g.net_name.is_none() {
                return Err(at("gadget net_address needs a net_name: the link config matches the interface by that name".into()));
            }
            valid_cidr(a).map_err(|e| at(format!("gadget net_address {a:?}: {e}")))?;
        }
    }
    let mut names: Vec<&String> = Vec::new();
    for n in host.gadget.iter().filter_map(|g| g.net_name.as_ref()) {
        if names.contains(&n) {
            return Err(at(format!("gadget net_name {n:?} is declared twice")));
        }
        names.push(n);
    }
    for m in &host.modules {
        safe_word("module", m).map_err(at)?;
    }
    for c in &host.commands {
        safe_word("command", c).map_err(at)?;
    }
    for t in &host.targets {
        safe_word("target", t).map_err(at)?;
    }
    Ok(Device {
        path: path.to_path_buf(),
        codename,
        host,
        lpoco_pin: str_at("tools", "lpoco"),
        fastboot_serial: str_at("fastboot", "serial"),
        usb_address: v
            .get("connection")
            .and_then(|c| c.get("addresses"))
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|a| a.as_str())
            .map(str::to_string),
        pieces: pieces_of(&v),
        user_configs: m.user_config.clone(),
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

/// `, GROUP="g", MODE="0660"` when the lab declares a gadget group, else nothing.
fn group_part(group: Option<&str>) -> String {
    group
        .map(|g| format!(", GROUP=\"{g}\", MODE=\"0660\""))
        .unwrap_or_default()
}

/// The udev rule file of one device: one line per access kind of each gadget.
pub fn rules_text(d: &Device, group: Option<&str>) -> String {
    let mut s = format!(
        "# lpoco host: {} - written by `lpoco host prepare now` from its device.toml; edit the manifest, not this file\n",
        d.codename
    );
    for g in &d.host.gadget {
        for a in &g.access {
            s += &format!(
                "ACTION!=\"remove\", SUBSYSTEM==\"{}\", ATTRS{{serial}}==\"{}\", TAG+=\"uaccess\"{}\n",
                a.subsystem(),
                g.serial,
                group_part(group)
            );
        }
        if let Some(n) = &g.net_name {
            s += &format!(
                "ACTION==\"add\", SUBSYSTEM==\"net\", ATTRS{{serial}}==\"{}\", NAME=\"{n}\"\n",
                g.serial
            );
        }
    }
    s
}

/// An IPv4 address with its prefix, `a.b.c.d/n`, n from 1 to 32: nothing else reaches a link file.
pub fn valid_cidr(s: &str) -> Result<(), String> {
    let (a, n) = s.split_once('/').ok_or("not a.b.c.d/n")?;
    a.parse::<std::net::Ipv4Addr>()
        .map_err(|_| "not an IPv4 address".to_string())?;
    match n.parse::<u8>() {
        Ok(p) if (1..=32).contains(&p) && n.chars().all(|c| c.is_ascii_digit()) => Ok(()),
        _ => Err("the prefix is not 1 to 32".into()),
    }
}

/// The network manager that runs on the host: it decides the format of a link config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkManager {
    NetworkManager,
    Networkd,
}

pub const NM_DIR: &str = "/etc/NetworkManager/system-connections";
pub const NETWORKD_DIR: &str = "/etc/systemd/network";

impl LinkManager {
    /// NetworkManager when it runs (`/run/NetworkManager`), else systemd-networkd when it runs (`/run/systemd/netif`), else none.
    pub fn detect(fs_root: &str) -> Option<LinkManager> {
        let has = |p: &str| Path::new(&format!("{fs_root}{p}")).is_dir();
        if has("/run/NetworkManager") {
            Some(LinkManager::NetworkManager)
        } else if has("/run/systemd/netif") {
            Some(LinkManager::Networkd)
        } else {
            None
        }
    }

    pub fn file_for(self, name: &str) -> String {
        match self {
            LinkManager::NetworkManager => format!("{NM_DIR}/{name}.nmconnection"),
            LinkManager::Networkd => format!("{NETWORKD_DIR}/70-{name}.network"),
        }
    }

    pub fn reload(self) -> Vec<String> {
        let v: &[&str] = match self {
            LinkManager::NetworkManager => &["nmcli", "connection", "reload"],
            LinkManager::Networkd => &["networkctl", "reload"],
        };
        v.iter().map(|x| x.to_string()).collect()
    }

    pub fn text(self, name: &str, address: &str) -> String {
        match self {
            LinkManager::NetworkManager => format!(
                "# lpoco host: the host's address on {name} - written by `lpoco host prepare now` from its device.toml; edit the manifest, not this file\n[connection]\nid={name}\ntype=ethernet\ninterface-name={name}\nautoconnect=true\n\n[ipv4]\nmethod=manual\naddress1={address}\n\n[ipv6]\nmethod=disabled\n"
            ),
            LinkManager::Networkd => format!(
                "# lpoco host: the host's address on {name} - written by `lpoco host prepare now` from its device.toml; edit the manifest, not this file\n[Match]\nName={name}\n\n[Network]\nAddress={address}\n"
            ),
        }
    }
}

/// NetworkManager refuses a keyfile that is not root's and private; every other file we write is world-readable.
pub fn file_mode(dst: &str) -> &'static str {
    if dst == share::SHARE_HELPER_DST {
        return "0755";
    }
    if dst.starts_with(NM_DIR) {
        "0600"
    } else {
        "0644"
    }
}

/// The gadgets that declare an address, as (interface name, address).
pub fn net_links(devices: &[Device]) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = devices
        .iter()
        .flat_map(|d| &d.host.gadget)
        .filter_map(|g| Some((g.net_name.clone()?, g.net_address.clone()?)))
        .collect();
    v.sort();
    v.dedup();
    v
}

pub fn modules_text(modules: &[String]) -> String {
    let mut s = String::from("# lpoco host: modules the devices' gadget functions need - written by `lpoco host prepare now`\n");
    for m in modules {
        s += m;
        s.push('\n');
    }
    s
}

/// What the host holds at a path, as far as the lab user can tell.
#[derive(Debug, Clone, PartialEq)]
pub enum Seen {
    Absent,
    Text(String),
    /// a root-only folder hides it; `recorded` is the hash of the content the last install put there, if known
    Unreadable {
        recorded: Option<String>,
    },
}

impl From<Option<String>> for Seen {
    fn from(o: Option<String>) -> Seen {
        o.map_or(Seen::Absent, Seen::Text)
    }
}

/// The hash a record of an installed file keeps: git's blob id of its content.
pub fn content_hash(s: &str) -> String {
    blob_id(s.as_bytes())
}

/// A file the host should hold, what it holds now, and what changes with it.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub dst: String,
    pub content: String,
    pub seen: Seen,
    /// gadget serials whose udev rule this file carries (to trigger them once written)
    pub serials: Vec<String>,
}

impl FileChange {
    pub fn changed(&self) -> bool {
        match &self.seen {
            Seen::Absent => true,
            Seen::Text(t) => *t != self.content,
            Seen::Unreadable { recorded } => {
                recorded.as_deref() != Some(content_hash(&self.content).as_str())
            }
        }
    }

    /// How the plan words this file: changes, new, or - for one it cannot read - what it knows of it.
    pub fn state(&self) -> &'static str {
        match (&self.seen, self.changed()) {
            (Seen::Absent, _) => "new",
            (Seen::Text(_), true) => "changes",
            (Seen::Text(_), false) => "as declared",
            (Seen::Unreadable { .. }, true) => {
                "unreadable, not known yet: no record of an install of this content"
            }
            (Seen::Unreadable { .. }, false) => "as declared, unreadable: recorded at install",
        }
    }

    /// A line diff: `+` a line the file will gain, `-` one it will lose.
    pub fn diff(&self) -> Vec<String> {
        if !self.changed() {
            return vec![];
        }
        let old: Vec<&str> = match &self.seen {
            Seen::Text(t) => t.lines().collect(),
            Seen::Unreadable { .. } => return vec![],
            Seen::Absent => vec![],
        };
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

pub fn files_for(
    devices: &[Device],
    group: Option<&str>,
    manager: Option<LinkManager>,
    share_user: Option<&str>,
    read: &dyn Fn(&str) -> Seen,
) -> Vec<FileChange> {
    let mut files = Vec::new();
    let mut modules: Vec<String> = Vec::new();
    for d in devices {
        if !d.host.gadget.is_empty() {
            let dst = format!("{UDEV_DIR}/70-lpoco-{}.rules", d.codename);
            files.push(FileChange {
                seen: read(&dst),
                content: rules_text(d, group),
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
    if let Some(m) = manager {
        for (name, address) in net_links(devices) {
            let dst = m.file_for(&name);
            files.push(FileChange {
                seen: read(&dst),
                content: m.text(&name, &address),
                serials: vec![],
                dst,
            });
        }
    }
    if let Some(user) = share_user.filter(|u| share::valid_user(u)) {
        for (dst, content) in [
            (share::SHARE_HELPER_DST, share::helper_text()),
            (share::SHARE_UNIT_DST, share::unit_text()),
            (share::POLKIT_DST, share::polkit_text(user)),
        ] {
            files.push(FileChange {
                seen: read(dst),
                content,
                serials: vec![],
                dst: dst.into(),
            });
        }
    }
    if !modules.is_empty() {
        modules.sort();
        files.push(FileChange {
            dst: MODULES_FILE.into(),
            content: modules_text(&modules),
            seen: read(MODULES_FILE),
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

/// Each declared command once, with every device that wants it, sorted by command.
pub fn commands_of(devices: &[Device]) -> Vec<(String, Vec<String>)> {
    let mut m: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for d in devices {
        for c in &d.host.commands {
            let w = m.entry(c.clone()).or_default();
            if !w.contains(&d.codename) {
                w.push(d.codename.clone());
            }
        }
    }
    m.into_iter().collect()
}

/// Each declared cross target once, with every device that wants it, sorted by target.
pub fn targets_of(devices: &[Device]) -> Vec<(String, Vec<String>)> {
    let mut m: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for d in devices {
        for t in &d.host.targets {
            let w = m.entry(t.clone()).or_default();
            if !w.contains(&d.codename) {
                w.push(d.codename.clone());
            }
        }
    }
    m.into_iter().collect()
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
            seen: Seen::Text("a\nb\n".into()),
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

    #[test]
    fn unasked_watchers_are_the_enabled_ones_no_asking_device_maps_to() {
        let mut asks = dev("/h/Projects/linux-phone-oco/a-lab/device.toml", None);
        asks.host.watch = true;
        let silent = dev("/h/Projects/linux-phone-oco/b-lab/device.toml", None);
        let enabled: Vec<String> = ["b-lab", "a-lab", "gone-lab", "b-lab"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            unasked_watchers("/h", &[asks, silent], &enabled),
            vec!["b-lab".to_string(), "gone-lab".to_string()]
        );
    }

    #[test]
    fn commands_are_listed_once_with_every_device_that_wants_them() {
        let mut a = dev("a/device.toml", None);
        a.codename = "a".into();
        a.host.commands = vec!["git".into(), "zip".into()];
        let mut b = dev("b/device.toml", None);
        b.codename = "b".into();
        b.host.commands = vec!["git".into(), "git".into()];
        assert_eq!(
            commands_of(&[a, b]),
            vec![
                ("git".to_string(), vec!["a".to_string(), "b".to_string()]),
                ("zip".to_string(), vec!["a".to_string()]),
            ]
        );
    }

    fn dev(path: &str, serial: Option<&str>) -> Device {
        let gadget = serial.map(|s| Gadget {
            serial: s.into(),
            access: vec![Access::Block],
            net_name: None,
            net_address: None,
        });
        Device {
            path: path.into(),
            codename: "w".into(),
            host: HostTable {
                watch: false,
                modules: vec![],
                commands: vec![],
                targets: vec![],
                gadget: gadget.into_iter().collect(),
            },
            lpoco_pin: None,
            fastboot_serial: None,
            usb_address: None,
            pieces: Vec::new(),
            user_configs: Vec::new(),
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
    if let Some(g) = &lab.gadget_group {
        safe_word("gadget_group", g)?;
    }
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

/// Is `user` in `group`: named in its member list in /etc/group, or with the group as primary gid in /etc/passwd?
/// `None` when the group is not there; an empty user is in no group.
pub fn group_has_member(
    etc_group: &str,
    etc_passwd: &str,
    group: &str,
    user: &str,
) -> Option<bool> {
    for line in etc_group.lines() {
        let mut f = line.split(':');
        if f.next() == Some(group) {
            if user.is_empty() {
                return Some(false);
            }
            let gid = f.nth(1).unwrap_or("");
            let members = f.next().unwrap_or("");
            let primary = etc_passwd.lines().any(|p| {
                let p: Vec<&str> = p.split(':').collect();
                p.first() == Some(&user) && p.get(3).is_some_and(|g| !gid.is_empty() && *g == gid)
            });
            return Some(primary || members.split(',').any(|m| m == user));
        }
    }
    None
}

/// A USB gadget (vendor 1d6b) the kernel log says is enumerated with a serial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenGadget {
    /// the kernel's name for the USB device, e.g. `usb 1-2`
    pub device: String,
    pub serial: String,
    /// the log's time stamp of the serial line, when the line has one
    pub when: Option<String>,
}

/// The gadgets (vendor 1d6b, the Linux Foundation's) a kernel log (`journalctl -k` lines) says are enumerated
/// with a serial now: the latest serial of each `usb B-P` device, dropped at its `USB disconnect`. A root hub
/// (`usb usbN`, which also carries 1d6b) is not a gadget.
pub fn gadgets_seen(log: &str) -> Vec<SeenGadget> {
    let mut gadget: Vec<String> = Vec::new();
    let mut now: Vec<SeenGadget> = Vec::new();
    for line in log.lines() {
        let (when, msg) = match line.split_once(" kernel: ") {
            Some((h, m)) => (
                Some(h.split_whitespace().take(3).collect::<Vec<_>>().join(" ")),
                m,
            ),
            None => (None, line),
        };
        let Some((dev, rest)) = msg.split_once(": ") else {
            continue;
        };
        if !dev.starts_with("usb ") || dev.starts_with("usb usb") {
            continue;
        }
        if rest.starts_with("New USB device found") {
            gadget.retain(|d| d != dev);
            now.retain(|g| g.device != dev);
            if rest.contains("idVendor=1d6b,") {
                gadget.push(dev.to_string());
            }
        } else if rest.starts_with("USB disconnect") {
            gadget.retain(|d| d != dev);
            now.retain(|g| g.device != dev);
        } else if let Some(serial) = rest.strip_prefix("SerialNumber: ") {
            if gadget.iter().any(|d| d == dev) {
                now.retain(|g| g.device != dev);
                now.push(SeenGadget {
                    device: dev.to_string(),
                    serial: serial.trim().to_string(),
                    when,
                });
            }
        }
    }
    now
}

/// The enumerated gadgets whose serial none of `declared` names.
pub fn undeclared_gadgets(log: &str, declared: &[String]) -> Vec<SeenGadget> {
    gadgets_seen(log)
        .into_iter()
        .filter(|g| !declared.contains(&g.serial))
        .collect()
}

#[cfg(test)]
mod gadget_log_tests {
    use super::*;

    const HUB: &str = "Nov 18 09:00:01 lab kernel: usb usb1: New USB device found, idVendor=1d6b, idProduct=0002, bcdDevice= 7.03\nNov 18 09:00:01 lab kernel: usb usb1: SerialNumber: 0000:02:00.0\n";

    #[test]
    fn a_root_hub_is_not_a_gadget() {
        assert!(gadgets_seen(HUB).is_empty());
    }

    #[test]
    fn a_serial_fixed_or_unplugged_is_no_longer_seen() {
        let log = format!(
            "{HUB}Nov 18 10:00:00 lab kernel: usb 1-2: New USB device found, idVendor=1d6b, idProduct=0104\nNov 18 10:00:00 lab kernel: usb 1-2: SerialNumber: bad\nNov 18 10:01:00 lab kernel: usb 1-2: USB disconnect, device number 7\n"
        );
        assert!(gadgets_seen(&log).is_empty());
        let log = format!(
            "{log}Nov 18 10:02:00 lab kernel: usb 1-2: New USB device found, idVendor=1d6b, idProduct=0104\nNov 18 10:02:00 lab kernel: usb 1-2: SerialNumber: good\n"
        );
        let g = gadgets_seen(&log);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].serial, "good");
        assert_eq!(g[0].when.as_deref(), Some("Nov 18 10:02:00"));
    }

    #[test]
    fn a_line_without_a_stamp_has_no_time() {
        let g = gadgets_seen("usb 1-2: New USB device found, idVendor=1d6b, idProduct=0104\nusb 1-2: SerialNumber: x\n");
        assert_eq!(g[0].when, None);
    }
}

#[cfg(test)]
mod group_tests {
    use super::group_has_member as has;
    const G: &str = "users:x:982:alice,mll\nusers2:x:983:bob\nempty:x:984:\n";
    const P: &str = "carol:x:1001:982::/home/carol:/bin/sh\n";

    #[test]
    fn membership_by_list_by_primary_gid_and_never_by_prefix_or_empty() {
        assert_eq!(has(G, P, "users", "mll"), Some(true));
        assert_eq!(has(G, P, "users", "bob"), Some(false));
        assert_eq!(has(G, P, "users", "carol"), Some(true));
        assert_eq!(has(G, P, "users", ""), Some(false));
        assert_eq!(has(G, P, "empty", ""), Some(false));
        assert_eq!(has(G, P, "empty", "mll"), Some(false));
        assert_eq!(has(G, P, "nope", "mll"), None);
    }
}

/// The user unit that replaces lab-machine's `setup/devlab-watch.sh`: one `devlab watch` per device found, the instance
/// named by the folder of the device's lab repository (as `%i`).
pub const WATCH_UNIT: &str = "devlab-watch@.service";
/// The tool whose binary the watcher units run (its second name is devlab): replaced, the units are restarted.
pub const WATCH_TOOL: &str = "lpoco-lab";

pub fn watch_unit_text() -> String {
    "[Unit]\nDescription=devlab watch: the health of the device whose lab repository is %i\nAfter=network-online.target\n\n[Service]\nExecStart=%h/.local/bin/devlab --device %h/Projects/linux-phone-oco/%i/device.toml watch\nRestart=on-failure\nRestartSec=30\n\n[Install]\nWantedBy=default.target\n".to_string()
}

/// The instance each device is watched by: its lab repository's folder name, for a device whose manifest lies at
/// `HOME/Projects/linux-phone-oco/NAME-lab/device.toml` (the path the unit template names). A worktree (`NAME-lab-wt`,
/// `wt-...`) or a template repository is not watched: it goes away or is no device. A device whose
/// `[host]` does not say `watch = true` is not watched either.
pub fn watch_instances(home: &str, devices: &[Device]) -> Vec<String> {
    let base = format!("{home}/Projects/linux-phone-oco");
    let mut v: Vec<String> = devices
        .iter()
        .filter(|d| d.host.watch)
        .filter_map(|d| {
            let dir = d.path.parent()?;
            if dir.parent()? != Path::new(&base) {
                return None;
            }
            let n = dir.file_name()?.to_string_lossy().into_owned();
            n.ends_with("-lab").then_some(n)
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The watchers, worked out without touching anything: the template file, and the user-level commands (no root) that
/// bring each instance up. `enabled` says whether an instance already has its wants link, `active` whether it runs
/// (an enabled one that does not is started); `binary_replaced` that this
/// run replaces the binary the units run, so each enabled one is restarted after the tools.
pub fn watch_plan(
    home: &str,
    devices: &[Device],
    read: &dyn Fn(&str) -> Option<String>,
    enabled: &dyn Fn(&str) -> bool,
    active: &dyn Fn(&str) -> bool,
    binary_replaced: bool,
) -> (FileChange, Vec<Vec<String>>) {
    let dst = format!("{home}/.config/systemd/user/{WATCH_UNIT}");
    let template = FileChange {
        seen: read(&dst).into(),
        content: watch_unit_text(),
        serials: vec![],
        dst,
    };
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let mut cmds = Vec::new();
    let instances = watch_instances(home, devices);
    if template.changed() && !instances.is_empty() {
        cmds.push(s(&["systemctl", "--user", "daemon-reload"]));
    }
    for i in &instances {
        let unit = format!("devlab-watch@{i}.service");
        if !enabled(i) {
            cmds.push(s(&["systemctl", "--user", "enable", "--now", &unit]));
        } else if binary_replaced || template.changed() {
            cmds.push(s(&["systemctl", "--user", "restart", &unit]));
        } else if !active(i) {
            // linked but stopped (failed, or stopped by hand): enabled alone would leave it down until the next login
            cmds.push(s(&["systemctl", "--user", "start", &unit]));
        }
    }
    (template, cmds)
}

/// The watcher instances already enabled that no device asks for: named in the plan, never stopped by prepare - a
/// watcher someone started may be wanted, and stopping it silently would hide the change.
pub fn unasked_watchers(home: &str, devices: &[Device], enabled: &[String]) -> Vec<String> {
    let asked = watch_instances(home, devices);
    let mut v: Vec<String> = enabled
        .iter()
        .filter(|i| !asked.contains(i))
        .cloned()
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The rule files of a host that are not this tool's, named when they could cross its own: a file in the rules
/// directory that is not a `70-lpoco-` file and not the kept wide rule, and that either names a serial a device
/// declares or sets MODE 0666. Each is (file name, why). Only named; prepare never edits or removes one.
/// `files` are (file name, text) pairs.
pub fn foreign_rules(files: &[(String, String)], serials: &[String]) -> Vec<(String, String)> {
    let kept = KEPT_RULE.rsplit('/').next().unwrap_or(KEPT_RULE);
    let mut out: Vec<(String, String)> = Vec::new();
    for (name, text) in files {
        if name == kept || name.starts_with("70-lpoco-") || !name.ends_with(".rules") {
            continue;
        }
        let mut why: Vec<String> = Vec::new();
        let live: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .collect();
        for s in serials {
            let quoted = format!("\"{s}\"");
            if live.iter().any(|l| l.contains(&quoted)) {
                why.push(format!("names our gadget serial {s}"));
            }
        }
        if live.iter().any(|l| sets_mode_0666(l)) {
            why.push("sets MODE 0666".to_string());
        }
        if !why.is_empty() {
            out.push((name.clone(), why.join(", ")));
        }
    }
    out.sort();
    out
}

/// Whether a rule line assigns MODE 0666 (or 666), as `MODE="0666"` or `MODE:="0666"`.
fn sets_mode_0666(line: &str) -> bool {
    let mut rest = line;
    while let Some(i) = rest.find("MODE") {
        let after = rest[i + 4..].trim_start();
        let after = after
            .strip_prefix(":=")
            .or_else(|| after.strip_prefix('='))
            .unwrap_or("");
        let v = after.trim_start().trim_start_matches('"');
        if v.starts_with("0666") || (v.starts_with("666") && !v.starts_with("6666")) {
            return true;
        }
        rest = &rest[i + 4..];
    }
    false
}
