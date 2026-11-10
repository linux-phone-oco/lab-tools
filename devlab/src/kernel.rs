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

/// The boot entry: sorted ahead of stock, both initramfs images, bless masked.
pub fn entry(name: &str, dtb: &str, options: &str) -> String {
    format!(
        "title postmarketOS (test kernel: {name})\nsort-key 0-test-{name}\nlinux vmlinuz-{name}\ninitrd initramfs\ninitrd initramfs-{name}\ndevicetree dtbs-{name}/qcom/{dtb}\noptions {options} systemd.mask=systemd-bless-boot.service\n"
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
        println!("second initramfs: {carried} modules + the timeout hook");
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
            entry = entry(name, dtbname, &format!("$options{}{extra}", if extra.is_empty() { "" } else { " " }))
        );
        let (ok, out) = phone.root(&script)?;
        print!("{out}");
        if !ok {
            bail!("installing on the phone failed");
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&w);
    result
}

pub fn try_once(phone: &Phone, name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("{name:?}: not a test kernel name");
    }
    let (ok, out) = phone.root(&format!(
        "set -e\nls /boot/loader/entries/ | grep -q '^pmos-{name}+[1-9]' || {{ echo 'pmos-{name} has no try left: install it again' >&2; exit 1; }}\n\
         rm -f /boot/test-boot.log /boot/test-boot.dmesg\nsync; systemd-run --on-active=2 systemctl reboot >/dev/null\necho 'trying pmos-{name} once'\n"
    ))?;
    print!("{out}");
    if !ok {
        bail!("no try started");
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
    let (ok, out) = phone.root(&format!(
        "set -e\nrelease=$(ls /lib/modules | grep -- '-{name}$' || true)\n\
         rm -f /boot/loader/entries/pmos-{name}.conf /boot/loader/entries/pmos-{name}+*.conf /boot/vmlinuz-{name} /boot/initramfs-{name} /boot/test-boot.log /boot/test-boot.dmesg\n\
         rm -rf /boot/dtbs-{name}\n[ -z \"$release\" ] || rm -rf \"/lib/modules/$release\"\necho \"removed: pmos-{name}${{release:+ and the modules of $release}}\"\n"
    ))?;
    print!("{out}");
    if !ok {
        bail!("removing failed");
    }
    Ok(())
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
        let e = entry("wcd", "sm7150-xiaomi-surya-huaxing.dtb", "quiet splash");
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
}
