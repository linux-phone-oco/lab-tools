// SPDX-License-Identifier: MIT OR Apache-2.0
//! A test kernel beside the stock one, tried once, never stranding the phone - proven on surya:
//! - boot counting: the entry is pmos-NAME+1-0.conf, sorted ahead of stock; systemd-boot counts the
//!   try down on the ESP, an entry with no tries left sorts last, so the next boot is stock; the test
//!   kernel's command line masks systemd-bless-boot, so even a good boot runs once. No EFI variables.
//! - a boot timeout: a second initramfs adds a hook that, with no real system after 5 minutes, saves
//!   the initramfs log and kernel messages to the ESP and reboots;
//! - the test kernel's own copies of the modules the stock initramfs carries.

use crate::phone::Phone;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The hook the second initramfs adds: shell builtins decide, so it cannot misfire after switch_root.
pub const TIMEOUT_HOOK: &str = r#"#!/bin/sh
# A test kernel's boot timeout: no real system after 5 minutes -> save the log to the ESP, reboot.
(
	sleep 300
	read -r pid1 < /proc/1/comm
	[ "$pid1" = systemd ] && exit 0
	echo "test boot timeout: saving the log to the ESP and rebooting" > /dev/kmsg
	uuid=$(sed -n 's/.*pmos_boot_uuid=\([^ ]*\).*/\1/p' /proc/cmdline)
	mkdir -p /tmp/esp
	if mount -t vfat "$(findfs "UUID=$uuid")" /tmp/esp; then
		cp /pmOS_init.log /tmp/esp/test-boot.log 2>/dev/null
		dmesg > /tmp/esp/test-boot.dmesg
		umount /tmp/esp
	fi
	sync
	reboot -f
) &
"#;

/// A name for files on the ESP: lowercase letters, digits and dashes.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The kernel release a modules archive holds, from its listing (`lib/modules/RELEASE/`).
pub fn release_from_listing(listing: &str) -> Option<String> {
    listing.lines().find_map(|l| {
        let rest = l.trim_start_matches("./").strip_prefix("lib/modules/")?;
        let (rel, tail) = rest.split_once('/')?;
        (tail.is_empty() && !rel.is_empty()).then(|| rel.to_string())
    })
}

/// The module paths (below the release folder) an initramfs listing carries - lib/ or usr/lib/.
pub fn module_names(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter_map(|l| {
            let l = l.trim_start_matches("./");
            let rest = l
                .strip_prefix("usr/lib/modules/")
                .or_else(|| l.strip_prefix("lib/modules/"))?;
            let (_, path) = rest.split_once('/')?;
            path.contains(".ko").then(|| path.to_string())
        })
        .collect()
}

/// An EFI program begins with "MZ" (the PE header the bootloader loads).
pub fn is_efi_executable(head: &[u8]) -> bool {
    head.starts_with(b"MZ")
}

/// The boot entry: sorted ahead of stock, both initramfs images, bless masked.
/// The release whose modules an entry boots with is said in the entry itself (a comment, which the
/// bootloader skips), so that removing one kernel never takes the modules another still boots with.
pub const RELEASE_MARK: &str = "# devlab release ";

pub fn entry(name: &str, dtb: &str, options: &str, release: &str) -> String {
    format!(
        "{RELEASE_MARK}{release}\ntitle postmarketOS (test kernel: {name})\nsort-key 0-test-{name}\nlinux vmlinuz-{name}\ninitrd initramfs\ninitrd initramfs-{name}\ndevicetree dtbs-{name}/qcom/{dtb}\noptions {options} systemd.mask=systemd-bless-boot.service\n"
    )
}

