// SPDX-License-Identifier: MIT OR Apache-2.0
//! The bring-up init: the shell script the phone's initramfs runs as /init, made from the device's
//! `[boot.gadget]` (serial, product, label, whether RNDIS is offered) and the gadget address. Replaces the
//! init files each phone's lab kept for itself (walleye carried about twenty variants).

use crate::initramfs::{rewrite_net, DEFAULT_NET};
use lpoco_manifest::{BootGadget, Console};

const TEMPLATE: &str = include_str!("init.sh");

const FILES: &[(&str, &str, u32)] = &[
    ("udhcpd.conf", include_str!("files/udhcpd.conf"), 0o644),
    ("udhcpc.script", include_str!("files/udhcpc.script"), 0o755),
    ("selftest.sh", include_str!("files/selftest.sh"), 0o755),
];

const RNDIS: &str = "mkdir -p $G/functions/rndis.usb0 $G/configs/c.2/strings/0x409
echo \"RNDIS + @CONSOLENAME@\" > $G/configs/c.2/strings/0x409/configuration
ln -s $G/functions/rndis.usb0 $G/configs/c.2/
ln -s $G/functions/@CONSOLE@ $G/configs/c.2/
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

/// The kernel's ACM console: bind the controller the init waited for.
const BIND_ACM: &str = "if [ -z \"$UDC\" ]; then
    say \"NO UDC after 60 s: the USB controller did not probe\"
else
    echo \"$UDC\" > $G/UDC && say \"gadget bound to $UDC\" || say \"binding $UDC failed\"
fi
";

/// The FunctionFS console: mount the function, start the helper, which writes the descriptors to ep0 and only then binds the UDC (so the init never does).
const BIND_FFS: &str = "mkdir -p /dev/ffs
if ! mount -t functionfs acm /dev/ffs; then
    say \"functionfs mount failed: the gadget is not bound\"
elif [ -x /bin/ffs-acm ]; then
    /bin/ffs-acm &
    say \"ffs-acm started: it binds the UDC after its descriptors\"
else
    say \"ffs-acm missing: /bin/ffs-acm is not in the ramdisk, the gadget is not bound\"
fi
";

/// A shell on the kernel's ACM console for as long as the phone runs (the FunctionFS helper bridges its own).
const SHELL_ACM: &str = "if [ -c /dev/ttyGS0 ]; then
    ( while :; do setsid sh -c 'exec sh </dev/ttyGS0 >/dev/ttyGS0 2>&1'; sleep 1; done ) &
fi
";

/// The init text for this gadget at 172.16.`net`.1. The address is rewritten in the template first, the
/// device's own words after, so a serial that looks like an address comes out as written.
pub fn make(g: &BootGadget, net: u8) -> Result<String, String> {
    let template = if net == DEFAULT_NET {
        TEMPLATE.to_string()
    } else {
        let bytes = rewrite_net(TEMPLATE.as_bytes().to_vec(), net, "init")?;
        String::from_utf8(bytes).map_err(|e| e.to_string())?
    };
    let ffs = g.console == Console::Ffs;
    let text = template
        .replace("@BIND@\n", if ffs { BIND_FFS } else { BIND_ACM })
        .replace("@SHELL@\n", if ffs { "" } else { SHELL_ACM })
        .replace("@RNDIS@\n", if g.rndis { RNDIS } else { "" });
    let (func, name) = if ffs {
        ("ffs.acm", "FFS-ACM")
    } else {
        ("acm.usb0", "ACM")
    };
    Ok(text
        .replace("@WINDOW@\n", if g.window { WINDOW } else { "" })
        .replace("@WINDOWLOOP@\n", if g.window { WINDOW_LOOP } else { "" })
        .replace("@CONSOLE@", func)
        .replace("@CONSOLENAME@", name)
        .replace("@LABEL@", g.label.as_str())
        .replace("@SERIAL@", g.serial.as_str())
        .replace("@MANUFACTURER@", g.manufacturer.as_str())
        .replace("@PRODUCT@", g.product.as_str()))
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
