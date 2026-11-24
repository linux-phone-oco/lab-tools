// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-host: the lab host prepared from declarations (run as `lpoco host ...`). `plan` (the default) changes
//! nothing; `prepare now` changes only what differs; `check` reads. See cli.rs for the invocation surface.

mod cli;

use argenv_resolve::{InputResolveExt, OnProblems, Resolution};
use cli::{Command, Contract};
use lpoco_host::*;
use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

// Linux open flags: a tty without carrier must not block the check, nor become its controlling terminal
const O_NONBLOCK: i32 = 0o4000;
const O_NOCTTY: i32 = 0o400;
use std::path::{Path, PathBuf};
use std::process::Command as Proc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let r = Contract::parse_and_lint(OnProblems::FailOnError);
    if Contract::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage("lpoco-host", "The lab host prepared from declarations (run as lpoco host). Commands: plan (the default; changes nothing on the host, only fetches each declaration's clone), prepare now (changes only what differs), follow [now] (a newer family release installed when no keeper holds a phone, checked, its pin landed), clones [now] (the shared clones under lab.toml [clones] fast-forwarded when clean and on main, any other named), check (reads, and exercises the share as the lab user), revoke now (removes the standing grant once the check passes)."));
        return;
    }
    if Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-host {VERSION}");
        return;
    }
    let code = match run(&r) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lpoco-host: {e}");
            2
        }
    };
    std::process::exit(code);
}

/// Everything the verbs need, read once.
struct State {
    home: String,
    user: Option<String>,
    path: String,
    root: Vec<String>,
    devices: Vec<Device>,
    /// every manifest found, before one-per-codename: the watchers go by lab repository folder
    found: Vec<Device>,
    missed: Vec<String>,
    reads: Vec<String>,
    lab_path: String,
    /// where follow reads the releases list
    releases_url: String,
    /// the token follow asks GitHub's API with (GITHUB_TOKEN); None: unauthenticated
    token: Option<String>,
    /// the time of day follow judges its window by, HH:MM; None: the system clock
    clock: Option<String>,
    lab: Lab,
    fs_root: String,
    /// where release assets are fetched from; None when this host fits none (then cargo builds)
    release_base: Option<String>,
    /// where other projects' releases are (a tool pinned by repository, tag and asset)
    github: String,
}

/// Where lab.toml is when none is given.
fn default_lab(home: &str) -> String {
    expand_home("~/Projects/lab-machine/lab.toml", home)
}

/// A path as the current folder reaches it: the units run elsewhere, so they are given it whole.
fn absolute(p: &str) -> String {
    if p.starts_with('/') {
        return p.to_string();
    }
    match std::env::current_dir() {
        Ok(d) => d.join(p).display().to_string(),
        Err(_) => p.to_string(),
    }
}

fn load(r: &Resolution) -> Result<State, String> {
    let home = Contract::HOME.get_from(r).ok_or("HOME is not set")?;
    let lab_path = absolute(
        &Contract::LAB
            .get_from(r)
            .unwrap_or_else(|| default_lab(&home)),
    );
    let (lab_text, lab_how) = declared_text(Path::new(&lab_path))?;
    let lab = parse_lab(Path::new(&lab_path), &lab_text)?;
    let root = Contract::ROOT
        .get_from(r)
        .or_else(|| lab.root.clone())
        .unwrap_or_else(|| DEFAULT_ROOT.to_string());
    let patterns = lab
        .devices
        .as_ref()
        .map(Devices::patterns)
        .unwrap_or_else(|| vec![DEFAULT_DEVICES.to_string()]);
    let mut devices = Vec::new();
    let mut reads: Vec<String> = lab_how.into_iter().collect();
    for p in find_devices(&patterns, &home)? {
        let (text, how) = declared_text(&p)?;
        reads.extend(how);
        devices.push(parse_device(&p, &text)?);
    }
    let found = devices.clone();
    let (devices, mut notes) = one_per_codename(devices);
    let mut devices = devices;
    let misses = pin_misses(&lab, &devices);
    let mut missed: Vec<String> = misses.iter().map(|(_, m)| m.clone()).collect();
    missed.append(&mut notes);
    let skip: Vec<PathBuf> = misses.iter().map(|(d, _)| d.path.clone()).collect();
    devices.retain(|d| !skip.contains(&d.path));
    Ok(State {
        lab_path: lab_path.clone(),
        clock: Contract::CLOCK.get_from(r),
        token: token_of(Contract::TOKEN.get_from(r))?,
        releases_url: Contract::RELEASES.get_from(r).unwrap_or_else(|| {
            "https://api.github.com/repos/linux-phone-oco/lab-tools/releases?per_page=100"
                .to_string()
        }),
        path: Contract::PATH.get_from(r).unwrap_or_default(),
        fs_root: Contract::FS_ROOT.get_from(r).unwrap_or_default(),
        release_base: asset_fits(std::env::consts::OS, std::env::consts::ARCH).then(|| {
            Contract::RELEASE_BASE
                .get_from(r)
                .unwrap_or_else(|| format!("{FAMILY_URL}/releases/download"))
        }),
        github: Contract::GITHUB
            .get_from(r)
            .unwrap_or_else(|| "https://github.com".to_string()),
        user: Contract::USER.get_from(r),
        home,
        root: root.split_whitespace().map(String::from).collect(),
        devices,
        found,
        missed,
        reads,
        lab,
    })
}

/// A declaration (`lab.toml`, `device.toml`) as its repository says it: a clone is fetched (never prompting, bounded) and the file is
/// read from `origin/main`, so a stale checkout cannot hide a `[host]` table. The second part names what was read: the
/// commit, how far the checkout is behind it, whether the working file differs; or why the file as it lies was read.
/// A plain file outside any clone has nothing to say.
fn declared_text(p: &Path) -> Result<(String, Option<String>), String> {
    let file = || fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()));
    let lie = |why: &str| -> Result<(String, Option<String>), String> {
        Ok((
            file()?,
            Some(format!("{}: the file as it lies ({why})", p.display())),
        ))
    };
    let Some(dir) = p.parent() else {
        return Ok((file()?, None));
    };
    let git = |args: &[&str]| -> Result<String, String> {
        let o = Proc::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "http.lowSpeedLimit=1000",
                "-c",
                "http.lowSpeedTime=15",
            ])
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env(
                "GIT_SSH_COMMAND",
                "ssh -o BatchMode=yes -o ConnectTimeout=10",
            )
            .output()
            .map_err(|e| e.to_string())?;
        if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
        }
    };
    let top = match git(&["rev-parse", "--show-toplevel"]) {
        Ok(t) if !t.trim().is_empty() => PathBuf::from(t.trim()),
        Ok(_) => return Ok((file()?, None)),
        Err(e) if e.contains("not a git repository") => return Ok((file()?, None)),
        Err(e) => return lie(&format!("git refused: {e}")),
    };
    let Some(rel) = p
        .canonicalize()
        .ok()
        .and_then(|c| c.strip_prefix(&top).ok().map(Path::to_path_buf))
    else {
        return lie("it lies outside its clone");
    };
    let fetch = git(&["fetch", "--quiet", "origin"]);
    let stale = match &fetch {
        Ok(_) => String::new(),
        Err(e) => format!("; fetch failed, origin/main as last fetched: {e}"),
    };
    if git(&["rev-parse", "--verify", "--quiet", "origin/main"]).is_err() {
        return lie(&format!("no origin/main{stale}"));
    }
    let Ok(text) = git(&["show", &format!("origin/main:{}", rel.display())]) else {
        return lie(&format!("not in origin/main{stale}"));
    };
    let sha = git(&["rev-parse", "--short", "origin/main"]).unwrap_or_default();
    let behind = match git(&["rev-list", "--count", "HEAD..origin/main"]) {
        Ok(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => "unknown".to_string(),
    };
    let differs = match fs::read_to_string(p) {
        Ok(w) if w != text => "; the working file differs and is ignored",
        _ => "",
    };
    Ok((
        text,
        Some(format!(
            "{} from origin/main {}, checkout {behind} behind{differs}{stale}",
            p.display(),
            sha.trim()
        )),
    ))
}

fn on_path(path: &str, name: &str) -> Option<PathBuf> {
    path.split(':')
        .filter(|d| !d.is_empty())
        .map(|d| Path::new(d).join(name))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// The version `NAME --version` reports: the last word of its first line.
fn version_found(path: &str, home: &str, name: &str) -> Option<String> {
    // the host's own ~/.local/bin counts even when PATH does not name it yet
    let local = format!("{home}/.local/bin");
    let prog = on_path(&format!("{local}:{path}"), name)?;
    let o = Proc::new(prog).arg("--version").output().ok()?;
    let line = String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()?
        .to_string();
    line.split_whitespace().last().map(str::to_string)
}

fn kernel_modules_present(fs_root: &str) -> bool {
    if !fs_root.is_empty() {
        return true;
    }
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|r| Path::new("/lib/modules").join(r.trim()).is_dir())
        .unwrap_or(true)
}

fn module_loaded(fs_root: &str, m: &str) -> bool {
    if !fs_root.is_empty() {
        return false;
    }
    Path::new("/sys/module").join(m.replace('-', "_")).exists()
}

/// The root commands that make `files` real, only where they differ: the fixed list the plan prints.
fn root_commands(
    files: &[FileChange],
    modules: &[String],
    stage: &str,
    fs_root: &str,
) -> Vec<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let mut cmds = Vec::new();
    let mut rules_changed = false;
    let mut unit_changed = false;
    let mut link_reload: Option<Vec<String>> = None;
    let mut serials = Vec::new();
    for f in files.iter().filter(|f| f.changed()) {
        let base = f.dst.rsplit('/').next().unwrap_or(&f.dst);
        cmds.push(s(&[
            "install",
            "-D",
            "-m",
            file_mode(&f.dst),
            &format!("{stage}/{base}"),
            &f.dst,
        ]));
        if f.dst == share::SHARE_UNIT_DST {
            unit_changed = true;
        }
        if f.dst.starts_with(NM_DIR) {
            link_reload = Some(LinkManager::NetworkManager.reload());
        } else if f.dst.starts_with(NETWORKD_DIR) {
            link_reload = Some(LinkManager::Networkd.reload());
        }
        if f.dst.starts_with(UDEV_DIR) {
            rules_changed = true;
            serials.extend(f.serials.clone());
        }
    }
    if rules_changed {
        cmds.push(s(&["udevadm", "control", "--reload"]));
        for sn in serials {
            cmds.push(s(&[
                "udevadm",
                "trigger",
                "--action=change",
                &format!("--attr-match=serial={sn}"),
            ]));
        }
        // the rules match the block and tty children of the USB device, which --attr-match on the device itself does not reach
        cmds.push(s(&[
            "udevadm",
            "trigger",
            "--action=change",
            "--subsystem-match=block",
            "--subsystem-match=tty",
        ]));
    }
    if unit_changed {
        cmds.push(s(&["systemctl", "daemon-reload"]));
    }
    // the name is set by udev at the next plug-in; the manager reads its new file now
    cmds.extend(link_reload);
    let modules_file_changed = files.iter().any(|f| f.dst == MODULES_FILE && f.changed());
    if kernel_modules_present(fs_root) {
        for m in modules
            .iter()
            .filter(|m| modules_file_changed || !module_loaded(fs_root, m))
        {
            cmds.push(s(&["modprobe", m]));
        }
    }
    cmds
}