fn run(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().with_context(|| format!("running {cmd:?}"))?;
    if !out.status.success() {
        bail!(
            "{cmd:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A fixed shell pipeline, its paths as arguments ($1, $2...), never spliced into the script.
fn pipeline(script: &str, args: &[&Path], dir: Option<&Path>) -> Result<String> {
    let mut c = Command::new("sh");
    c.arg("-c")
        .arg(script)
        .arg("pipeline")
        .args(args)
        .stderr(Stdio::inherit());
    if let Some(d) = dir {
        c.current_dir(d);
    }
    run(&mut c)
}

pub fn install(
    phone: &Phone,
    name: &str,
    vmlinuz: &Path,
    dtb: &Path,
    mods: &Path,
    extra: &str,
) -> Result<()> {
    // the bootloader starts an EFI program and nothing else: a gzip or raw Image never begins, and
    // the try is lost before the kernel can log anything (2026-11-11: Image.gz was given)
    let head = std::fs::read(vmlinuz).with_context(|| format!("reading {}", vmlinuz.display()))?;
    if !is_efi_executable(&head) {
        bail!("{} is not an EFI executable the bootloader can start: give the EFI image (vmlinuz.efi, or Image with the EFI stub)", vmlinuz.display());
    }
    if extra.contains('\n') || extra.contains('$') || extra.contains('`') {
        bail!("extra kernel options must be one line, without $ or backquotes");
    }
    if !valid_name(name) {
        bail!("{name:?}: a test kernel's name is lowercase letters, digits and dashes");
    }
    let listing = run(Command::new("tar").arg("tzf").arg(mods))?;
    let release = release_from_listing(&listing)
        .with_context(|| format!("no lib/modules/RELEASE/ in {}", mods.display()))?;
    let running = phone.sh("uname -r\n")?.1.trim().to_string();
    if release == running {
        bail!("{release} is the running kernel's version: its modules would replace the stock ones - refusing");
    }
    let w = tempdir()?;
    let result = (|| -> Result<()> {
        let m = w.join("mods");
        std::fs::create_dir_all(&m)?;
        run(Command::new("tar").arg("xzf").arg(mods).arg("-C").arg(&m))?;
        // the stock initramfs, as bytes, to learn which modules it carries
        let stock = w.join("stock.cpio.gz");
        std::fs::write(&stock, phone.root_bytes("cat /boot/initramfs\n")?)?;
        let names = module_names(&pipeline(
            "gzip -dc \"$1\" | cpio -it --quiet",
            &[&stock],
            None,
        )?);
        let src = m.join("lib/modules").join(&release);
        let stage = w.join("stage/lib/modules").join(&release);
        let mut carried = 0;
        for n in &names {
            let from = src.join(n);
            if from.is_file() {
                let to = stage.join(n);
                std::fs::create_dir_all(to.parent().context("module path")?)?;
                std::fs::copy(&from, &to)?;
                carried += 1;
            }
        }
        if carried == 0 {
            bail!(
                "none of the stock initramfs's {} modules exist for {release}",
                names.len()
            );
        }
        for f in [
            "modules.order",
            "modules.builtin",
            "modules.builtin.modinfo",
        ] {
            let _ = std::fs::copy(src.join(f), stage.join(f));
        }
        run(Command::new("depmod")
            .arg("-b")
            .arg(w.join("stage"))
            .arg(&release))?;
        // the second initramfs: usr/lib/modules only (lib is a link there), and the timeout hook
        let rd = w.join("rd");
        std::fs::create_dir_all(rd.join("usr/lib/modules"))?;
        std::fs::create_dir_all(rd.join("hooks"))?;
        run(Command::new("cp")
            .arg("-a")
            .arg(&stage)
            .arg(rd.join("usr/lib/modules")))?;
        let hook = rd.join("hooks/00-test-boot-timeout.sh");
        std::fs::write(&hook, TIMEOUT_HOOK)?;
        run(Command::new("chmod").arg("755").arg(&hook))?;
        let initrd = w.join(format!("initramfs-{name}"));
        pipeline(
            "find . | cpio -o -H newc -R root:root --quiet | gzip -9 > \"$1\"",
            &[&initrd],
            Some(&rd),
        )?;
        eprintln!("second initramfs: {carried} modules + the timeout hook");
        // onto the phone
        let dtbname = dtb
            .file_name()
            .and_then(|f| f.to_str())
            .context("device tree file name")?;
        for (local, remote) in [
            (vmlinuz, "/tmp/te-vmlinuz"),
            (dtb, "/tmp/te-dtb"),
            (mods, "/tmp/te-mods.tgz"),
            (initrd.as_path(), "/tmp/te-initramfs"),
        ] {
            phone.put(local.to_str().context("path")?, remote)?;
        }
        let script = format!(
            "set -e\nrm -f /boot/loader/entries/pmos-{name}.conf /boot/loader/entries/pmos-{name}+*.conf\n\
             install -m 644 /tmp/te-vmlinuz /boot/vmlinuz-{name}\ninstall -m 644 /tmp/te-initramfs /boot/initramfs-{name}\n\
             mkdir -p /boot/dtbs-{name}/qcom && install -m 644 /tmp/te-dtb /boot/dtbs-{name}/qcom/{dtbname}\n\
             tar xzf /tmp/te-mods.tgz -C / && depmod -a {release}\n\
             options=$(sed -n 's/^options //p' /boot/loader/entries/pmos.conf)\n\
             cat > '/boot/loader/entries/pmos-{name}+1-0.conf' <<ENTRY\n{entry}ENTRY\n\
             rm -f /tmp/te-vmlinuz /tmp/te-dtb /tmp/te-mods.tgz /tmp/te-initramfs\n\
             echo \"installed: pmos-{name}+1-0.conf (one try), kernel {release}, $(find /lib/modules/{release} -name '*.ko*' | wc -l) modules\"\n",
            entry = entry(name, dtbname, &format!("$options{}{extra}", if extra.is_empty() { "" } else { " " }), &release)
        );
        let (ok, out) = phone.root(&script)?;
        eprint!("{out}");
        if !ok {
            bail!("installing on the phone failed");
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&w);
    result
}

/// What a root shell does to set up a device-tree try of installed kernel NAME: an entry NAME-dt with
/// one try, NAME's kernel, initramfs and modules and the device tree put at /tmp/te-dtb, bless masked -
/// whatever NAME is (kept or test). The entry says NAME's release, so removing NAME-dt keeps the modules.
pub fn with_dtb_script(name: &str, dtb: &str) -> String {
    format!(
        "set -e\ncd /boot/loader/entries\n\
         from=$(ls pmos-{name}.conf pmos-{name}+*.conf 2>/dev/null | head -n1)\n\
         [ -n \"$from\" ] || {{ echo 'pmos-{name} is not installed' >&2; rm -f /tmp/te-dtb; exit 1; }}\n\
         rm -f pmos-{name}-dt.conf pmos-{name}-dt+*.conf\nrm -rf /boot/dtbs-{name}-dt\n\
         mkdir -p /boot/dtbs-{name}-dt/qcom && install -m 644 /tmp/te-dtb /boot/dtbs-{name}-dt/qcom/{dtb} && rm -f /tmp/te-dtb\n\
         release=$(sed -n 's/^{RELEASE_MARK}//p' \"$from\")\n\
         {{ [ -z \"$release\" ] || echo \"{RELEASE_MARK}$release\"\n\
         sed -e '/^{RELEASE_MARK}/d' -e 's/^title .*/title postmarketOS (test device tree: {name}, {dtb})/' -e 's/^sort-key .*/sort-key 0-test-{name}-dt/' \
         -e 's|^devicetree .*|devicetree dtbs-{name}-dt/qcom/{dtb}|' \
         -e '/^options /s/ systemd.mask=systemd-bless-boot.service//' -e '/^options /s/$/ systemd.mask=systemd-bless-boot.service/' \"$from\"; }} > 'pmos-{name}-dt+1-0.conf'\n\
         sync\necho 'installed: pmos-{name}-dt+1-0.conf (one try): the kernel of pmos-{name} with {dtb}'\n"
    )
}

/// Sets up a device-tree try of installed kernel NAME; the try is then NAME-dt's.
pub fn with_dtb(phone: &Phone, name: &str, dtb: &Path) -> Result<String> {
    if !valid_name(name) {
        bail!("{name:?}: not a test kernel name");
    }
    let file = dtb
        .file_name()
        .and_then(|f| f.to_str())
        .context("the device tree's file name")?;
    if !file.ends_with(".dtb")
        || !file
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        bail!("{file:?}: a device tree is a .dtb, its name letters, digits, - _ and .");
    }
    phone.put(dtb.to_str().context("path")?, "/tmp/te-dtb")?;
    let (ok, out) = phone.root(&with_dtb_script(name, file))?;
    eprint!("{out}");
    if !ok {
        bail!("setting up the device-tree try failed");
    }
    Ok(format!("{name}-dt"))
}

pub fn try_once(phone: &Phone, name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("{name:?}: not a test kernel name");
    }
    let (ok, out) = phone.root(&format!(
        "set -e\nls /boot/loader/entries/ | grep -q '^pmos-{name}+[1-9]' || {{ echo 'pmos-{name} has no try left: install it again' >&2; exit 1; }}\n\
         rm -f /boot/test-boot.log /boot/test-boot.dmesg\nsync; systemd-run --on-active=2 systemctl reboot >/dev/null\necho 'trying pmos-{name} once'\n"
    ))?;
    eprint!("{out}");
    if !ok {
        bail!("no try started");
    }
    Ok(())
}

/// How many boots a kept kernel has to come up well before it sorts last again and stock boots.
pub const KEEP_TRIES: u32 = 3;

/// What a root shell does to keep test kernel NAME as the default: its entry rewritten - a kept title
/// and a sort key after every test entry's (a try still comes first) but before stock's, a kept title
/// and sort key, systemd-bless-boot no longer masked, KEEP_TRIES tries - so that its first good boot is
/// blessed (the counter dropped: the default from then on), and three bad ones in a row sort it last
/// (stock boots by itself). One kernel is kept at a time.
pub fn keep_script(name: &str) -> String {
    format!(
        "set -e\ncd /boot/loader/entries\n\
         from=$(ls pmos-{name}.conf pmos-{name}+*.conf 2>/dev/null | head -n1)\n\
         [ -n \"$from\" ] || {{ echo 'pmos-{name} is not installed: lpoco-lab kernel install {name} ... first' >&2; exit 1; }}\n\
         other=$(grep -l '^sort-key 1-kept-' *.conf 2>/dev/null | grep -v \"^pmos-{name}[.+]\" || true)\n\
         [ -z \"$other\" ] || {{ echo \"another kernel is kept ($other): lpoco-lab kernel unkeep it first\" >&2; exit 1; }}\n\
         sed -e 's/^title .*/title postmarketOS (kept kernel: {name})/' -e 's/^sort-key .*/sort-key 1-kept-{name}/' \
         -e 's/ systemd.mask=systemd-bless-boot.service//' \"$from\" > /tmp/kept-entry\n\
         rm -f pmos-{name}.conf pmos-{name}+*.conf\n\
         install -m 644 /tmp/kept-entry 'pmos-{name}+{KEEP_TRIES}-0.conf' && rm -f /tmp/kept-entry\n\
         sync\necho 'kept: pmos-{name}+{KEEP_TRIES}-0.conf - the default from the next boot; blessed by its first good boot, stock after {KEEP_TRIES} bad ones'\n"
    )
}

/// What a root shell does to give the default back to stock: the kept entry becomes a test entry again,
/// with no try left (bless masked, sorted last); the kernel stays installed.
pub fn unkeep_script(name: &str) -> String {
    format!(
        "set -e\ncd /boot/loader/entries\n\
         from=$(ls pmos-{name}.conf pmos-{name}+*.conf 2>/dev/null | head -n1)\n\
         [ -n \"$from\" ] || {{ echo 'pmos-{name} is not installed' >&2; exit 1; }}\n\
         grep -q '^sort-key 1-kept-{name}$' \"$from\" || {{ echo 'pmos-{name} is not kept' >&2; exit 1; }}\n\
         sed -e 's/^title .*/title postmarketOS (test kernel: {name})/' -e 's/^sort-key .*/sort-key 0-test-{name}/' \
         -e '/^options /s/$/ systemd.mask=systemd-bless-boot.service/' \"$from\" > /tmp/test-entry\n\
         rm -f pmos-{name}.conf pmos-{name}+*.conf\n\
         install -m 644 /tmp/test-entry 'pmos-{name}+0-0.conf' && rm -f /tmp/test-entry\n\
         sync\necho 'unkept: stock is the default from the next boot; pmos-{name} stays installed, with no try left'\n"
    )
}

pub fn keep(phone: &Phone, name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("{name:?}: not a test kernel name");
    }
    let (ok, out) = phone.root(&keep_script(name))?;
    eprint!("{out}");
    if !ok {
        bail!("keeping pmos-{name} failed");
    }
    Ok(())
}

pub fn unkeep(phone: &Phone, name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("{name:?}: not a test kernel name");
    }
    let (ok, out) = phone.root(&unkeep_script(name))?;
    eprint!("{out}");
    if !ok {
        bail!("unkeeping pmos-{name} failed");
    }
    Ok(())
}

pub fn log(phone: &Phone) -> Result<()> {
    let (_, out) = phone.root("for f in /boot/test-boot.log /boot/test-boot.dmesg; do [ -f $f ] && { echo \"== $f\"; tail -60 $f; }; done; true\n")?;
    print!("{out}");
    Ok(())
}

pub fn remove(phone: &Phone, name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("{name:?}: not a test kernel name");
    }
    let (ok, out) = phone.root(&remove_script(name))?;
    eprint!("{out}");
    if !ok {
        bail!("removing failed");
    }
    Ok(())
}

