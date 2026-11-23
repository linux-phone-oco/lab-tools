// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-layer: a phone's device layer ([layer] in device.toml, typed in lpoco-manifest) as packages. The
//! declared files, and kernel-install's devicetree file, go into `NAME-config`, the same in every format; the glue a format's base needs - the
//! systemd preset, the initramfs module list in the initramfs tool's own form - goes into
//! `NAME-boot` (for apk only the mkinitfs module list and the scriptlet that enables it). Each package is an nfpm
//! config; nfpm packs it. The dependency token map is `[layer.names.FORMAT]`; the kernel package (`linux-NAME`) is not here yet.

use lpoco_manifest::{Boot, BootMethod, FileKind, Layer, Manifest};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Apk,
    Deb,
    Rpm,
    Archlinux,
}

impl Format {
    pub const TOKENS: &'static [&'static str] = &["apk", "deb", "rpm", "archlinux"];

    pub fn parse(s: &str) -> Option<Format> {
        Some(match s.trim() {
            "apk" => Format::Apk,
            "deb" => Format::Deb,
            "rpm" => Format::Rpm,
            "archlinux" => Format::Archlinux,
            _ => return None,
        })
    }

    pub fn token(self) -> &'static str {
        Self::TOKENS[self as usize]
    }

    /// The file a package of this format is written to, in the form each format's tools expect.
    pub fn file_name(self, name: &str, version: &str) -> String {
        self.file_name_for(name, version, None)
    }

    /// The same for a package built for one architecture (`arm64`, as nfpm says it); none is architecture-independent.
    pub fn file_name_for(self, name: &str, version: &str, arch: Option<&str>) -> String {
        let arm = arch.is_some();
        match self {
            Format::Apk => format!("{name}-{version}-r0.apk"),
            Format::Deb => format!("{name}_{version}_{}.deb", arch.unwrap_or("all")),
            Format::Rpm => format!(
                "{name}-{version}-1.{}.rpm",
                if arm { "aarch64" } else { "noarch" }
            ),
            Format::Archlinux => format!(
                "{name}-{version}-1-{}.pkg.tar.zst",
                if arm { "aarch64" } else { "any" }
            ),
        }
    }
}

/// Where a package's file comes from: the layer's tree (relative to the manifest) or text this crate writes.
#[derive(Clone, PartialEq, Debug)]
pub enum Source {
    Tree(String),
    Text(String),
    /// A whole directory, packed as a tree.
    Dir(String),
}