fn stage_dir(home: &str) -> String {
    format!("{home}/.cache/lpoco-host/stage")
}

fn run(r: &Resolution) -> Result<i32, String> {
    let command = Contract::COMMAND.get_from(r).unwrap_or(Command::Plan);
    let now = Contract::NOW.get_from(r).as_deref() == Some("now");
    // a run that changes the host holds the lock before it reads anything the other run may be changing
    let _lock = if now && matches!(command, Command::Prepare | Command::Follow) {
        let home = Contract::HOME.get_from(r).ok_or("HOME is not set")?;
        match lock::take(&home) {
            Ok(l) => Some(l),
            Err(lock::Refused::Held(pid)) if matches!(command, Command::Follow) => {
                println!("waits: another lpoco-host run (pid {pid}) is changing the host; the next timer run follows");
                return Ok(0);
            }
            Err(refused) => {
                return Err(format!(
                    "{}: wait for it, then run again",
                    refused.said(&lock::lock_path(&home))
                ));
            }
        }
    } else {
        None
    };
    let st = load(r)?;
    match command {
        Command::Plan => {
            plan(&st);
            Ok(0)
        }
        Command::Prepare => {
            if Contract::NOW.get_from(r).as_deref() != Some("now") {
                return Err("prepare changes the host: say `lpoco host prepare now` (plain `lpoco host` shows the plan)".into());
            }
            prepare(&st)
        }
        Command::Check => check(&st),
        Command::Follow => follow(&st, Contract::NOW.get_from(r).as_deref() == Some("now")),
        Command::Clones => clones(&st, Contract::NOW.get_from(r).as_deref() == Some("now")),
        Command::Revoke => {
            if Contract::NOW.get_from(r).as_deref() != Some("now") {
                return Err("revoke removes the standing grant: say `lpoco host revoke now` (it refuses unless the check passes)".into());
            }
            revoke(&st)
        }
    }
}

/// Where the host keeps the hash of each root file it installed: a folder that hides a file from the lab user
/// (polkit rules, NetworkManager keyfiles) cannot be read back, so the record stands for it.
fn record_path(home: &str) -> String {
    format!("{home}/.local/state/lpoco-host/installed")
}

/// The record: one `HASH DST` a line.
fn records(home: &str) -> Vec<(String, String)> {
    fs::read_to_string(record_path(home))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(' '))
        .map(|(h, d)| (h.to_string(), d.to_string()))
        .collect()
}

fn record_installed(home: &str, files: &[FileChange]) -> Result<(), String> {
    let mut all = records(home);
    for f in files.iter().filter(|f| f.changed()) {
        all.retain(|(_, d)| *d != f.dst);
        all.push((content_hash(&f.content), f.dst.clone()));
    }
    let path = record_path(home);
    if let Some(d) = Path::new(&path).parent() {
        fs::create_dir_all(d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
    }
    let text: String = all.iter().map(|(h, d)| format!("{h} {d}\n")).collect();
    replace_file(&path, text.as_bytes(), 0o644).map_err(|e| format!("cannot write {path}: {e}"))
}

/// A file of the host as plain text, None when it cannot be read.
fn read_text(fs_root: &str, p: &str) -> Option<String> {
    fs::read_to_string(format!("{fs_root}{p}")).ok()
}

fn read_file(st: &State, p: &str) -> Seen {
    match fs::read_to_string(format!("{}{p}", st.fs_root)) {
        Ok(t) => Seen::Text(t),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Seen::Unreadable {
            recorded: records(&st.home)
                .into_iter()
                .find(|(_, d)| d == p)
                .map(|(h, _)| h),
        },
        Err(_) => Seen::Absent,
    }
}

fn tool_plans(st: &State) -> Vec<ToolPlan> {
    tools_for(
        &st.lab,
        &st.home,
        &|n| version_found(&st.path, &st.home, n),
        &|n| {
            fs::read_to_string(stamp_path(&st.home, n))
                .ok()
                .map(|s| s.trim().to_string())
        },
        &|n| on_path(&format!("{}/.local/bin:{}", st.home, st.path), n).is_some(),
        st.release_base.as_deref(),
        &st.github,
    )
}

fn watchers(st: &State, tools: &[ToolPlan]) -> (FileChange, Vec<Vec<String>>) {
    let replaced = tools.iter().any(|t| t.name == WATCH_TOOL && !t.ok());
    let wants = format!("{}/.config/systemd/user/default.target.wants", st.home);
    watch_plan(
        &st.home,
        &st.found,
        &|p| fs::read_to_string(p).ok(),
        &|i| Path::new(&format!("{wants}/devlab-watch@{i}.service")).exists(),
        &|i| {
            Proc::new("systemctl")
                .args(["--user", "is-active", "--quiet"])
                .arg(format!("devlab-watch@{i}.service"))
                .status()
                // is-active exits 3 for a unit that is not running; any other answer (or no systemctl) proposes nothing
                .map(|s| s.code() != Some(3))
                .unwrap_or(true)
        },
        replaced,
    )
}

/// The instances with a wants link: what is enabled, whoever enabled it.
fn enabled_watchers(st: &State) -> Vec<String> {
    let wants = format!("{}/.config/systemd/user/default.target.wants", st.home);
    let mut v: Vec<String> = fs::read_dir(wants)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_prefix("devlab-watch@")?
                .strip_suffix(".service")
                .map(str::to_string)
        })
        .collect();
    v.sort();
    v
}