/// What a root shell does to remove test kernel NAME: its entry and files - and its release's modules
/// only when nothing else may boot with them. They stay when the running kernel is that release,
/// another entry says it uses it, another entry does not say which it uses (one installed before
/// entries said it), or a package owns them; the kept kernel is not removed at all (unkeep it first).
pub fn remove_script(name: &str) -> String {
    format!(
        "set -e\ncd /boot/loader/entries\n\
         mine=$(ls pmos-{name}.conf pmos-{name}+*.conf 2>/dev/null | head -n1)\n\
         if [ -n \"$mine\" ] && grep -q '^sort-key 1-kept-' \"$mine\"; then echo 'pmos-{name} is the kept kernel: lpoco-lab kernel unkeep {name} now first' >&2; exit 1; fi\n\
         release=''\n[ -z \"$mine\" ] || release=$(sed -n 's/^{RELEASE_MARK}//p' \"$mine\")\n\
         [ -n \"$release\" ] || release=$(ls /lib/modules | grep -- '-{name}$' || true)\n\
         keep=''\n\
         if [ -n \"$release\" ]; then\n\
         [ \"$(uname -r)\" != \"$release\" ] || keep=\"the running kernel is $release\"\n\
         for e in *.conf; do\n\
         case \"$e\" in pmos-{name}.conf|pmos-{name}+*|pmos.conf) continue ;; esac\n\
         r=$(sed -n 's/^{RELEASE_MARK}//p' \"$e\")\n\
         if [ \"$r\" = \"$release\" ]; then keep=\"$e boots with it\"; elif [ -z \"$r\" ] && [ -z \"$keep\" ]; then keep=\"$e does not say which release it boots with\"; fi\n\
         done\n\
         apk info -q --who-owns \"/lib/modules/$release\" >/dev/null 2>&1 && keep=\"a package owns them\"\n\
         fi\n\
         rm -f pmos-{name}.conf pmos-{name}+*.conf /boot/vmlinuz-{name} /boot/initramfs-{name} /boot/test-boot.log /boot/test-boot.dmesg\n\
         rm -rf /boot/dtbs-{name}\n\
         if [ -z \"$release\" ]; then echo 'removed: pmos-{name} (no modules of its own found)'\n\
         elif [ -n \"$keep\" ]; then echo \"removed: pmos-{name}; the modules of $release stay: $keep\"\n\
         else rm -rf \"/lib/modules/$release\"; echo \"removed: pmos-{name} and the modules of $release\"; fi\n"
    )
}