#[derive(Clone, PartialEq, Debug)]
pub struct Item {
    pub source: Source,
    pub dst: String,
    pub mode: u32,
    pub config: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Package {
    pub name: String,
    /// The architecture it is built for (`arm64`); none when it holds the same files for every phone.
    pub arch: Option<&'static str>,
    pub description: String,
    pub depends: Vec<String>,
    pub items: Vec<Item>,
    /// A shell script the package manager runs after installing it.
    pub post_install: Option<String>,
}

/// An item with its source made a file on disk.
#[derive(Clone, PartialEq, Debug)]
pub struct Entry {
    pub src: PathBuf,
    pub dst: String,
    pub mode: u32,
    pub config: bool,
    pub tree: bool,
}

/// A dependency token with its name in this format's repositories, its version bound kept.
fn named(layer: &Layer, format: Format, token: &str) -> String {
    let at = token.find(['<', '>', '=']).unwrap_or(token.len());
    let (name, bound) = token.split_at(at);
    match layer
        .names
        .of(format.token())
        .and_then(|m| m.get(name.trim()))
    {
        Some(n) => format!("{n}{bound}"),
        None => token.to_string(),
    }
}

pub fn packages(layer: &Layer, format: Format) -> Vec<Package> {
    let name = &layer.name;
    let mut out = Vec::new();
    let mut config = Vec::new();
    for f in &layer.file {
        config.push(Item {
            source: Source::Tree(f.from.clone()),
            dst: f.to.clone(),
            mode: if f.kind == FileKind::Script {
                0o755
            } else {
                0o644
            },
            config: f.kind == FileKind::Config,
        });
    }
    let text = |dst: String, body: String, mode: u32| Item {
        source: Source::Text(body),
        dst,
        mode,
        config: false,
    };
    if let (Some(b), Some(k)) = (&layer.boot, &layer.kernel) {
        // kernel-install(8) installs the one dtb this file names; a phone with several variants gets its first.
        // It is in the config package of every format, so the formats hold the same files (Alpine never reads it).
        if b.method == BootMethod::EfiKernelInstall {
            if let Some(dtb) = k.dtbs.first() {
                config.push(text(
                    "/usr/lib/kernel/devicetree".into(),
                    format!("{dtb}\n"),
                    0o644,
                ));
            }
        }
    }
    if !config.is_empty() {
        out.push(Package {
            arch: None,
            name: format!("{name}-config"),
            description: format!("{name}: device configuration"),
            depends: layer
                .needs
                .iter()
                .map(|n| named(layer, format, n))
                .collect(),
            items: config,
            post_install: None,
        });
    }
    if format == Format::Apk {
        // Alpine runs OpenRC, so no systemd preset. mkinitfs globs each line of features.d/*.modules against the
        // files under /lib/modules/KERNEL (a bare name matches nothing), and a feature counts only when
        // mkinitfs.conf names it: the scriptlet adds it once and keeps what the user set.
        if let Some(i) = layer.initramfs.as_ref().filter(|i| !i.modules.is_empty()) {
            out.push(Package {
                arch: None,
                name: format!("{name}-boot"),
                description: format!("{name}: boot glue (mkinitfs modules)"),
                depends: vec![format!("{name}-config"), "mkinitfs".to_string()],
                items: vec![text(
                    format!("/etc/mkinitfs/features.d/{name}.modules"),
                    mkinitfs_globs(&i.modules),
                    0o644,
                )],
                post_install: Some(format!(
                    "#!/bin/sh\n# mkinitfs builds a feature in only when mkinitfs.conf lists it; add {name} once\nconf=/etc/mkinitfs/mkinitfs.conf\nif [ ! -f \"$conf\" ]; then\n    echo \"{name}: no $conf; add {name} to its features by hand\" >&2\nelif grep -q '^features=\"\\([^\"]* \\)\\{{0,1\\}}{re}\\( [^\"]*\\)\\{{0,1\\}}\"$' \"$conf\"; then\n    :\nelif grep -q '^features=\"[^\"]*\"$' \"$conf\"; then\n    sed -i 's/^features=\"\\(.*\\)\"$/features=\"\\1 {name}\"/' \"$conf\"\nelse\n    echo \"{name}: no one-line features=\\\"...\\\" in $conf; add {name} to it by hand\" >&2\nfi\n",
                    re = name.replace('.', "\\.")
                )),
            });
        }
        return out;
    }
    let mut boot = Vec::new();
    let units: Vec<&str> = layer
        .file
        .iter()
        .filter(|f| f.kind == FileKind::Unit)
        .filter_map(|f| f.to.rsplit('/').next())
        .filter(|u| {
            [".service", ".socket", ".timer", ".path", ".mount"]
                .iter()
                .any(|s| u.ends_with(s))
        })
        .collect();
    let mut post_install = None;
    if !units.is_empty() {
        let first = match format {
            Format::Deb => "[ -z \"$2\" ]",
            Format::Rpm => "[ \"$1\" = 1 ]",
            _ => "true",
        };
        post_install = Some(format!(
            "#!/bin/sh\n# the units are in {name}-config; enable them as the preset says, on first install only\n# (an upgrade keeps what the user chose); a chroot has no running systemd and presets as well\nif command -v systemctl >/dev/null 2>&1 && {first}; then\n    systemctl preset {} || echo \"{name}: systemctl preset failed; enable the units by hand\" >&2\nfi\n",
            units.join(" ")
        ));
        let body: String = units.iter().map(|u| format!("enable {u}\n")).collect();
        boot.push(text(
            format!("/usr/lib/systemd/system-preset/50-{name}.preset"),
            body,
            0o644,
        ));
    }
    if let Some(i) = layer.initramfs.as_ref().filter(|i| !i.modules.is_empty()) {
        let mods = &i.modules;
        boot.push(match format {
            Format::Deb => text(
                format!("/usr/share/initramfs-tools/hooks/{name}"),
                format!(
                    "#!/bin/sh\nPREREQ=\"\"\nprereqs() {{ echo \"$PREREQ\"; }}\ncase \"$1\" in prereqs) prereqs; exit 0 ;; esac\n. /usr/share/initramfs-tools/hook-functions\n{}",
                    mods.iter().map(|m| format!("manual_add_modules {m}\n")).collect::<String>()
                ),
                0o755,
            ),
            Format::Rpm => text(
                format!("/usr/lib/dracut/dracut.conf.d/50-{name}.conf"),
                format!("add_drivers+=\" {} \"\n", mods.join(" ")),
                0o644,
            ),
            _ => text(
                format!("/etc/mkinitcpio.conf.d/{name}.conf"),
                format!("MODULES+=({})\n", mods.join(" ")),
                0o644,
            ),
        });
    }
    if !boot.is_empty() {
        out.push(Package {
            arch: None,
            name: format!("{name}-boot"),
            description: format!("{name}: boot glue (systemd preset, initramfs modules)"),
            depends: if post_install.is_some() {
                vec![format!("{name}-config")]
            } else {
                Vec::new()
            },
            items: boot,
            post_install,
        });
    }
    out
}

/// Makes every item a file: tree files are checked to exist, text is written under `stage`.
pub fn materialize(pkg: &Package, manifest_dir: &Path, stage: &Path) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    for it in &pkg.items {
        let src = match &it.source {
            Source::Tree(rel) => {
                let p = manifest_dir.join(rel);
                if !p.is_file() {
                    return Err(format!(
                        "{}: {rel} is not a file under {}",
                        pkg.name,
                        manifest_dir.display()
                    ));
                }
                p
            }
            Source::Dir(rel) => {
                let p = manifest_dir.join(rel);
                if !p.is_dir() {
                    return Err(format!(
                        "{}: {rel} is not a folder under {}",
                        pkg.name,
                        manifest_dir.display()
                    ));
                }
                p
            }
            Source::Text(body) => {
                let p = stage.join(&pkg.name).join(it.dst.trim_start_matches('/'));
                fs::create_dir_all(p.parent().unwrap())
                    .map_err(|e| format!("{}: {e}", p.display()))?;
                fs::write(&p, body).map_err(|e| format!("{}: {e}", p.display()))?;
                p
            }
        };
        out.push(Entry {
            src,
            dst: it.dst.clone(),
            mode: it.mode,
            config: it.config,
            tree: matches!(it.source, Source::Dir(_)),
        });
    }
    Ok(out)
}