fn plan(st: &State) {
    let files = files_for(
        &st.devices,
        st.lab.gadget_group.as_deref(),
        LinkManager::detect(&st.fs_root),
        st.user.as_deref(),
        &|p| read_file(st, p),
    );
    let modules = modules_of(&st.devices);
    let stage = stage_dir(&st.home);
    println!(
        "lpoco host plan (nothing on the host is changed; each declaration's clone is fetched)"
    );
    for r in &st.reads {
        println!("read: {r}");
    }
    for m in &st.missed {
        println!("not prepared: {m}");
    }
    println!(
        "\ndevices: {}",
        if st.devices.is_empty() {
            "none declare a [host] table".into()
        } else {
            st.devices
                .iter()
                .map(|d| d.codename.clone())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    for f in &files {
        println!("\n{} ({})", f.dst, f.state());
        for l in f.diff() {
            println!("    {l}");
        }
    }
    if modules.iter().any(|m| !module_loaded(&st.fs_root, m))
        && !kernel_modules_present(&st.fs_root)
    {
        println!("\nreboot: the running kernel has no modules (/lib/modules for it is gone), so {} cannot be loaded until the host boots its installed kernel", modules.join(", "));
    }
    let cmds = root_commands(&files, &modules, &stage, &st.fs_root);
    println!(
        "\nwhat root is for (RootNeed; no standing grant, prepare asks sudo for your password):"
    );
    for n in share::RootNeed::ALL {
        println!("    {:<32} {}", n.name(), n.when());
    }
    println!("\nroot commands, through `{}`:", st.root.join(" "));
    if cmds.is_empty() {
        println!("    none");
    }
    for c in &cmds {
        println!("    {}", c.join(" "));
    }
    if !net_links(&st.devices).is_empty() && LinkManager::detect(&st.fs_root).is_none() {
        println!("\nlink config: no NetworkManager or systemd-networkd runs on this host, so the address of {} is not written; start one and plan again", net_links(&st.devices).iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "));
    }
    if Path::new(&format!("{}{KEPT_RULE}", st.fs_root)).exists() {
        println!("\nkept: {KEPT_RULE} is a wide rule; it is kept for now and prepare never removes or edits it");
    }
    let _ = print_commands(st, false);
    let _ = print_targets(st, false);
    print_foreign_rules(st);
    let tools = tool_plans(st);
    let (unit, ucmds) = watchers(st, &tools);
    println!("\nwatchers (user units, no root; replaces lab-machine setup/devlab-watch.sh):");
    println!("    {} ({})", unit.dst, unit.state());
    let unasked = unasked_watchers(&st.home, &st.found, &enabled_watchers(st));
    if ucmds.is_empty() && unasked.is_empty() {
        println!("    nothing to do");
    }
    for i in &unasked {
        println!("    devlab-watch@{i}.service is enabled but no device that asks to be watched ([host] watch = true) maps to it; it is left as it is - prepare never stops it");
    }
    for c in &ucmds {
        println!("    {}", c.join(" "));
    }
    if let Some((files, cmds)) = follow_units(st) {
        println!("\nfollow (user timer, no root; `lpoco host follow now` installs a newer release when no keeper holds a phone):");
        for f in &files {
            println!("    {} ({})", f.dst, f.state());
            for l in f.diff() {
                println!("        {l}");
            }
        }
        for c in &cmds {
            println!("    {}", c.join(" "));
        }
    }
    if let Some((files, cmds)) = clones_units(st) {
        println!("\nclones (user timer, no root; `lpoco host clones now` fast-forwards each clean main checkout under the declared folders and names the others):");
        for f in &files {
            println!("    {} ({})", f.dst, f.state());
            for l in f.diff() {
                println!("        {l}");
            }
        }
        for c in &cmds {
            println!("    {}", c.join(" "));
        }
    }
    match tool_timer_units(st) {
        Ok(all) => {
            for t in &all {
                println!("\ntool timers (user units from the pinned tag, no root; prepare writes them; one that differs is hand-changed or from an older tag):");
                for (f, mode) in &t.files {
                    let state = match f.state() {
                        "changes" => "differs: hand-changed or from an older tag",
                        _ if wrong_mode(&f.dst, *mode) => "mode differs",
                        s => s,
                    };
                    println!("    {} ({state})", f.dst);
                    // the private file holds values: its lines are never printed
                    if *mode != 0o600 {
                        for l in f.diff() {
                            println!("        {l}");
                        }
                    }
                }
                for c in &t.cmds {
                    println!("    {}", c.join(" "));
                }
            }
        }
        Err(e) => println!("\ntool timers: not measured, {e}"),
    }
    println!("\ntools:");
    for t in tools {
        let found = t.found_text();
        if t.unpinned() {
            println!(
                "    {:<14} pin not known yet, found {found}: nothing installed",
                t.name
            );
            continue;
        }
        if t.ok() {
            println!(
                "    {:<14} pinned {:<8} found {found}: nothing to do",
                t.name, t.pinned
            );
        } else {
            println!("    {:<14} pinned {:<8} found {found}", t.name, t.pinned);
            for s in &t.steps {
                println!("        {}", s.show());
            }
            if let Some(by) = &t.installed_by {
                println!("        off the pin: an older {} is first on PATH; {by} installs this name too, so reinstall {by} into ~/.local and remove any copy that shadows it", t.name);
            }
        }
    }
}

fn run_root(st: &State, cmd: &[String]) -> Result<(), String> {
    let (prog, pre) = st.root.split_first().ok_or("the root prefix is empty")?;
    let status = Proc::new(prog)
        .args(pre)
        .args(cmd)
        .status()
        .map_err(|e| format!("cannot run {prog}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("root command failed ({status}): {}", cmd.join(" ")))
    }
}

fn prepare(st: &State) -> Result<i32, String> {
    for r in &st.reads {
        println!("read: {r}");
    }
    // fetched before anything changes: a tag without its unit stops the run with nothing written
    let tool_timers = tool_timer_units(st)?;
    let files = files_for(
        &st.devices,
        st.lab.gadget_group.as_deref(),
        LinkManager::detect(&st.fs_root),
        st.user.as_deref(),
        &|p| read_file(st, p),
    );
    let modules = modules_of(&st.devices);
    let stage = stage_dir(&st.home);
    // root installs from here: a fresh folder only this user can write
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(&stage).map_err(|e| format!("cannot make {stage}: {e}"))?;
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("{stage}: {e}"))?;
    for f in files.iter().filter(|f| f.changed()) {
        let base = f.dst.rsplit('/').next().unwrap_or(&f.dst);
        fs::write(format!("{stage}/{base}"), &f.content)
            .map_err(|e| format!("cannot stage {base}: {e}"))?;
    }
    let cmds = root_commands(&files, &modules, &stage, &st.fs_root);
    for c in &cmds {
        println!("root: {}", c.join(" "));
        run_root(st, c)?;
    }
    record_installed(&st.home, &files)?;
    let _ = fs::remove_dir_all(&stage);
    // worked out before the tools change: a replaced binary restarts the watchers that run it
    let tools = tool_plans(st);
    let (unit, mut ucmds) = watchers(st, &tools);
    install_tools(st, tools)?;
    if unit.changed() {
        if let Some(d) = Path::new(&unit.dst).parent() {
            fs::create_dir_all(d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
        }
        replace_file(&unit.dst, unit.content.as_bytes(), 0o644)
            .map_err(|e| format!("cannot write {}: {e}", unit.dst))?;
    }
    for (files, cmds) in [follow_units(st), clones_units(st)].into_iter().flatten() {
        for f in files.iter().filter(|f| f.changed()) {
            if let Some(d) = Path::new(&f.dst).parent() {
                fs::create_dir_all(d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
            }
            replace_file(&f.dst, f.content.as_bytes(), 0o644)
                .map_err(|e| format!("cannot write {}: {e}", f.dst))?;
        }
        ucmds.extend(cmds);
    }
    for t in &tool_timers {
        for (f, mode) in t
            .files
            .iter()
            .filter(|(f, m)| f.changed() || wrong_mode(&f.dst, *m))
        {
            if let Some(d) = Path::new(&f.dst).parent() {
                fs::create_dir_all(d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
            }
            replace_file(&f.dst, f.content.as_bytes(), *mode)
                .map_err(|e| format!("cannot write {}: {e}", f.dst))?;
        }
        ucmds.extend(t.cmds.clone());
    }
    for c in &ucmds {
        println!("user: {}", c.join(" "));
        let status = Proc::new(&c[0])
            .args(&c[1..])
            .status()
            .map_err(|e| format!("cannot run {}: {e}", c[0]))?;
        if !status.success() {
            return Err(format!("{} failed ({status})", c.join(" ")));
        }
    }
    println!("prepared: {} root command(s)", cmds.len());
    // as kubeadm runs its checks, the verb that installed the share also exercises it
    let (share_bad, _) = share_checks(st);
    Ok(if st.missed.is_empty() && share_bad == 0 {
        0
    } else {
        1
    })
}

/// Installs each tool that is off its pin, by the steps its plan names.
fn install_tools(st: &State, tools: Vec<ToolPlan>) -> Result<(), String> {
    for t in tools.into_iter().filter(|t| !t.ok()) {
        println!("tool {}: pinned {}", t.name, t.pinned);
        for s in &t.steps {
            match s {
                Step::Run(a) => {
                    if a.first().is_some_and(|x| x == "git")
                        && a.get(1).is_some_and(|x| x == "clone")
                    {
                        // a clone into a folder left by an earlier run: start it again
                        let _ = fs::remove_dir_all(a.last().unwrap());
                    }
                    let status = Proc::new(&a[0])
                        .args(&a[1..])
                        .status()
                        .map_err(|e| format!("cannot run {}: {e}", a[0]))?;
                    if !status.success() {
                        return Err(format!("{} failed ({status})", s.show()));
                    }
                }
                Step::Release {
                    url,
                    sha_url,
                    to,
                    also,
                    fallback,
                } => install_release(st, url, sha_url, to, also, fallback)?,
                Step::ReleaseArchive {
                    url,
                    sums_url,
                    member,
                    to,
                } => install_release_archive(st, url, sums_url, member, to)?,
                Step::Stamp { to, rev } => {
                    if let Some(d) = Path::new(to).parent() {
                        fs::create_dir_all(d)
                            .map_err(|e| format!("cannot make {}: {e}", d.display()))?;
                    }
                    replace_file(to, format!("{rev}\n").as_bytes(), 0o644)
                        .map_err(|e| format!("cannot write {to}: {e}"))?;
                }
                Step::Copy { from, to } => {
                    if let Some(d) = Path::new(to).parent() {
                        fs::create_dir_all(d)
                            .map_err(|e| format!("cannot make {}: {e}", d.display()))?;
                    }
                    let bytes = fs::read(from).map_err(|e| format!("cannot read {from}: {e}"))?;
                    replace_file(to, &bytes, 0o755)
                        .map_err(|e| format!("cannot copy {from} to {to}: {e}"))?;
                }
            }
        }
    }
    Ok(())
}

/// Device nodes of `class` (block or tty) whose USB ancestor reports `serial`.
fn nodes_with_serial(class: &str, serial: &str) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(rd) = fs::read_dir(Path::new("/sys/class").join(class)) else {
        return found;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if class == "tty" && !(name.starts_with("ttyACM") || name.starts_with("ttyUSB")) {
            continue;
        }
        let Ok(real) = fs::canonicalize(e.path()) else {
            continue;
        };
        if real.join("partition").exists() {
            continue;
        }
        let mut dir = real.parent();
        while let Some(d) = dir {
            if let Ok(s) = fs::read_to_string(d.join("serial")) {
                if s.trim() == serial {
                    found.push(format!("/dev/{name}"));
                }
                break;
            }
            dir = d.parent();
        }
    }
    found.sort();
    found
}

/// The share, exercised for each device that declares an address: (failures, devices measured ok).
fn share_checks(st: &State) -> (i32, i32) {
    let (mut bad, mut ok) = (0, 0);
    for d in &st.devices {
        match share::probe(&d.codename, d.usb_address.as_deref(), &st.fs_root) {
            share::Measured::Ok(m) => {
                ok += 1;
                println!("share {m}");
            }
            share::Measured::NotMeasured(m) => println!("share: not measured, {m}"),
            share::Measured::Failed(m) => {
                bad += 1;
                println!("share: FAIL, {m}");
            }
        }
    }
    // one device failing means the check failed: it records nothing
    if bad == 0 && ok > 0 {
        record_proved_share_files(st);
    }
    (bad, ok)
}

/// A share the lab user just started and stopped proves the files it stands on are in place. Their folders can be
/// root-only (polkit), so plan cannot read them: the staged hash of each one it could not read is recorded, as an
/// install would, and the next plan asks no root for it. A file with a record of another content stays a root install: the probe shows some rule works, not that it is this one. This is the one place `check` writes. Only called after a passing share check.
fn record_proved_share_files(st: &State) {
    let proved = [
        share::POLKIT_DST,
        share::SHARE_UNIT_DST,
        share::SHARE_HELPER_DST,
    ];
    let files: Vec<FileChange> = files_for(
        &st.devices,
        st.lab.gadget_group.as_deref(),
        LinkManager::detect(&st.fs_root),
        st.user.as_deref(),
        &|p| read_file(st, p),
    )
    .into_iter()
    .filter(|f| {
        proved.contains(&f.dst.as_str()) && matches!(f.seen, Seen::Unreadable { recorded: None })
    })
    .collect();
    if files.iter().any(|f| f.changed()) {
        match record_installed(&st.home, &files) {
            Ok(()) => println!("share: the files it proved are recorded as installed"),
            Err(e) => println!("share: could not record what it proved: {e}"),
        }
    }
}

/// What `revoke now` needs: the host checked and well, the share measured at least once (not merely "not
/// known yet"), and no root prefix that still goes through lab-agent. Then the standing grant goes, and the
/// host is asked whether it is gone.
fn revoke(st: &State) -> Result<i32, String> {
    const GRANT: &str = "/etc/sudoers.d/lab-agent";
    if st.root.iter().any(|w| w.contains("lab-agent")) {
        return Err(format!(
            "the root prefix still goes through lab-agent ({}): nothing is revoked",
            st.root.join(" ")
        ));
    }
    println!("revoke: checking the host first");
    if check(st)? != 0 {
        return Err("the check did not pass: nothing is revoked".into());
    }
    let (_, measured) = share_checks(st);
    if measured == 0 {
        return Err("the share was not measured on any device (is a phone attached?): nothing is revoked, the grant is the one way left to do it".into());
    }
    let (prog, pre) = st.root.split_first().ok_or("the root prefix is empty")?;
    println!("root: {} rm -f -- {GRANT}", st.root.join(" "));
    let status = Proc::new(prog)
        .args(pre)
        .args(["rm", "-f", "--", GRANT])
        .status()
        .map_err(|e| format!("cannot run {prog}: {e}"))?;
    if !status.success() {
        return Err(format!("removing {GRANT} failed ({status})"));
    }
    let listed = Proc::new("sudo")
        .args(["-n", "-l"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if listed.contains("NOPASSWD: /usr/bin/lab-agent") {
        return Err("the grant is still listed by sudo: another file may carry it".into());
    }
    println!("revoked: {GRANT} is gone; sudo no longer lists lab-agent as passwordless");
    Ok(0)
}

fn check(st: &State) -> Result<i32, String> {
    for r in &st.reads {
        println!("read: {r}");
    }
    let mut bad = 0;
    if let Some(g) = &st.lab.gadget_group {
        let user = st.user.as_deref().unwrap_or("");
        match group_has_member(
            &read_text(&st.fs_root, "/etc/group").unwrap_or_default(),
            &read_text(&st.fs_root, "/etc/passwd").unwrap_or_default(),
            g,
            user,
        ) {
            Some(true) => println!("gadget group {g}: ok, {user} is in it (per /etc/group)"),
            Some(false) => {
                bad += 1;
                if user.is_empty() {
                    println!(
                        "gadget group {g}: FAIL, USER is not set, so who is in it is not known"
                    );
                } else {
                    println!("gadget group {g}: FAIL, {user} is not in it");
                }
            }
            None => {
                bad += 1;
                println!("gadget group {g}: FAIL, no such group");
            }
        }
    }
    if Path::new(&format!("{}{KEPT_RULE}", st.fs_root)).exists() {
        println!("kept: {KEPT_RULE} is a wide rule, kept for now");
    }
    bad += print_commands(st, true) as i32;
    bad += print_targets(st, true) as i32;
    print_foreign_rules(st);
    for d in &st.devices {
        for g in &d.host.gadget {
            for a in &g.access {
                let nodes = nodes_with_serial(a.subsystem(), &g.serial);
                if nodes.is_empty() {
                    println!(
                        "{} {} {}: not measured, no such node present",
                        d.codename,
                        a.subsystem(),
                        g.serial
                    );
                }
                for n in nodes {
                    let mut o = fs::OpenOptions::new();
                    o.read(true)
                        .write(*a == Access::Tty)
                        .custom_flags(O_NONBLOCK | O_NOCTTY);
                    match o.open(&n) {
                        Ok(_) => println!(
                            "{} {}: ok, {n} opens as this user",
                            d.codename,
                            a.subsystem()
                        ),
                        Err(e) => {
                            bad += 1;
                            println!(
                                "{} {}: FAIL, {n} does not open as this user: {e}",
                                d.codename,
                                a.subsystem()
                            );
                        }
                    }
                }
            }
        }
        if let Some(s) = &d.fastboot_serial {
            match Proc::new("fastboot").arg("devices").output() {
                Ok(o) if String::from_utf8_lossy(&o.stdout).contains(s.as_str()) => {
                    println!("{} fastboot: ok, serial {s} listed", d.codename)
                }
                Ok(_) => println!(
                    "{} fastboot: not measured, serial {s} is not in fastboot now",
                    d.codename
                ),
                Err(_) => println!("{} fastboot: not measured, no fastboot on PATH", d.codename),
            }
        }
        print_stale_pieces(d);
    }
    let declared: Vec<String> = st
        .devices
        .iter()
        .flat_map(|d| d.host.gadget.iter().map(|g| g.serial.clone()))
        .collect();
    if declared.is_empty() {
        println!("kernel log: not measured, no gadget is declared");
    } else {
        match kernel_log(&st.fs_root) {
            None => println!("kernel log: not measured, it cannot be read (journalctl -k)"),
            Some(log) => {
                let bad_ones = undeclared_gadgets(&log, &declared);
                if bad_ones.is_empty() {
                    println!("kernel log: ok, every gadget enumerated since boot and not unplugged has a declared serial");
                }
                for g in bad_ones {
                    bad += 1;
                    println!(
                        "kernel log: FAIL, undeclared gadget serial: {} enumerated with serial {} at {}; declared: {}",
                        g.device,
                        g.serial,
                        g.when.as_deref().unwrap_or("time not known"),
                        declared.join(", ")
                    );
                }
            }
        }
    }
    for t in tool_plans(st) {
        if t.unpinned() {
            println!("tool {}: not measured, its pin is not known yet", t.name);
        } else if t.ok() {
            println!("tool {}: ok, {}", t.name, t.found.unwrap_or_default());
        } else {
            bad += 1;
            println!(
                "tool {}: FAIL, pinned {} found {}",
                t.name,
                t.pinned,
                t.found_text()
            );
        }
    }
    bad += share_checks(st).0;
    bad += print_follow_age(st) as i32;
    for m in &st.missed {
        bad += 1;
        println!("FAIL {m}");
    }
    Ok(if bad == 0 { 0 } else { 1 })
}

/// The kernel log since boot: `kernel.log` under the test root when there is one, else `journalctl -k -b`.
fn kernel_log(fs_root: &str) -> Option<String> {
    if !fs_root.is_empty() {
        return read_text(fs_root, "/kernel.log");
    }
    let o = Proc::new("journalctl")
        .args(["-k", "-b", "--no-pager"])
        .output()
        .ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Name the rule files that are not ours and could cross ours; read-only, whatever the verb. Only /etc/udev/rules.d is scanned (the admin's place; the vendors' own are not ours to judge).
fn print_foreign_rules(st: &State) {
    let dir = format!("{}{UDEV_DIR}", st.fs_root);
    let mut files: Vec<(String, String)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if let (Some(n), Ok(t)) = (e.file_name().to_str(), std::fs::read_to_string(e.path())) {
                files.push((n.to_string(), t));
            }
        }
    }
    let serials: Vec<String> = st
        .devices
        .iter()
        .flat_map(|d| d.host.gadget.iter().map(|g| g.serial.clone()))
        .collect();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().is_file() && std::fs::read_to_string(e.path()).is_err() {
                println!("foreign: {UDEV_DIR}/{} could not be read, so it is not known whether it crosses ours", e.file_name().to_string_lossy());
            }
        }
    }
    for (name, why) in foreign_rules(&files, &serials) {
        println!("foreign: {UDEV_DIR}/{name} is not ours and {why}; named only, prepare never edits or removes it");
    }
}

/// Each declared host command, once: found on PATH (where) or missing, with every device that wants it. Read-only; returns how many are missing (only `check` counts them as failures). A command in ~/.local/bin counts only when PATH names it.
fn print_commands(st: &State, judge: bool) -> usize {
    let mut missing = 0;
    for (c, wanted) in commands_of(&st.devices) {
        let by = wanted.join(", ");
        match on_path(&st.path, &c) {
            Some(p) => println!("command {c}: ok, {} (wanted by {by})", p.display()),
            None => {
                missing += 1;
                let word = if judge { "FAIL, missing" } else { "missing" };
                println!("command {c}: {word} from PATH (wanted by {by}); named only, the tool installs nothing for it");
            }
        }
    }
    missing
}

/// A libdir that holds a standard library: a `libstd-*` file (a bare or leftover directory is not one).
fn has_std(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|rd| {
        rd.flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("libstd-"))
    })
}

/// Does the host have a standard library for `target`: the PATH rustc's own, or the lab's separate toolchain in
/// ~/.local/share/devlab-cross (what health/build.sh falls back to)? Returns where it is.
fn target_found(st: &State, target: &str) -> Option<PathBuf> {
    if target == "." {
        return None;
    }
    if let Some(rustc) = on_path(&st.path, "rustc") {
        // from a neutral directory and with no auto-install: asking must never fetch a toolchain
        if let Ok(o) = Proc::new(rustc)
            .args(["--print", "target-libdir", "--target", target])
            .current_dir("/")
            .env("RUSTUP_AUTO_INSTALL", "0")
            .output()
        {
            let dir = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
            if o.status.success() && has_std(&dir) {
                return Some(dir);
            }
        }
    }
    let tc = format!("{}/.local/share/devlab-cross/rustup/toolchains", st.home);
    fs::read_dir(tc).ok()?.flatten().find_map(|e| {
        let d = e.path().join("lib/rustlib").join(target).join("lib");
        has_std(&d).then_some(d)
    })
}

/// Each declared cross target, once, with every device that wants it. Read-only; returns how many are missing.
fn print_targets(st: &State, judge: bool) -> usize {
    let mut missing = 0;
    for (t, wanted) in targets_of(&st.devices) {
        let by = wanted.join(", ");
        match target_found(st, &t) {
            Some(p) => println!("target {t}: ok, {} (wanted by {by})", p.display()),
            None => {
                missing += 1;
                let word = if judge { "FAIL, missing" } else { "missing" };
                println!("target {t}: {word} a standard library (wanted by {by}); named only, the tool installs nothing for it");
            }
        }
    }
    missing
}

/// Puts `bytes` at `to` by writing a new file beside it and renaming it over the old one, so a program running
/// or reading the old file keeps the old inode and finishes with its old text.
fn replace_file(to: &str, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    use std::io::Write;
    let path = Path::new(to);
    let dir = path.parent().unwrap_or(Path::new("."));
    let base = path
        .file_name()
        .map(|b| b.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{base}.new-{}", std::process::id()));
    let _ = fs::remove_file(&tmp);
    let written = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.set_permissions(fs::Permissions::from_mode(mode))?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        // the rename itself reaches the disk
        fs::File::open(dir)?.sync_all()
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// Names what the phone's last gathered report (its newest tests/results/*.jsonl at origin/main) holds off main: a
/// piece whose blob differs, a package off the pin, with the commit the installed copy came from. Never fails the
/// check and never touches the phone: no report is "not measured".
fn print_stale_pieces(d: &Device) {
    let Some(dir) = d.path.parent() else { return };
    let git = |args: &[&str]| {
        Proc::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
    };
    let Some(listing) = git(&[
        "ls-tree",
        "-r",
        "--name-only",
        "origin/main",
        "tests/results/",
    ]) else {
        println!(
            "{} phone pieces: not measured, no report at origin/main",
            d.codename
        );
        return;
    };
    let listing = String::from_utf8_lossy(&listing.stdout).to_string();
    // report names are YYYYMMDD-HHMMSS (devlab gather refuses any other), so the greatest name is the newest
    let Some(newest) = listing.lines().filter(|l| l.ends_with(".jsonl")).max() else {
        println!(
            "{} phone pieces: not measured, no report at origin/main",
            d.codename
        );
        return;
    };
    let Some(report) = git(&["show", &format!("origin/main:{newest}")]) else {
        return;
    };
    let report = String::from_utf8_lossy(&report.stdout).to_string();
    match stale_lines(d, &report, newest) {
        Ok(lines) => lines.iter().for_each(|l| println!("{l}")),
        Err(e) => println!("{} phone pieces: not measured, {e}", d.codename),
    }
}

/// Fetches `url` with curl into `to_file`: Ok(true) when it came, Ok(false) when the release has no such file
/// (HTTP 404 or 410, or a `file://` that is not there), Err for any other failure (an outage, a timeout, no curl).
fn fetch(url: &str, to_file: &Path) -> Result<bool, String> {
    fetch_with(url, to_file, None)
}

/// `fetch`, and a `token` sent as a bearer header to GitHub's API only (never to another host, never on the command
/// line: curl reads it as a config from its stdin).
fn fetch_with(url: &str, to_file: &Path, token: Option<&str>) -> Result<bool, String> {
    let token = token.filter(|_| url.starts_with("https://api.github.com/"));
    let mut curl = Proc::new("curl");
    curl.args([
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https,file",
        "--connect-timeout",
        "20",
        "--max-time",
        "300",
        "--write-out",
        "%{http_code}",
        url,
        "--output",
    ])
    .arg(to_file);
    let o = run_curl(&mut curl, url, token)?;
    let code = String::from_utf8_lossy(&o.stdout);
    if o.status.success() {
        Ok(true)
    } else if matches!(code.trim(), "404" | "410") || o.status.code() == Some(37) {
        Ok(false)
    } else {
        Err(format!(
            "cannot fetch {url} (HTTP {}, {}): not installed, not built",
            code.trim(),
            o.status
        ))
    }
}

/// The token as GitHub writes it (letters, digits, `_` and `-`), trimmed; blank is none; anything else is refused
/// (it goes into a curl config line), without repeating it.
fn token_of(raw: Option<String>) -> Result<Option<String>, String> {
    let Some(raw) = raw.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    if raw
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Ok(Some(raw))
    } else {
        Err("GITHUB_TOKEN holds a character a GitHub token does not (only letters, digits, _ and -)".into())
    }
}

/// Whether `url` is GitHub's API: the only host a token is ever sent to.
fn is_github_api(url: &str) -> bool {
    url.starts_with("https://api.github.com/")
}

/// The token `gh` is logged in with, for a GitHub API url when no GITHUB_TOKEN is given: the service and the user then ask
/// the API as the same account without a token file. None when `gh` is not there, not logged in, or prints what is not a token.
fn gh_token(url: &str) -> Option<String> {
    if !is_github_api(url) {
        return None;
    }
    let mut gh = Proc::new("gh")
        .args(["auth", "token", "--hostname", "github.com", "--no-refresh"])
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let Some(mut stdout) = gh.stdout.take() else {
        let _ = gh.kill();
        let _ = gh.wait();
        return None;
    };
    // the reader is not joined: a helper gh started may hold the pipe open after gh is gone
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        let _ = sent.send(text);
    });
    if !wait_or_kill(&mut gh, GH_TOKEN_SECONDS) {
        return None;
    }
    let out = received
        .recv_timeout(std::time::Duration::from_secs(1))
        .ok()?;
    token_of(Some(out)).ok().flatten()
}

/// How long `gh auth token` may take: it reads a local file, a keyring or a credential helper that can wait for a person.
const GH_TOKEN_SECONDS: u64 = 10;

/// Waits for the child up to `seconds`; true when it ended with success, false when it failed or was killed for taking longer.
fn wait_or_kill(child: &mut std::process::Child, seconds: u64) -> bool {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if std::time::Instant::now() < until => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// Runs curl; a token goes to it as a config on its stdin (`header = "Authorization: Bearer ..."`), never as an argument.
fn run_curl(
    curl: &mut Proc,
    url: &str,
    token: Option<&str>,
) -> Result<std::process::Output, String> {
    use std::io::Write;
    let cannot = |e: std::io::Error| format!("cannot run curl for {url}: {e}");
    let Some(token) = token else {
        return curl.output().map_err(cannot);
    };
    let mut child = curl
        .args(["--proto-redir", "=https", "--config", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(cannot)?;
    let mut stdin = child.stdin.take().ok_or("curl has no stdin")?;
    writeln!(stdin, "header = \"Authorization: Bearer {token}\"")
        .map_err(|e| format!("cannot give curl the token: {e}"))?;
    let wrote = writeln!(stdin, "header = \"Authorization: Bearer {token}\"");
    drop(stdin);
    let out = child.wait_with_output().map_err(cannot)?;
    wrote.map_err(|e| format!("cannot give curl the token: {e}"))?;
    Ok(out)
}

/// The hash a `.sha256` file names: its first word, as `sha256sum` writes it.
fn named_hash(text: &str) -> Option<String> {
    let word = text.split_whitespace().next()?.to_ascii_lowercase();
    (word.len() == 64 && word.bytes().all(|b| b.is_ascii_hexdigit())).then_some(word)
}

fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Installs a program from its release asset after the hash matches (`to`, and `also` under the old names). A
/// release without the asset or its hash runs `fallback` (cargo); a hash that does not match, or one that cannot be
/// read, installs nothing and fails, and never falls back: the next run asks again.
fn install_release(
    st: &State,
    url: &str,
    sha_url: &str,
    to: &str,
    also: &[String],
    fallback: &[String],
) -> Result<(), String> {
    let work = format!("{}/.cache/lpoco-host/asset-{}", st.home, std::process::id());
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| format!("cannot make {work}: {e}"))?;
    let (file, sum) = (
        Path::new(&work).join("asset"),
        Path::new(&work).join("sha256"),
    );
    let got = fetch(url, &file);
    let result = if got.as_ref().is_ok_and(|g| *g) {
        match fetch(sha_url, &sum) {
            Ok(true) => {}
            Ok(false) => {
                return finish(
                    &work,
                    Err(format!("{url} has no .sha256 beside it: not installed")),
                )
            }
            Err(e) => return finish(&work, Err(e)),
        }
        let bytes = fs::read(&file).map_err(|e| format!("cannot read {url}: {e}"));
        let named = fs::read_to_string(&sum)
            .ok()
            .and_then(|t| named_hash(&t))
            .ok_or_else(|| format!("{sha_url} names no sha256"));
        match (bytes, named) {
            (Ok(bytes), Ok(named)) => {
                let have = hex_sha256(&bytes);
                if have != named {
                    Err(format!(
                        "{to}: sha256 of {url} is {have}, the release says {named}: not installed"
                    ))
                } else {
                    also.iter()
                        .map(String::as_str)
                        .chain(std::iter::once(to))
                        .try_for_each(|dst| {
                            if let Some(d) = Path::new(dst).parent() {
                                fs::create_dir_all(d)
                                    .map_err(|e| format!("cannot make {}: {e}", d.display()))?;
                            }
                            replace_file(dst, &bytes, 0o755)
                                .map_err(|e| format!("cannot write {dst}: {e}"))
                        })
                }
            }
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    } else if let Err(e) = got {
        Err(e)
    } else if fallback.is_empty() {
        Err(format!("no release asset at {url}: not installed"))
    } else {
        println!("no release asset at {url}: building with cargo");
        let status = Proc::new(&fallback[0])
            .args(&fallback[1..])
            .status()
            .map_err(|e| format!("cannot run {}: {e}", fallback[0]));
        status.and_then(|s| {
            s.success()
                .then_some(())
                .ok_or_else(|| format!("{} failed ({s})", fallback.join(" ")))
        })
    };
    finish(&work, result)
}

/// The hash a release's checksums file (`HASH  NAME` lines) gives for exactly `name`: a neighbour that only starts
/// with the name (`NAME.sbom.json`) is not it.
fn hash_for(checksums: &str, name: &str) -> Option<String> {
    // the first line naming the archive decides: a broken hash there is not skipped for a later line
    let line = checksums.lines().find(|l| {
        l.split_once(char::is_whitespace)
            .is_some_and(|(_, f)| f.trim_start().trim_start_matches('*') == name)
    })?;
    named_hash(line.split_once(char::is_whitespace)?.0)
}

/// Installs `member` of a release's tar.gz at `to` after the archive's hash, from the release's checksums file,
/// matches; nothing is installed when the archive, the file, its line, the hash or the member is missing or wrong.
fn install_release_archive(
    st: &State,
    url: &str,
    sums_url: &str,
    member: &str,
    to: &str,
) -> Result<(), String> {
    let work = format!("{}/.cache/lpoco-host/asset-{}", st.home, std::process::id());
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| format!("cannot make {work}: {e}"))?;
    let (file, sums) = (
        Path::new(&work).join("archive.tar.gz"),
        Path::new(&work).join("checksums"),
    );
    let name = url.rsplit('/').next().unwrap_or(url);
    let result = (|| {
        if !fetch(url, &file)? {
            return Err(format!("no release asset at {url}: not installed"));
        }
        if !fetch(sums_url, &sums)? {
            return Err(format!(
                "{sums_url} is not in the release: {name} not installed"
            ));
        }
        let text = fs::read_to_string(&sums).map_err(|e| format!("cannot read {sums_url}: {e}"))?;
        let named = hash_for(&text, name)
            .ok_or_else(|| format!("{sums_url} has no sha256 line for {name}: not installed"))?;
        let bytes = fs::read(&file).map_err(|e| format!("cannot read {url}: {e}"))?;
        let have = hex_sha256(&bytes);
        if have != named {
            return Err(format!(
                "{to}: sha256 of {url} is {have}, the release says {named}: not installed"
            ));
        }
        let out = Proc::new("tar")
            .args(["-xzf"])
            .arg(&file)
            .arg("-C")
            .arg(&work)
            .arg("--")
            .arg(member)
            .output()
            .map_err(|e| format!("cannot run tar: {e}"))?;
        let got = Path::new(&work).join(member);
        if !out.status.success()
            || !fs::symlink_metadata(&got).is_ok_and(|m| m.file_type().is_file())
        {
            return Err(format!(
                "{name} holds no file {member}: not installed ({})",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let bytes = fs::read(&got).map_err(|e| format!("cannot read {member}: {e}"))?;
        if let Some(d) = Path::new(to).parent() {
            fs::create_dir_all(d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
        }
        replace_file(to, &bytes, 0o755).map_err(|e| format!("cannot write {to}: {e}"))
    })();
    finish(&work, result)
}

fn finish(work: &str, result: Result<(), String>) -> Result<(), String> {
    let _ = fs::remove_dir_all(work);
    result
}

/// `follow [now]`: a family release newer than lab.toml's pin is installed when no phone's keeper holds the lab,
/// judged by its own `check` and `plan`, and its pin landed; a failing check or a release that needs root puts the
/// previous programs back and lands nothing. A declared `[follow] window` holds the install until its
/// hours (a run that began inside it finishes after it closes). Without `now` it only says what it would do.
/// What is installed is judged too: an installed `lpoco` older than the pin (a prepare that did not run after the
/// pin moved) is reinstalled at the pin, checked the same way, and no pin is landed. Only `lpoco` is read of the
/// family; an older other family program under a current `lpoco` is not noticed.
/// When the family is current, a declared tool of another project (`[tools.rhizoid]`) whose installed copy is not at
/// its pin is installed the same way (`follow_tools`).
fn follow(st: &State, now: bool) -> Result<i32, String> {
    let began = epoch_now()?;
    let Some(ToolPin::Version(pinned)) = st.lab.tools.get("lpoco") else {
        return Err("lab.toml pins no lpoco release: nothing to follow from".into());
    };
    if now {
        step_back_a_killed_follow(&st.home, pinned)?;
    }
    let released = newest_release(st)?.filter(|n| follow::is_newer(n, pinned));
    // what is installed is judged against the release the pin allows, not against the pin alone
    let installed = version_found(&st.path, &st.home, "lpoco");
    let newest = released.clone().unwrap_or_else(|| pinned.clone());
    let drifted = installed
        .as_deref()
        .is_some_and(|v| follow::is_newer(&newest, v));
    if released.is_none() && !drifted {
        let moved = moved_tools(st);
        if !moved.is_empty() {
            return follow_tools(st, now, began, moved);
        }
        let unread = installed
            .as_deref()
            .is_none_or(|v| follow::version_key(v).is_none());
        println!(
            "up to date: lpoco {pinned} is the newest family release{}",
            if unread {
                " (the installed lpoco's version is not readable: not judged)"
            } else {
                ""
            }
        );
        if now {
            keep_success(&st.home);
        }
        return Ok(0);
    }
    if !now {
        match (&released, &installed) {
            (Some(_), _) => println!("lpoco {pinned} is pinned, {newest} is released: `lpoco host follow now` installs it when no keeper holds a phone, checks it and lands the pin"),
            (None, v) => println!("lpoco {} is installed, {pinned} is pinned: `lpoco host follow now` installs {pinned} when no keeper holds a phone and checks it; the pin stays", v.as_deref().unwrap_or("?")),
        }
        return Ok(0);
    }
    if outside_window(st)? {
        return Ok(0);
    }
    if waits_for_keeper(st, &format!("release {newest}")) {
        return Ok(0);
    }
    if tried_too_lately(st, &newest, &format!("lpoco {newest}"))? {
        return Ok(0);
    }
    // everything that can be known before a program is touched
    let staged = if released.is_some() {
        let staged = stage_lab(st, pinned, &newest)?;
        preflight_landing(st)?;
        staged
    } else {
        st.lab_path.clone()
    };
    // a tool whose pin moved fails the check until follow installs it: that is not the host being broken
    let moved: Vec<String> = moved_tools(st).into_iter().map(|t| t.name).collect();
    let before = Judged::before_tools(&host_program(st), &st.lab_path, &moved)?;
    if !before.check_ok {
        println!(
            "waits: the host's own check fails now ({}); it is fixed before a release is followed",
            before.check_tail
        );
        return Ok(1);
    }
    // the baseline took time: a keeper may have come. From here every phone is kept by the follow until it ends
    // (a keeper that asks meanwhile is told the phone is kept), so no keeper meets programs of two versions.
    let _leases = match follow::take_leases(&st.home, &st.found, &lease_age) {
        Ok(l) => l,
        Err(follow::NotTaken::Kept { phone, who }) => {
            println!("waits: {who} keeps {phone}; release {newest} is installed in a window with no keeper");
            return Ok(0);
        }
        Err(follow::NotTaken::Unusable { phone, why }) => {
            return Err(format!(
                "the lease of {phone} could not be taken ({why}): nothing is changed"
            ));
        }
    };
    let previous = save_programs(&st.home)?;
    // a drift install lands no pin, so its journal never names the pinned release (that means "the pin landed")
    let journal_target = if released.is_some() {
        newest.clone()
    } else {
        format!("{newest}-reinstall")
    };
    write_journal(&st.home, &previous, &journal_target)?;
    // what is replaced: the programs of the pinned release, or, on a reinstall, the older ones installed
    let was = if released.is_some() {
        pinned.clone()
    } else {
        installed.clone().unwrap_or_else(|| "unreadable".into())
    };
    println!("following: lpoco {was} -> {newest}");
    let stepped_back = |why: String| {
        restore_programs(&previous).map_err(|e| format!("{why}; and the programs could not be put back ({e}): the journal is kept for the next run"))?;
        clear_journal(&st.home);
        remember_failed_try(&st.home, &newest, began);
        Err(format!(
            "{why}; the programs of {was} are back and the pin stays"
        ))
    };
    if let Err(why) = install_family(st, &newest) {
        return stepped_back(why);
    }
    let after = match Judged::before_tools(
        &format!("{}/.local/bin/lpoco-host", st.home),
        &staged,
        &moved,
    ) {
        Ok(a) => a,
        Err(why) => return stepped_back(why),
    };
    if !after.check_ok {
        return stepped_back(format!(
            "lpoco-host {newest} check failed: {}",
            after.check_tail
        ));
    }
    let Some(root) = after.root.clone() else {
        return stepped_back(format!(
            "lpoco-host {newest} plan did not answer (no root-commands section, or it failed): not applied"
        ));
    };
    let new_root: Vec<&String> = root
        .iter()
        .filter(|c| !before.root.as_ref().is_some_and(|b| b.contains(c)))
        .collect();
    if !new_root.is_empty() {
        return stepped_back(format!(
            "release {newest} needs root ({}): not applied by the timer, say so to orchestration",
            new_root
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    if released.is_some() {
        if let Err(why) = land_pin(st, pinned, &newest) {
            return stepped_back(format!("the pin was not landed ({why})"));
        }
    }
    clear_journal(&st.home);
    let _ = fs::remove_file(tried_file(&st.home));
    keep_success(&st.home);
    restart_watchers(st);
    if released.is_some() {
        println!("followed: lpoco {newest} is installed, checked, and its pin landed");
    } else {
        println!("followed: lpoco {newest} (the pin) is installed and checked; the pin stays");
    }
    Ok(0)
}

/// The declared tools of other projects whose installed copy is not the one lab.toml pins: installed already (a tool
/// that was never installed is prepare's to bring in) and not a program of the family (follow's other path).
fn moved_tools(st: &State) -> Vec<ToolPlan> {
    tool_plans(st)
        .into_iter()
        .filter(|t| {
            !t.ok()
                && !t.steps.is_empty()
                && t.installed_by.is_none()
                && !FAMILY.contains(&t.name.as_str())
                && !OLD_NAMES.iter().any(|(old, _)| *old == t.name)
                && (t.found.is_some() || t.on_path)
        })
        .collect()
}

/// What each moved tool's binary and stamp held, to step back to (None: not there).
type SavedTools = Vec<(String, Option<Vec<u8>>)>;

fn save_tools(st: &State, tools: &[ToolPlan]) -> Result<SavedTools, String> {
    tools
        .iter()
        .flat_map(|t| {
            [
                format!("{}/.local/bin/{}", st.home, t.name),
                stamp_path(&st.home, &t.name),
            ]
        })
        .map(|p| match fs::read(&p) {
            Ok(b) => Ok((p, Some(b))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((p, None)),
            Err(e) => Err(format!("cannot keep {p} to step back to: {e}")),
        })
        .collect()
}

/// Puts each tool and its stamp back; every one is tried, and what could not be put back is named.
fn restore_tools(previous: &SavedTools) -> Result<(), String> {
    let failed: Vec<String> = previous
        .iter()
        .filter_map(|(to, bytes)| match bytes {
            Some(b) => {
                let mode = if to.contains("/.local/share/") {
                    0o644
                } else {
                    0o755
                };
                replace_file(to, b, mode)
                    .err()
                    .map(|e| format!("{to}: {e}"))
            }
            None => match fs::remove_file(to) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Some(format!("{to}: {e}")),
                _ => None,
            },
        })
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; "))
    }
}

/// The moved tools in words: `rhizoid v0.3.6 -> v0.3.8`.
fn moved_words(tools: &[ToolPlan]) -> String {
    tools
        .iter()
        .map(|t| {
            format!(
                "{} {} -> {}",
                t.name,
                t.found.as_deref().unwrap_or("?"),
                t.pinned
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `follow` for tools of other projects: each declared tool whose pin moved is installed from its release at the pin,
/// at once (decision 2026-11-23: a tool outside the family never drives a phone, so no keeper's lease holds it back and
/// none is taken; only the family keeps the lease window), judged by the host's own `check` and
/// stepped back (binary and stamp) when the install or the check fails. No pin is landed: the pin is what moved.
/// Kept in memory only: a run killed after the install and before its check leaves the new tool, unchecked.
fn follow_tools(st: &State, now: bool, began: u64, moved: Vec<ToolPlan>) -> Result<i32, String> {
    let words = moved_words(&moved);
    if !now {
        println!("{words}: `lpoco host follow now` installs it at once (inside the follow window) and checks it");
        return Ok(0);
    }
    if outside_window(st)? {
        return Ok(0);
    }
    let key = format!("tools:{words}").replace(' ', "_");
    if tried_too_lately(st, &key, &words)? {
        return Ok(0);
    }
    let names: Vec<String> = moved.iter().map(|t| t.name.clone()).collect();
    let before = Judged::before_tools(&host_program(st), &st.lab_path, &names)?;
    if !before.check_ok {
        println!(
            "waits: the host's own check fails now ({}); it is fixed before a tool is followed",
            before.check_tail
        );
        return Ok(1);
    }
    let previous = save_tools(st, &moved)?;
    println!("following: {words}");
    let stepped_back = |why: String| {
        restore_tools(&previous)
            .map_err(|e| format!("{why}; and the tools could not be put back ({e})"))?;
        remember_failed_try(&st.home, &key, began);
        Err(format!("{why}; {words} is back as it was"))
    };
    if let Err(why) = install_tools(st, moved) {
        return stepped_back(why);
    }
    match Judged::of(&host_program(st), &st.lab_path) {
        Ok(a) if a.check_ok => {}
        Ok(a) => {
            return stepped_back(format!(
                "the host's check fails with the new tool: {}",
                a.check_tail
            ))
        }
        Err(why) => return stepped_back(why),
    }
    let _ = fs::remove_file(tried_file(&st.home));
    keep_success(&st.home);
    println!("followed: {words} is installed and checked");
    Ok(0)
}

/// What a host program says about the lab: whether its `check` passes, the tail of what it printed, and the root
/// commands its `plan` lists (None when the plan failed or has no such section).
struct Judged {
    check_ok: bool,
    check_tail: String,
    root: Option<Vec<String>>,
}

impl Judged {
    fn of(host: &str, lab: &str) -> Result<Judged, String> {
        Self::run_judged(host, lab, &[])
    }

    /// The baseline before a tool is followed: the moved tools are off their pins by definition, so their own FAIL
    /// lines are not held against the host.
    fn before_tools(host: &str, lab: &str, tools: &[String]) -> Result<Judged, String> {
        Self::run_judged(host, lab, tools)
    }

    fn run_judged(host: &str, lab: &str, ignore: &[String]) -> Result<Judged, String> {
        let run = |verb: &str| {
            Proc::new(host)
                .args([verb, "--lab", lab])
                .output()
                .map_err(|e| format!("cannot run {host} {verb}: {e}"))
        };
        let check = run("check")?;
        let plan = run("plan")?;
        let (check_ok, check_tail) = follow::judge_check_without(
            check.status.code(),
            &String::from_utf8_lossy(&check.stdout),
            ignore,
        );
        Ok(Judged {
            check_ok,
            check_tail,
            root: plan
                .status
                .success()
                .then(|| follow::plan_root_commands(&String::from_utf8_lossy(&plan.stdout)))
                .flatten(),
        })
    }
}

/// The installed host, else this very program.
fn host_program(st: &State) -> String {
    let installed = format!("{}/.local/bin/lpoco-host", st.home);
    if Path::new(&installed).exists() {
        installed
    } else {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(installed)
    }
}

/// Where the second of follow's last success is kept (a run that installed and checked, or found nothing to do).
fn ok_file(home: &str) -> String {
    format!("{home}/.cache/lpoco-host/follow/ok")
}

fn keep_success(home: &str) {
    let file = ok_file(home);
    let at = match epoch_now() {
        Ok(at) => at,
        Err(e) => {
            return eprintln!("cannot keep follow's success: {e}; `check` will not know it ran")
        }
    };
    let _ = fs::create_dir_all(Path::new(&file).parent().unwrap_or(Path::new(".")));
    if let Err(e) = fs::write(&file, format!("{at}\n")) {
        eprintln!("cannot keep follow's success ({file}): {e}; `check` will not know it ran");
    }
}

/// `check`'s line on follow's last success, only for a lab that declares `[follow]`: (fails, whether a line was said).
fn print_follow_age(st: &State) -> bool {
    let Some(f) = &st.lab.follow else {
        return false;
    };
    let at = fs::read_to_string(ok_file(&st.home))
        .ok()
        .and_then(|t| t.trim().parse::<u64>().ok());
    let Some((fails, line)) = follow::judge_last_success(&f.every, at, epoch_now().unwrap_or(0))
    else {
        return false;
    };
    println!("{line}");
    fails
}

/// Where a failed try is kept: `RELEASE START-SECOND`.
fn tried_file(home: &str) -> String {
    format!("{home}/.cache/lpoco-host/follow/tried")
}

fn epoch_now() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| format!("cannot read the time: {e}"))
}

/// Says so and returns true when `release` failed a try that began less than `[follow] every` ago: a timer run is
/// never held (it comes `every` after the last began), a manual run or a restart is. Without `[follow]` there is
/// no `every`, so no limit.
fn tried_too_lately(st: &State, release: &str, what: &str) -> Result<bool, String> {
    let Some(every) = st.lab.follow.as_ref().map(|f| f.every.as_str()) else {
        return Ok(false);
    };
    let Some(every_s) = follow::every_seconds(every) else {
        return Ok(false);
    };
    let tried = fs::read_to_string(tried_file(&st.home))
        .ok()
        .and_then(|t| follow::parse_tried(&t));
    let Some(wait) = follow::retry_wait(tried, release, every_s, epoch_now()?) else {
        return Ok(false);
    };
    println!(
        "waits: {what} failed its last try less than {every} ago; it is tried again in {} min",
        wait.div_ceil(60)
    );
    Ok(true)
}

fn remember_failed_try(home: &str, release: &str, began: u64) {
    let file = tried_file(home);
    let _ = fs::create_dir_all(Path::new(&file).parent().unwrap_or(Path::new(".")));
    if let Err(e) = fs::write(&file, format!("{release} {began}\n")) {
        eprintln!("cannot keep the failed try of {release} ({file}): {e}; it is not rate limited");
    }
}

/// Says so and returns true when a phone's keeper holds the lab now.
fn waits_for_keeper(st: &State, what: &str) -> bool {
    let held = follow::held_leases(&st.home, &st.found, &lease_age);
    for (phone, who) in &held {
        println!("waits: {who} keeps {phone}; {what} is installed in a window with no keeper");
    }
    !held.is_empty()
}

/// lab.toml with the pin moved, written where the new host reads it from.
fn stage_lab(st: &State, pinned: &str, newest: &str) -> Result<String, String> {
    let text = declared_lab_text(st)?;
    let bumped = follow::bump_pin(&text, pinned, newest)
        .ok_or_else(|| format!("{} has no line `lpoco = \"{pinned}\"` to move", st.lab_path))?;
    let staged = format!("{}/.cache/lpoco-host/follow/lab.toml", st.home);
    fs::write(&staged, &bumped).map_err(|e| format!("cannot write {staged}: {e}"))?;
    Ok(staged)
}

/// The landing is possible before a program is touched: lab.toml lies in a clone that has an origin, and `land` is found.
fn preflight_landing(st: &State) -> Result<(), String> {
    let lab = Path::new(&st.lab_path);
    let dir = lab.parent().unwrap_or(Path::new("."));
    let origin = Proc::new("git")
        .arg("-C")
        .arg(dir)
        .args(["remote", "get-url", "origin"])
        .output();
    if !origin.is_ok_and(|o| o.status.success()) {
        return Err(format!(
            "{} is not in a clone with an origin: the pin could not be landed, nothing is changed",
            st.lab_path
        ));
    }
    if on_path(&st.path, "land").is_none() {
        return Err(
            "`land` is not on PATH: the pin could not be landed, nothing is changed".into(),
        );
    }
    Ok(())
}

/// Restarts each enabled watcher: it runs the old `devlab` until it is.
fn restart_watchers(st: &State) {
    for i in enabled_watchers(st) {
        let unit = format!("devlab-watch@{i}.service");
        println!("user: systemctl --user restart {unit}");
        let _ = Proc::new("systemctl")
            .args(["--user", "restart", &unit])
            .status();
    }
}

fn newest_release(st: &State) -> Result<Option<String>, String> {
    let work = format!("{}/.cache/lpoco-host/follow", st.home);
    fs::create_dir_all(&work).map_err(|e| format!("cannot make {work}: {e}"))?;
    let file = Path::new(&work).join("releases.json");
    let _ = fs::remove_file(&file);
    let from_gh = if st.token.is_none() {
        gh_token(&st.releases_url)
    } else {
        None
    };
    let token = st.token.clone().or(from_gh.clone());
    let got = match fetch_with(&st.releases_url, &file, token.as_deref()) {
        // a stale gh login must not lock follow out: ask again as anyone; a token the user gave is never dropped
        Err(e) if from_gh.is_some() && e.contains("(HTTP 401,") => {
            fetch_with(&st.releases_url, &file, None)?
        }
        other => other?,
    };
    if !got {
        return Err(format!("{} has no releases list", st.releases_url));
    }
    let text =
        fs::read_to_string(&file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    follow::newest_family_version(&text)
}

/// A keeper lease's holder and the time since its last renewal.
fn lease_age(holder_file: &str) -> Option<(String, std::time::Duration)> {
    let who = fs::read_to_string(holder_file).ok()?;
    let age = fs::metadata(holder_file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| std::time::SystemTime::now().duration_since(t).ok())
        .unwrap_or_default();
    Some((who.trim().to_string(), age))
}

/// The names the family's programs are installed under: each program and the second names of lpoco-lab and lpoco-tone.
fn program_names() -> Vec<&'static str> {
    FAMILY
        .iter()
        .copied()
        .chain(OLD_NAMES.iter().map(|(old, _)| *old))
        .collect()
}

/// Each program's path and what it held (None: it was not there).
type Saved = Vec<(String, Option<Vec<u8>>)>;

/// What each program in ~/.local/bin held before, kept to step back to; a program that was not there is None.
fn save_programs(home: &str) -> Result<Saved, String> {
    program_names()
        .into_iter()
        .map(|n| {
            let to = format!("{home}/.local/bin/{n}");
            match fs::read(&to) {
                Ok(b) => Ok((to, Some(b))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((to, None)),
                Err(e) => Err(format!("cannot keep {to} to step back to: {e}")),
            }
        })
        .collect()
}

/// Puts each program back; every one is tried, and what could not be put back is named.
fn restore_programs(previous: &Saved) -> Result<(), String> {
    let failed: Vec<String> = previous
        .iter()
        .filter_map(|(to, bytes)| match bytes {
            Some(b) => replace_file(to, b, 0o755)
                .err()
                .map(|e| format!("{to}: {e}")),
            None => match fs::remove_file(to) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Some(format!("{to}: {e}")),
                _ => None,
            },
        })
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; "))
    }
}

/// Where the programs to step back to are kept while an install runs: a folder with each program's bytes under its
/// name (`NAME.absent`, empty, for one that was not there) and `TARGET`, the release being installed. It stands from
/// before the first program is touched until the follow has put them back or has landed: a run that finds it is a
/// follow that was killed in between.
fn journal_dir(home: &str) -> String {
    format!("{home}/.cache/lpoco-host/follow/journal")
}

const JOURNAL_TARGET: &str = "TARGET";

fn sync_dir(dir: &str) -> std::io::Result<()> {
    fs::File::open(dir)?.sync_all()
}

/// Writes the journal whole or not at all, and to disk before the first program is touched: built beside, synced,
/// then moved into place.
fn write_journal(home: &str, previous: &Saved, target: &str) -> Result<(), String> {
    use std::io::Write;
    let dir = journal_dir(home);
    let building = format!("{dir}.new");
    let _ = fs::remove_dir_all(&building);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&building).map_err(|e| format!("cannot make {building}: {e}"))?;
    let mut entries: Vec<(String, Vec<u8>)> = previous
        .iter()
        .map(|(to, bytes)| {
            let name = Path::new(to)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            match bytes {
                Some(b) => (name, b.clone()),
                None => (format!("{name}.absent"), Vec::new()),
            }
        })
        .collect();
    entries.push((JOURNAL_TARGET.into(), target.as_bytes().to_vec()));
    for (file, body) in &entries {
        fs::File::create(format!("{building}/{file}"))
            .and_then(|mut f| f.write_all(body).and_then(|_| f.sync_all()))
            .map_err(|e| format!("cannot keep {file} in the journal: {e}"))?;
    }
    sync_dir(&building).map_err(|e| format!("cannot sync {building}: {e}"))?;
    fs::rename(&building, &dir).map_err(|e| format!("cannot place the journal {dir}: {e}"))?;
    let parent = format!("{home}/.cache/lpoco-host/follow");
    sync_dir(&parent).map_err(|e| format!("cannot sync {parent}: {e}"))
}

/// What a killed follow was going to step back to: None when no follow was cut short; else the release it was
/// installing and the programs it replaced, read from what the journal holds. A journal without its TARGET was not
/// written whole: an error, and nothing is put back from it.
fn read_journal(home: &str) -> Result<Option<(String, Saved)>, String> {
    let dir = journal_dir(home);
    if !Path::new(&dir).is_dir() {
        return Ok(None);
    }
    let torn = |why: String| {
        format!("the journal {dir} is not whole ({why}): nothing is put back from it; remove it with `rm -r {dir}` once the programs are known good")
    };
    let target = fs::read_to_string(format!("{dir}/{JOURNAL_TARGET}"))
        .map_err(|e| torn(format!("{JOURNAL_TARGET}: {e}")))?;
    let mut saved = Saved::new();
    let listing = fs::read_dir(&dir).map_err(|e| torn(e.to_string()))?;
    for entry in listing {
        let name = entry
            .map_err(|e| torn(e.to_string()))?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name == JOURNAL_TARGET {
            continue;
        }
        match name.strip_suffix(".absent") {
            Some(program) => saved.push((format!("{home}/.local/bin/{program}"), None)),
            None => {
                let bytes =
                    fs::read(format!("{dir}/{name}")).map_err(|e| torn(format!("{name}: {e}")))?;
                saved.push((format!("{home}/.local/bin/{name}"), Some(bytes)));
            }
        }
    }
    if saved.is_empty() {
        return Err(torn("it names no program".into()));
    }
    Ok(Some((target.trim().to_string(), saved)))
}

fn clear_journal(home: &str) {
    let _ = fs::remove_dir_all(journal_dir(home));
}

/// A follow that was killed after it began to install left programs of two versions: they are put back first, unless
/// its pin had already landed, when the programs are the new release's and the journal is only dropped.
fn step_back_a_killed_follow(home: &str, pinned: &str) -> Result<(), String> {
    let Some((target, previous)) = read_journal(home)? else {
        return Ok(());
    };
    if target == pinned {
        println!("a follow was killed after its pin landed: lpoco {target} stays installed");
    } else {
        println!("a follow was killed mid-install: the programs it was replacing are put back");
        restore_programs(&previous).map_err(|e| {
            format!("cannot put back the programs of a killed follow ({e}): the journal is kept")
        })?;
    }
    clear_journal(home);
    Ok(())
}

/// Installs the family's programs at release `newest`, each from its asset after the hash matches.
fn install_family(st: &State, newest: &str) -> Result<(), String> {
    let mut lab = st.lab.clone();
    lab.tools
        .insert("lpoco".into(), ToolPin::Version(newest.to_string()));
    let tools: Vec<ToolPlan> = tools_for(
        &lab,
        &st.home,
        &|n| version_found(&st.path, &st.home, n),
        &|_| None,
        &|_| false,
        st.release_base.as_deref(),
        &st.github,
    )
    .into_iter()
    .filter(|t| FAMILY.contains(&t.name.as_str()))
    .collect();
    install_tools(st, tools)
}

/// lab.toml as origin's main has it when it is in a clone, else as the file lies.
fn declared_lab_text(st: &State) -> Result<String, String> {
    declared_text(Path::new(&st.lab_path)).map(|(t, _)| t)
}

/// Lands the pin: a fresh clone of the lab's repository, one commit on a branch of its own signed lab-host-follow,
/// and `land` (which checks it, merges it and reads main back).
fn land_pin(st: &State, pinned: &str, newest: &str) -> Result<(), String> {
    let lab = Path::new(&st.lab_path);
    let dir = lab.parent().unwrap_or(Path::new("."));
    let git = |dir: &Path, args: &[&str]| -> Result<String, String> {
        let o = Proc::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                &format!("user.name={}", follow::SIGNER),
                "-c",
                &format!("user.email={}@localhost", follow::SIGNER),
            ])
            .args(args)
            .output()
            .map_err(|e| format!("cannot run git: {e}"))?;
        if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else {
            Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&o.stderr).trim()
            ))
        }
    };
    let top = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?);
    let rel = lab
        .canonicalize()
        .map_err(|e| format!("{}: {e}", lab.display()))?;
    let rel = rel
        .strip_prefix(top.canonicalize().map_err(|e| e.to_string())?)
        .map_err(|_| "lab.toml is outside its repository".to_string())?
        .to_path_buf();
    let origin = git(&top, &["remote", "get-url", "origin"])?;
    let work = PathBuf::from(format!("{}/.cache/lpoco-host/follow/pin", st.home));
    let _ = fs::remove_dir_all(&work);
    git(
        Path::new("."),
        &["clone", "--quiet", &origin, &work.to_string_lossy()],
    )?;
    let branch = format!("lab-host-follow-{newest}");
    git(&work, &["checkout", "-q", "-b", &branch])?;
    let file = work.join(&rel);
    let text =
        fs::read_to_string(&file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let bumped = follow::bump_pin(&text, pinned, newest).ok_or_else(|| {
        format!(
            "main's {} has no line `lpoco = \"{pinned}\"`: someone moved the pin",
            rel.display()
        )
    })?;
    fs::write(&file, bumped).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    git(
        &work,
        &[
            "commit",
            "-q",
            "-am",
            &format!("chore(lab): follow the family to lpoco {newest}"),
        ],
    )?;
    let status = Proc::new("land")
        .current_dir(&work)
        .status()
        .map_err(|e| format!("cannot run land: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("land failed ({status})"))
    }
}

/// The user units that run `follow`, when lab.toml declares `[follow]`: the timer, the service it starts, and the
/// user commands (no root) that make them known and start the timer.
fn follow_units(st: &State) -> Option<(Vec<FileChange>, Vec<Vec<String>>)> {
    let every = &st.lab.follow.as_ref()?.every;
    Some(timer_units(
        st,
        follow::FOLLOW_TIMER,
        follow::FOLLOW_SERVICE,
        follow::timer_text(every),
        follow::service_text(service_lab(st)),
    ))
}

/// The lab a user service is told to read: none when it is the default one.
fn service_lab(st: &State) -> Option<&str> {
    (st.lab_path != default_lab(&st.home)).then_some(st.lab_path.as_str())
}

/// The user units that run `clones`, when lab.toml declares `[clones]`.
fn clones_units(st: &State) -> Option<(Vec<FileChange>, Vec<Vec<String>>)> {
    let every = &st.lab.clones.as_ref()?.every;
    Some(timer_units(
        st,
        clones::TIMER,
        clones::SERVICE,
        clones::timer_text(every),
        clones::service_text(service_lab(st)),
    ))
}

/// A user timer and the service it starts, as files to write and the user commands (no root) that make them known.
fn timer_units(
    st: &State,
    timer: &str,
    service: &str,
    timer_text: String,
    service_text: String,
) -> (Vec<FileChange>, Vec<Vec<String>>) {
    let dir = format!("{}/.config/systemd/user", st.home);
    let unit = |name: &str, content: String| {
        let dst = format!("{dir}/{name}");
        FileChange {
            seen: fs::read_to_string(&dst).ok().into(),
            content,
            serials: vec![],
            dst,
        }
    };
    let files = vec![unit(timer, timer_text), unit(service, service_text)];
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let mut cmds = Vec::new();
    if files.iter().any(FileChange::changed) {
        cmds.push(s(&["systemctl", "--user", "daemon-reload"]));
    }
    let wanted = format!("{dir}/timers.target.wants/{timer}");
    if files.iter().any(FileChange::changed) || !Path::new(&wanted).exists() {
        cmds.push(s(&["systemctl", "--user", "enable", "--now", timer]));
    }
    (files, cmds)
}

/// `clones [now]`: each shared clone under `[clones] folders` that is clean and on main is fast-forwarded to
/// origin/main; any other is left as it is and named with why. Without `now` it only says what it would do.
fn clones(st: &State, now: bool) -> Result<i32, String> {
    let Some(c) = &st.lab.clones else {
        println!("no [clones] declared in lab.toml: nothing to keep fresh");
        return Ok(0);
    };
    let (mut unreadable, mut fetched, mut fetch_failed) = (false, 0, 0);
    for folder in &c.folders {
        let folder = PathBuf::from(expand_home(folder, &st.home));
        let found = match clones::clones_in(&folder) {
            Ok(f) => f,
            Err(said) => {
                println!("{said}");
                unreadable = true;
                continue;
            }
        };
        if found.is_empty() {
            println!("{}: holds no clone", folder.display());
        }
        for dir in found {
            let name = dir.file_name().map_or_else(
                || dir.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            let outcome = clones::freshen(&dir, now);
            match outcome {
                clones::Outcome::FetchFailed(_) => fetch_failed += 1,
                clones::Outcome::UpToDate | clones::Outcome::Behind(_) => fetched += 1,
                clones::Outcome::Left(_) | clones::Outcome::NotCommons => {}
            }
            println!("{}", clones::line(&name, &outcome, now));
        }
    }
    // a clone left alone is named, not a failed unit; a run that could reach nothing, or a folder that is not
    // there, is: the unit shows failed
    Ok(i32::from(unreadable || (fetch_failed > 0 && fetched == 0)))
}

/// `[follow] window` declared and the time of day outside it: says so; the next timer run looks again.
fn outside_window(st: &State) -> Result<bool, String> {
    let Some(window) = st.lab.follow.as_ref().and_then(|f| f.window.as_deref()) else {
        return Ok(false);
    };
    let span = follow::parse_window("[follow]", window)?;
    let clock = clock_now(st)?;
    let minute = follow::parse_clock(&clock)
        .ok_or_else(|| format!("the time of day {clock:?} is not HH:MM"))?;
    if follow::in_window(span, minute) {
        return Ok(false);
    }
    println!("waits: it is {clock}, outside the window {window} a release is installed in; the next timer run looks again");
    Ok(true)
}

/// The time of day, HH:MM local: LPOCO_CLOCK, else the system's.
fn clock_now(st: &State) -> Result<String, String> {
    if let Some(c) = &st.clock {
        return Ok(c.trim().to_string());
    }
    let o = Proc::new("date")
        .arg("+%H:%M")
        .output()
        .map_err(|e| format!("cannot read the time of day (date): {e}"))?;
    if !o.status.success() {
        return Err("cannot read the time of day: date failed".into());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// One declared tool timer's files: each with the mode it is written in, and the user commands (no root) that make them known.
struct ToolTimers {
    files: Vec<(FileChange, u32)>,
    cmds: Vec<Vec<String>>,
}

/// The user units and env files the `[tools.NAME] timers` declare: each unit is the pinned tag's own text
/// (`packaging/systemd/NAME.service|.timer`, fetched), the env file is written from `env` and private (mode 600).
/// Err names the unit the tag does not hold or a fetch that failed: nothing is written then.
fn tool_timer_units(st: &State) -> Result<Vec<ToolTimers>, String> {
    let mut out = Vec::new();
    for pin in st.lab.tools.values() {
        let ToolPin::Release {
            repository,
            tag,
            timers,
            env_file,
            env,
            ..
        } = pin
        else {
            continue;
        };
        if timers.is_empty() {
            continue;
        }
        let dir = format!("{}/.config/systemd/user", st.home);
        let base = format!(
            "{}/{repository}/raw/{tag}/packaging/systemd",
            st.github.trim_end_matches('/')
        );
        let mut files = Vec::new();
        let mut cmds = Vec::new();
        let scratch = PathBuf::from(format!("{}/.cache/lpoco-host", st.home));
        fs::create_dir_all(&scratch)
            .map_err(|e| format!("cannot make {}: {e}", scratch.display()))?;
        for timer in timers {
            for kind in ["service", "timer"] {
                let url = format!("{base}/{timer}.{kind}");
                let to = scratch.join(format!("unit-{timer}.{kind}-{}", std::process::id()));
                if !fetch(&url, &to)? {
                    return Err(format!(
                        "{repository} {tag} holds no packaging/systemd/{timer}.{kind} ({url}): nothing written"
                    ));
                }
                let content = fs::read_to_string(&to)
                    .map_err(|e| format!("cannot read the fetched {url}: {e}"))?;
                let _ = fs::remove_file(&to);
                let dst = format!("{dir}/{timer}.{kind}");
                files.push((
                    FileChange {
                        seen: fs::read_to_string(&dst).ok().into(),
                        content,
                        serials: vec![],
                        dst,
                    },
                    0o644,
                ));
            }
        }
        if let Some(f) = env_file {
            let dst = format!("{}/{f}", st.home);
            let content: String = env
                .iter()
                .map(|(k, v)| format!("{k}={}\n", expand_home(v, &st.home)))
                .collect();
            files.push((
                FileChange {
                    seen: fs::read_to_string(&dst).ok().into(),
                    content,
                    serials: vec![],
                    dst,
                },
                0o600,
            ));
        }
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        if files.iter().any(|(f, _)| f.changed()) {
            cmds.push(s(&["systemctl", "--user", "daemon-reload"]));
        }
        for timer in timers {
            let wanted = format!("{dir}/timers.target.wants/{timer}.timer");
            if files.iter().any(|(f, _)| f.changed()) || !Path::new(&wanted).exists() {
                cmds.push(s(&[
                    "systemctl",
                    "--user",
                    "enable",
                    "--now",
                    &format!("{timer}.timer"),
                ]));
                // a timer already running keeps its old schedule until it is restarted
                let timer_file = format!("{dir}/{timer}.timer");
                if files.iter().any(|(f, _)| {
                    f.dst == timer_file && f.changed() && !matches!(f.seen, Seen::Absent)
                }) {
                    cmds.push(s(&[
                        "systemctl",
                        "--user",
                        "restart",
                        &format!("{timer}.timer"),
                    ]));
                }
            }
        }
        out.push(ToolTimers { files, cmds });
    }
    Ok(out)
}

/// A file that is there with another mode than it is declared in.
fn wrong_mode(dst: &str, mode: u32) -> bool {
    fs::metadata(dst).is_ok_and(|m| m.permissions().mode() & 0o777 != mode)
}