fn tempdir() -> Result<PathBuf> {
    let d = std::env::temp_dir().join(format!("devlab-kernel-{}", std::process::id()));
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_kernel_is_blessed_once_and_falls_back_after_three_bad_boots() {
        let k = keep_script("mic");
        assert!(k.contains("s/^sort-key .*/sort-key 1-kept-mic/"));
        assert!(
            k.contains("s/ systemd.mask=systemd-bless-boot.service//"),
            "bless no longer masked"
        );
        assert!(k.contains("install -m 644 /tmp/kept-entry 'pmos-mic+3-0.conf'"));
        assert!(k.contains("another kernel is kept"), "one kept at a time");
        let mut keys = ["postmarketOS", "1-kept-mic", "0-test-wcd"];
        keys.sort();
        assert_eq!(
            keys,
            ["0-test-wcd", "1-kept-mic", "postmarketOS"],
            "a try first, then the kept kernel, then stock"
        );
        let u = unkeep_script("mic");
        assert!(
            u.contains("'pmos-mic+0-0.conf'"),
            "no try left: sorted last"
        );
        assert!(u.contains("/^options /s/$/ systemd.mask=systemd-bless-boot.service/"));
        assert!(
            u.contains("grep -q '^sort-key 1-kept-mic$'"),
            "only a kept kernel is unkept"
        );
    }

    /// Runs remove_script in a fake root: /boot and /lib/modules under `root`, uname saying `running`.
    fn removed(root: &Path, name: &str, running: &str) -> (bool, String) {
        let r = root.to_string_lossy();
        let script = remove_script(name)
            .replace("/boot/", &format!("{r}/boot/"))
            .replace("/lib/modules", &format!("{r}/lib/modules"));
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(
            root.join("bin/uname"),
            format!("#!/bin/sh\necho {running}\n"),
        )
        .unwrap();
        std::fs::write(root.join("bin/apk"), "#!/bin/sh\nexit 1\n").unwrap();
        for b in ["uname", "apk"] {
            std::fs::set_permissions(
                root.join("bin").join(b),
                std::os::unix::fs::PermissionsExt::from_mode(0o755),
            )
            .unwrap();
        }
        let out = Command::new("sh")
            .arg("-c")
            .arg(&script)
            .env("PATH", format!("{r}/bin:/usr/bin:/bin"))
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
        )
    }

    #[test]
    fn removing_a_kernel_never_takes_modules_another_boots_with() {
        let root = std::env::temp_dir().join(format!("devlab-remove-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let e = root.join("boot/loader/entries");
        std::fs::create_dir_all(&e).unwrap();
        std::fs::create_dir_all(root.join("lib/modules/7.1.0-mic+")).unwrap();
        std::fs::create_dir_all(root.join("lib/modules/7.1.0-other")).unwrap();
        std::fs::write(e.join("pmos.conf"), "title postmarketOS\n").unwrap();
        std::fs::write(
            e.join("pmos-mic.conf"),
            "# devlab release 7.1.0-mic+\nsort-key 1-kept-mic\n",
        )
        .unwrap();
        std::fs::write(
            e.join("pmos-ear+0-1.conf"),
            "# devlab release 7.1.0-mic+\nsort-key 0-test-ear\n",
        )
        .unwrap();
        std::fs::write(
            e.join("pmos-old+0-1.conf"),
            "# devlab release 7.1.0-other\nsort-key 0-test-old\n",
        )
        .unwrap();
        // the kept kernel is not removed at all
        let (ok, said) = removed(&root, "mic", "6.1-stock");
        assert!(!ok && said.contains("unkeep"), "{said}");
        // ear shares the kept kernel's release: its entry goes, the modules stay
        let (ok, said) = removed(&root, "ear", "6.1-stock");
        assert!(ok && said.contains("pmos-mic.conf boots with it"), "{said}");
        assert!(
            !e.join("pmos-ear+0-1.conf").exists() && root.join("lib/modules/7.1.0-mic+").exists()
        );
        // old is the only one with its release: the modules go with it
        let (ok, said) = removed(&root, "old", "6.1-stock");
        assert!(
            ok && said.contains("and the modules of 7.1.0-other"),
            "{said}"
        );
        assert!(!root.join("lib/modules/7.1.0-other").exists());
        // an entry that does not say its release: nothing is taken on a guess
        std::fs::create_dir_all(root.join("lib/modules/7.1.0-x")).unwrap();
        std::fs::write(e.join("pmos-x+0-1.conf"), "# devlab release 7.1.0-x\n").unwrap();
        std::fs::write(e.join("pmos-legacy+0-1.conf"), "sort-key 0-test-legacy\n").unwrap();
        let (ok, said) = removed(&root, "x", "6.1-stock");
        assert!(ok && said.contains("does not say which release"), "{said}");
        assert!(root.join("lib/modules/7.1.0-x").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_device_tree_try_reuses_the_kernel_once() {
        let s = with_dtb_script("mic", "sm7150-xiaomi-surya-huaxing.dtb");
        assert!(s.contains("> 'pmos-mic-dt+1-0.conf'"), "one try");
        assert!(s.contains(
            "s|^devicetree .*|devicetree dtbs-mic-dt/qcom/sm7150-xiaomi-surya-huaxing.dtb|"
        ));
        assert!(
            s.contains("s/^sort-key .*/sort-key 0-test-mic-dt/"),
            "ahead of the kept kernel"
        );
        assert!(
            s.contains("'/^options /s/$/ systemd.mask=systemd-bless-boot.service/'"),
            "never blessed"
        );
        assert!(
            s.contains("echo \"# devlab release $release\""),
            "its release said, so remove keeps the modules"
        );
    }

    #[test]
    fn names_are_safe_for_the_esp() {
        assert!(valid_name("wcd") && valid_name("wcd-2"));
        assert!(!valid_name("") && !valid_name("../x") && !valid_name("a b") && !valid_name("WCD"));
    }

    #[test]
    fn the_release_comes_from_the_archive_listing() {
        let l = "lib/\nlib/modules/\nlib/modules/7.1.0-rc3-sm7150-wcd/\nlib/modules/7.1.0-rc3-sm7150-wcd/kernel/a.ko.zst\n";
        assert_eq!(
            release_from_listing(l).as_deref(),
            Some("7.1.0-rc3-sm7150-wcd")
        );
        assert_eq!(release_from_listing("lib/\n"), None);
    }

    #[test]
    fn module_names_read_both_layouts() {
        let l = "usr/lib/modules/7.1.0-rc3-sm7150/kernel/drivers/input/evdev.ko.zst\n./lib/modules/6.1/kernel/fs/xfs/xfs.ko\nusr/lib/modules/7.1.0-rc3-sm7150/modules.dep\nbin/sh\n";
        assert_eq!(
            module_names(l),
            ["kernel/drivers/input/evdev.ko.zst", "kernel/fs/xfs/xfs.ko"]
        );
    }

    #[test]
    fn the_entry_sorts_first_carries_both_initramfs_and_masks_bless() {
        let e = entry(
            "wcd",
            "sm7150-xiaomi-surya-huaxing.dtb",
            "quiet splash",
            "7.1.0-wcd",
        );
        assert!(
            e.starts_with("# devlab release 7.1.0-wcd\n"),
            "the release said, as a comment"
        );
        assert!(
            e.contains("sort-key 0-test-wcd\n"),
            "'0' sorts before 'postmarketOS'"
        );
        assert!(e.contains("initrd initramfs\ninitrd initramfs-wcd\n"));
        assert!(e.ends_with("options quiet splash systemd.mask=systemd-bless-boot.service\n"));
    }

    #[test]
    fn the_timeout_hook_decides_with_builtins_only() {
        assert!(
            TIMEOUT_HOOK.contains("read -r pid1 < /proc/1/comm")
                && TIMEOUT_HOOK.contains("[ \"$pid1\" = systemd ] && exit 0")
        );
    }

    #[test]
    fn only_an_efi_program_is_offered_to_the_bootloader() {
        assert!(is_efi_executable(b"MZ\x40\x00rest"));
        assert!(!is_efi_executable(&[0x1f, 0x8b, 0x08]), "gzip");
        assert!(!is_efi_executable(b""));
    }
}