/// A dependency token as the format writes it: `tqftpserv>=1.2` is `tqftpserv (>= 1.2)` in a deb, with spaces in an rpm.
fn dependency(token: &str, format: Format) -> Result<String, String> {
    let Some(at) = token.find(['<', '>', '=']) else {
        return Ok(token.trim().to_string());
    };
    let (name, rest) = token.split_at(at);
    let (name, rest) = (name.trim(), rest.trim());
    let op_len = rest.chars().take_while(|c| "<>=".contains(*c)).count();
    let (op, ver) = rest.split_at(op_len);
    let ver = ver.trim();
    if name.is_empty() || ver.is_empty() || !["<", ">", "<=", ">=", "="].contains(&op) {
        return Err(format!(
            "dependency {token:?} is not NAME or NAME<op>VERSION with < > <= >= ="
        ));
    }
    Ok(match format {
        // Debian's bare < and > mean <= and >=: its strict forms are << and >>
        Format::Deb => {
            let op = match op {
                "<" => "<<",
                ">" => ">>",
                o => o,
            };
            format!("{name} ({op} {ver})")
        }
        Format::Rpm => format!("{name} {op} {ver}"),
        _ => format!("{name}{op}{ver}"),
    })
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn nfpm_yaml(
    pkg: &Package,
    format: Format,
    version: &str,
    entries: &[Entry],
    script: Option<&Path>,
) -> Result<String, String> {
    let mut y = format!(
        "name: {}\narch: {}\nplatform: linux\nversion: {}\nsection: misc\nmaintainer: linux-phone-oco\ndescription: {}\nlicense: MIT OR Apache-2.0\n",
        pkg.name,
        pkg.arch.unwrap_or("all"),
        quote(version),
        quote(&pkg.description)
    );
    if !pkg.depends.is_empty() {
        y.push_str("depends:\n");
        for d in &pkg.depends {
            y.push_str(&format!("- {}\n", dependency(d, format)?));
        }
    }
    if let Some(s) = script {
        y.push_str(&format!(
            "scripts:\n  postinstall: {}\n",
            quote(&s.to_string_lossy())
        ));
    }
    y.push_str("contents:\n");
    for e in entries {
        y.push_str(&format!(
            "- src: {}\n  dst: {}\n",
            quote(&e.src.to_string_lossy()),
            quote(&e.dst)
        ));
        if e.tree {
            // a tree keeps each file's own mode: nfpm would otherwise stamp one mode on every file in it
            y.push_str("  type: tree\n");
        } else {
            y.push_str(&format!("  file_info:\n    mode: {:04o}\n", e.mode));
        }
        if e.config {
            y.push_str("  type: config|noreplace\n");
        }
    }
    Ok(y)
}

/// What would make a package wrong, said before anything is written: kernel-install without a dtb to name, and
/// initramfs module names that are not one word of kernel-module characters (they go into a shell hook and a config line).
pub fn check(layer: &Layer, format: Format) -> Result<(), String> {
    if let Some(b) = &layer.boot {
        if b.method == BootMethod::EfiKernelInstall
            && layer.kernel.as_ref().is_none_or(|k| k.dtbs.is_empty())
        {
            return Err(
                "boot method efi-kernel-install needs [layer.kernel] dtbs: it names the devicetree"
                    .into(),
            );
        }
    }
    // apk writes no preset: the unit names go into a root scriptlet and a preset line, one word each
    if format != Format::Apk {
        for f in layer.file.iter().filter(|f| f.kind == FileKind::Unit) {
            let u = f.to.rsplit('/').next().unwrap_or("");
            if !u
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
            {
                return Err(format!(
                    "unit name {u:?} is not a systemd unit name (letters, digits, _ - . @ : only)"
                ));
            }
        }
    }
    // every format writes the modules into a script or a config line
    if let Some(i) = &layer.initramfs {
        for m in &i.modules {
            if m.is_empty()
                || !m
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(format!(
                    "initramfs module {m:?} is not a kernel module name (letters, digits, _ and - only)"
                ));
            }
        }
    }
    Ok(())
}

