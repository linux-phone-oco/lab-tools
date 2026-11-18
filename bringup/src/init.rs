//! The bring-up init: the shell script the phone's initramfs runs as /init, made from the device's
//! `[boot.gadget]` (serial, product, label, whether RNDIS is offered) and the gadget address. Replaces the
//! init files each phone's lab kept for itself (walleye carried about twenty variants).

use crate::initramfs::{rewrite_net, DEFAULT_NET};
use lpoco_manifest::{shell_word, BootGadget};

const TEMPLATE: &str = include_str!("init.sh");

const FILES: &[(&str, &str, u32)] = &[
    ("udhcpd.conf", include_str!("files/udhcpd.conf"), 0o644),
    ("udhcpc.script", include_str!("files/udhcpc.script"), 0o755),
    ("selftest.sh", include_str!("files/selftest.sh"), 0o755),
];

const RNDIS: &str = "mkdir -p $G/functions/rndis.usb0 $G/configs/c.2/strings/0x409
echo \"RNDIS + ACM\" > $G/configs/c.2/strings/0x409/configuration
ln -s $G/functions/rndis.usb0 $G/configs/c.2/
ln -s $G/functions/acm.usb0 $G/configs/c.2/
";

/// The log window's function: a 2 MiB read-only LUN in memory, in the first configuration.
const WINDOW: &str =
    "mkdir -p $G/functions/mass_storage.0 || say \"window: mass_storage function failed\"
L=$G/functions/mass_storage.0/lun.0
WIN=/tmp/window.img
dd if=/dev/zero of=$WIN bs=1024 count=2048 2>/dev/null
echo 1 > $L/ro
echo 1 > $L/removable
echo $WIN > $L/file || say \"window: lun file failed\"
ln -s $G/functions/mass_storage.0 $G/configs/c.1/
";

/// The log window's writer, started before the gadget is bound (the host never reads a blank first image): every 5 s the first 64 KiB of the LUN is
/// rewritten in place with the uptime, the command line and the kernel log.
const WINDOW_LOOP: &str = "( while :; do
    { echo \"@LABEL@-LOG up=$(cut -d' ' -f1 /proc/uptime)s\"; echo '--- uname'; uname -a; echo '--- cmdline'; cat /proc/cmdline
      echo '--- kmsg'; dmesg | tail -n 200; } > /tmp/window.txt
    dd if=/tmp/window.txt of=$WIN bs=65536 count=1 conv=notrunc,sync 2>/dev/null
    sleep 5
done ) &
";

/// The init text for this gadget at 172.16.`net`.1. The address is rewritten in the template first, the
/// device's own words after, so a serial that looks like an address comes out as written.
/// (Re-checks every word: a `BootGadget` can be built without `Manifest::parse`.)
pub fn make(g: &BootGadget, net: u8) -> Result<String, String> {
    let template = if net == DEFAULT_NET {
        TEMPLATE.to_string()
    } else {
        let bytes = rewrite_net(TEMPLATE.as_bytes().to_vec(), net, "init")?;
        String::from_utf8(bytes).map_err(|e| e.to_string())?
    };
    Ok(template
        .replace("@RNDIS@\n", if g.rndis { RNDIS } else { "" })
        .replace("@WINDOW@\n", if g.window { WINDOW } else { "" })
        .replace("@WINDOWLOOP@\n", if g.window { WINDOW_LOOP } else { "" })
        .replace("@LABEL@", shell_word("label", &g.label)?)
        .replace("@SERIAL@", shell_word("serial", &g.serial)?)
        .replace(
            "@MANUFACTURER@",
            shell_word("manufacturer", &g.manufacturer)?,
        )
        .replace("@PRODUCT@", shell_word("product", &g.product)?))
}

/// The files the init reads beside itself: the DHCP server's one lease, the client script and the self-test,
/// each at 172.16.`net`.x. Written into `dir` (made when missing); the scripts are executable.
pub fn write_files(dir: &std::path::Path, net: u8) -> Result<Vec<&'static str>, String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let mut names = Vec::new();
    for (name, text, mode) in FILES {
        let bytes = if net == DEFAULT_NET {
            text.as_bytes().to_vec()
        } else {
            rewrite_net(text.as_bytes().to_vec(), net, name)?
        };
        let p = dir.join(name);
        std::fs::write(&p, bytes).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(*mode))
            .map_err(|e| format!("cannot set the mode of {}: {e}", p.display()))?;
        names.push(*name);
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `make` re-checks: a `BootGadget` built without `Manifest::parse` is still refused.
    #[test]
    fn a_hand_built_gadget_with_a_bad_word_is_refused() {
        let g = BootGadget {
            serial: "a\"b".into(),
            manufacturer: "m".into(),
            product: "p".into(),
            label: "l".into(),
            rndis: true,
            window: false,
        };
        assert!(make(&g, DEFAULT_NET).unwrap_err().contains("serial"));
    }
}