/// Packages of this layer's name that an earlier run left in `out` and this run does not make (its `-boot`
/// when the layer has no boot glue now); the caller removes them so nobody installs a stale one.
pub fn stale(
    layer: &Layer,
    format: Format,
    version: &str,
    out: &Path,
    with_kernel: bool,
) -> Vec<PathBuf> {
    let mut made: Vec<String> = packages(layer, format)
        .into_iter()
        .map(|p| p.name)
        .collect();
    if with_kernel {
        made.push(format!("linux-{}", layer.name));
    }
    let arm = |n: &str| (n.starts_with("linux-").then_some("arm64"), n.to_string());
    ["config", "boot"]
        .iter()
        .map(|s| format!("{}-{s}", layer.name))
        .chain([format!("linux-{}", layer.name)])
        .filter(|n| !made.contains(n))
        .map(|n| arm(&n))
        .map(|(a, n)| out.join(format.file_name_for(&n, version, a)))
        .filter(|p| p.exists())
        .collect()
}

/// Writes the package's post-install script under `stage` and says where; none when the package has none.
pub fn post_install(pkg: &Package, stage: &Path) -> Result<Option<PathBuf>, String> {
    let Some(body) = &pkg.post_install else {
        return Ok(None);
    };
    let p = stage.join(&pkg.name).join("scripts/postinstall");
    fs::create_dir_all(p.parent().unwrap()).map_err(|e| format!("{}: {e}", p.display()))?;
    fs::write(&p, body).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(Some(p))
}

/// The kernel package `linux-NAME` from a finished build, laid out where kernel-install(8) looks: the image
/// as `/usr/lib/modules/RELEASE/vmlinuz`, each declared dtb under `.../RELEASE/dtb/` by the path
/// `/usr/lib/kernel/devicetree` names, and the build's module tree (`modules/`, when there is one) as
/// `/usr/lib/modules/RELEASE`. The build folder holds `Image`, `kernel.release`, `dtbs/` and optionally `modules/`.
pub fn kernel_package(layer: &Layer, build: &Path) -> Result<Package, String> {
    let k = layer
        .kernel
        .as_ref()
        .ok_or("a kernel build is given but the layer declares no [layer.kernel]")?;
    let file = |rel: &str| -> Result<String, String> {
        let p = build.join(rel);
        if p.is_file() {
            Ok(p.to_string_lossy().into_owned())
        } else {
            Err(format!("{}: {rel} is not in the kernel build", p.display()))
        }
    };
    let image = file("Image")?;
    let rel_file = build.join("kernel.release");
    file("kernel.release")?;
    let release = fs::read_to_string(&rel_file)
        .map_err(|e| format!("{}: {e}", rel_file.display()))?
        .trim()
        .to_string();
    if release.is_empty()
        || release.starts_with('.')
        || !release
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
    {
        return Err(format!(
            "kernel release {release:?} is not one word of letters, digits and . _ + - (it names a folder under /usr/lib/modules)"
        ));
    }
    let base = format!("/usr/lib/modules/{release}");
    let mut items = vec![Item {
        source: Source::Tree(image),
        dst: format!("{base}/vmlinuz"),
        mode: 0o644,
        config: false,
    }];
    for d in &k.dtbs {
        items.push(Item {
            source: Source::Tree(file(&format!("dtbs/{d}"))?),
            dst: format!("{base}/dtb/{d}"),
            mode: 0o644,
            config: false,
        });
    }
    let modules = build.join("modules");
    for clash in ["vmlinuz", "dtb"] {
        if modules.join(clash).exists() {
            return Err(format!(
                "{}: modules/{clash} would overwrite the kernel package's own file; modules/ must be a clean module tree",
                build.display()
            ));
        }
    }
    if modules.is_dir() {
        if fs::symlink_metadata(&modules)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(format!(
                "{}: modules is a link; it must be the module tree itself",
                build.display()
            ));
        }
        outside_links(build, &modules, &modules)?;
        items.push(Item {
            source: Source::Dir(modules.to_string_lossy().into_owned()),
            dst: base,
            mode: 0o755,
            config: false,
        });
    }
    Ok(Package {
        name: format!("linux-{}", layer.name),
        arch: Some("arm64"),
        description: format!(
            "{}: Linux {release}, its device trees and modules",
            layer.name
        ),
        depends: Vec::new(),
        items,
        post_install: None,
    })
}

/// The lines of an mkinitfs `features.d/*.modules` file for these module names: mkinitfs globs each line under
/// /lib/modules/KERNEL, and a module's folder is not declared, so each name (in both spellings, `-` and `_`)
/// is tried at depths 1 to 8 under kernel/ (the deepest driver in a stock kernel sits at 7; one deeper is not found); a glob that matches nothing is skipped by mkinitfs.
fn mkinitfs_globs(modules: &[String]) -> String {
    let mut out = String::new();
    for m in modules {
        let mut spellings = vec![m.clone()];
        for s in [m.replace('_', "-"), m.replace('-', "_")] {
            if !spellings.contains(&s) {
                spellings.push(s);
            }
        }
        for dirs in 1..=8 {
            for s in &spellings {
                out.push_str(&format!("kernel/{}{s}.ko*\n", "*/".repeat(dirs)));
            }
        }
    }
    out
}

/// Refuses a symlink under the module tree whose target is absolute, has a `..` after a name, or climbs out of the tree: a kernel build's
/// `build` and `source` links name the build machine's folders and would dangle on the phone.
fn outside_links(build: &Path, root: &Path, dir: &Path) -> Result<(), String> {
    let rd = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in rd {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.is_dir() {
            outside_links(build, root, &path)?;
        } else if meta.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let mut depth = rel.components().count() as i64 - 1;
            let mut out = target.is_absolute();
            let mut named = false;
            for c in target.components() {
                match c {
                    std::path::Component::ParentDir => {
                        // a `..` after a name could climb through another link: refused, not resolved
                        out |= named;
                        depth -= 1;
                    }
                    std::path::Component::Normal(_) => named = true,
                    _ => {}
                }
            }
            if out || depth < 0 {
                return Err(format!(
                    "{}: modules/{} links to {}, outside the module tree (it would dangle on the phone); remove it, as a package's modules/ holds no build or source link",
                    build.display(),
                    rel.display(),
                    target.display()
                ));
            }
        }
    }
    Ok(())
}

/// Nura's (postmarketOS's) `/etc/deviceinfo`, which overrides the device package's `/usr/share/deviceinfo/deviceinfo` (so it has no format_version, name or manufacturer): the shell-sourced file mkinitfs and boot-deploy read the phone's
/// boot facts from, written from the manifest's `[boot]` and `[device]`; none when the manifest has no `[boot]`.
/// Only what the manifest declares is written. Its values are sourced by shell, so a cmdline holding a quote,
/// `$`, a backtick or a backslash is refused.
pub fn deviceinfo(m: &Manifest) -> Result<Option<String>, String> {
    let Some(b) = &m.boot else {
        return Ok(None);
    };
    if let Some(c) = b.cmdline.chars().find(|c| "\"$`\\\n".contains(*c)) {
        return Err(format!(
            "[boot] cmdline holds {c:?}: deviceinfo is read by shell scripts, which would run or mangle it"
        ));
    }
    let mut s = String::new();
    if let Some(d) = &m.device {
        if !d
            .codename
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
            || d.codename.is_empty()
        {
            return Err(format!("[device] codename {:?} is not one word of letters, digits and _ - .: deviceinfo is read by shell scripts", d.codename));
        }
        s += &format!("deviceinfo_codename=\"{}\"\n", d.codename);
    }
    s += "deviceinfo_arch=\"aarch64\"\n";
    s += &format!("deviceinfo_header_version=\"{}\"\n", b.header_version);
    s += "deviceinfo_generate_bootimg=\"true\"\n";
    s += &format!("deviceinfo_flash_pagesize=\"{}\"\n", b.pagesize);
    for (k, v) in [
        ("base", b.base),
        ("kernel", b.kernel_offset),
        ("ramdisk", b.ramdisk_offset),
        ("second", b.second_offset),
        ("tags", b.tags_offset),
    ] {
        s += &format!("deviceinfo_flash_offset_{k}=\"0x{v:08x}\"\n");
    }
    s += &format!("deviceinfo_kernel_cmdline=\"{}\"\n", b.cmdline);
    Ok(Some(s))
}

/// Puts `/etc/deviceinfo` into `NAME-config` (made when the layer declares no file).
pub fn add_deviceinfo(pkgs: &mut Vec<Package>, layer: &Layer, body: &str, cmdline: &str) {
    let name = &layer.name;
    let item = Item {
        source: Source::Text(body.to_string()),
        dst: "/etc/deviceinfo".into(),
        mode: 0o644,
        config: false,
    };
    // the same cmdline in the file boot-deploy and kernel-install read now (pmaports marks the deviceinfo key obsolete);
    // `cmdline` has passed `deviceinfo`'s refusal of what a shell would run, which also keeps it to one line
    let cmdline_d = Item {
        source: Source::Text(format!("{cmdline}\n")),
        dst: format!("/usr/lib/kernel-cmdline.d/50-{name}.conf"),
        mode: 0o644,
        config: false,
    };
    let want = format!("{name}-config");
    match pkgs.iter_mut().find(|p| p.name == want) {
        Some(p) => p.items.extend([item, cmdline_d]),
        None => pkgs.insert(
            0,
            Package {
                arch: None,
                name: want,
                description: format!("{name}: device configuration"),
                depends: layer
                    .needs
                    .iter()
                    .map(|n| named(layer, Format::Apk, n))
                    .collect(),
                items: vec![item, cmdline_d],
                post_install: None,
            },
        ),
    }
}

/// The one script that writes the phone's boot image when a kernel is installed (`/usr/libexec/NAME-bootimg`),
/// made from `[boot]` and the layer's first device tree. The base's trigger only calls it:
///
///   NAME-bootimg KVER --kernel IMAGE --initrd RAMDISK --out BOOT.IMG    one image
///   NAME-bootimg --alpm                                                 each kernel a libalpm hook names on stdin
///
/// The device tree is appended to the kernel (as `Image.gz-dtb` is) and mkbootimg packs the header the manifest
/// declares; the image is written beside its target and moved in, so a failure leaves the old one. Its values are
/// in single quotes, so a cmdline holding a quote, `$`, a backtick, a backslash or a newline is refused.
pub fn bootimg_script(layer: &Layer, b: &Boot) -> Result<String, String> {
    if let Some(c) = b.cmdline.chars().find(|c| "'$`\\\n".contains(*c)) {
        return Err(format!(
            "[boot] cmdline holds {c:?}: the boot image script is read by shell, which would run or mangle it"
        ));
    }
    let dtb = layer.kernel.as_ref().and_then(|k| k.dtbs.first()).ok_or(
        "the boot image script appends the first of [layer.kernel] dtbs, and none is declared",
    )?;
    let word = |what: &str, s: &str| {
        if !s.is_empty()
            && !s.starts_with('-')
            && !s.contains("..")
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c))
        {
            Ok(s.to_string())
        } else {
            Err(format!(
                "{what} {s:?} is not one word of letters, digits and . _ - /"
            ))
        }
    };
    let dtb = word("[layer.kernel] dtb", dtb)?;
    let mut opts = format!(
        "--base 0x{:08x} --kernel_offset 0x{:08x} --ramdisk_offset 0x{:08x} --second_offset 0x{:08x} --tags_offset 0x{:08x} --pagesize {} --header_version {}",
        b.base, b.kernel_offset, b.ramdisk_offset, b.second_offset, b.tags_offset, b.pagesize, b.header_version
    );
    for (flag, what, v) in [
        ("--os_version", "[boot] os_version", &b.os_version),
        (
            "--os_patch_level",
            "[boot] os_patch_level",
            &b.os_patch_level,
        ),
    ] {
        if let Some(v) = v {
            opts += &format!(" {flag} {}", word(what, v)?);
        }
    }
    let name = &layer.name;
    Ok(format!(
        r#"#!/bin/sh
# {name}: writes the phone's boot image when a kernel is installed (generated by lpoco layer pack)
set -eu
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
root=${{LPOCO_DTB_ROOT:-}}
dtb={dtb}

build() {{
    kver=$1 kernel=$2 initrd=$3 out=$4
    found= tried=
    for d in usr/lib/modules/$kver/dtb usr/lib/linux-image-$kver boot/dtbs/$kver; do
        if [ -f "$root/$d/$dtb" ]; then found=$root/$d/$dtb; break; fi
        tried="$tried $d/$dtb"
    done
    if [ -z "$found" ]; then
        echo "{name}: no device tree for $kver; looked for:$tried" >&2
        return 1
    fi
    cat "$kernel" "$found" > "$tmp/kernel-dtb"
    if ! mkbootimg --kernel "$tmp/kernel-dtb" --ramdisk "$initrd" {opts} --cmdline '{cmdline}' --output "$out.new"; then
        rm -f "$out.new"
        return 1
    fi
    mv "$out.new" "$out"
}}

if [ "${{1:-}}" = --alpm ]; then
    # libalpm hands each installed kernel's vmlinuz as a path on stdin
    while read -r path; do
        kver=$(basename "$(dirname "$path")")
        base=$(cat "/usr/lib/modules/$kver/pkgbase" 2>/dev/null) || {{ echo "{name}: $kver has no pkgbase" >&2; continue; }}
        build "$kver" "/$path" "/boot/initramfs-$base.img" "/boot/{name}-$base.img" || echo "{name}: no boot image for $kver" >&2
    done
    exit 0
fi

kver=$1 kernel= initrd= out=
shift
while [ $# -gt 1 ]; do
    case $1 in
        --kernel) kernel=$2 ;;
        --initrd) initrd=$2 ;;
        --out) out=$2 ;;
        *) echo "{name}: unknown argument $1" >&2; exit 2 ;;
    esac
    shift 2
done
[ -n "$kernel" ] && [ -n "$initrd" ] && [ -n "$out" ] || {{ echo "usage: {name}-bootimg KVER --kernel IMAGE --initrd RAMDISK --out BOOT.IMG" >&2; exit 2; }}
build "$kver" "$kernel" "$initrd" "$out"
"#,
        cmdline = b.cmdline
    ))
}

/// Adds the boot image trigger of a base to `NAME-boot` (made when absent): the script in every format but apk, and
/// a kernel-install plugin (deb, rpm) or a libalpm hook (archlinux) that calls it. Alpine's boot-deploy reads the
/// deviceinfo instead, so apk gets none; a layer that does not boot by `android-bootimg` gets none anywhere.
pub fn add_boot_trigger(
    pkgs: &mut Vec<Package>,
    layer: &Layer,
    b: &Boot,
    format: Format,
) -> Result<(), String> {
    if format == Format::Apk
        || layer.boot.as_ref().map(|l| l.method) != Some(BootMethod::AndroidBootimg)
    {
        return Ok(());
    }
    let name = &layer.name;
    let script = format!("/usr/libexec/{name}-bootimg");
    let text = |dst: String, body: String, mode: u32| Item {
        source: Source::Text(body),
        dst,
        mode,
        config: false,
    };
    let mut items = vec![text(script.clone(), bootimg_script(layer, b)?, 0o755)];
    // Arch's mkbootimg is a python script that android-tools does not pull in (found in the archlinux container)
    let deps: &[&str] = match format {
        Format::Deb => &["mkbootimg"],
        Format::Archlinux => &["android-tools", "python"],
        _ => &["android-tools"],
    };
    if format == Format::Archlinux {
        items.push(text(
            format!("/usr/share/libalpm/hooks/95-{name}-bootimg.hook"),
            format!(
                "[Trigger]\nType = Path\nOperation = Install\nOperation = Upgrade\nTarget = usr/lib/modules/*/vmlinuz\n\n[Action]\nDescription = {name}: writing the boot image\nWhen = PostTransaction\nNeedsTargets\nExec = {script} --alpm\n"
            ),
            0o644,
        ));
    } else {
        // kernel-install(8): $1 add|remove, $2 kernel version, $3 entry directory, $4 kernel image, $5 the initrds
        items.push(text(
            format!("/usr/lib/kernel/install.d/95-{name}-bootimg.install"),
            format!(
                "#!/bin/sh\n# {name}: the phone's boot image beside the entry's kernel (generated by lpoco layer pack)\nset -eu\n[ \"$1\" = add ] || exit 0\n# this runs after 90-loaderentry, which has copied the initrd into the entry directory and keeps only the files it knows: the image is written there\nentry=${{3:-}}\n[ -n \"$entry\" ] || {{ echo \"{name}: no entry directory (a layout other than bls)\" >&2; exit 1; }}\ninitrd=$entry/initrd\n[ -f \"$initrd\" ] || initrd=${{KERNEL_INSTALL_STAGING_AREA:-.}}/initrd\n[ -f \"$initrd\" ] || initrd=${{5:-}}\n[ -f \"$initrd\" ] || {{ echo \"{name}: no initrd for $2\" >&2; exit 1; }}\nexec {script} \"$2\" --kernel \"$4\" --initrd \"$initrd\" --out \"$entry/boot.img\"\n"
            ),
            0o755,
        ));
    }
    let want = format!("{name}-boot");
    match pkgs.iter_mut().find(|p| p.name == want) {
        Some(p) => {
            p.items.extend(items);
            p.depends.extend(deps.iter().map(|d| d.to_string()));
        }
        None => pkgs.push(Package {
            arch: None,
            name: want,
            description: format!("{name}: boot image trigger"),
            depends: deps.iter().map(|d| d.to_string()).collect(),
            items,
            post_install: None,
        }),
    }
    Ok(())
}
