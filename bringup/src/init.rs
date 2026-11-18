//! The bring-up init: the shell script the phone's initramfs runs as /init, made from the device's
//! `[boot.gadget]` (serial, product, label, whether RNDIS is offered) and the gadget address. Replaces the
//! init files each phone's lab kept for itself (walleye carried about twenty variants).

use crate::initramfs::{rewrite_net, DEFAULT_NET};
use lpoco_manifest::{shell_word, BootGadget};

const TEMPLATE: &str = include_str!("init.sh");

const RNDIS: &str = "mkdir -p $G/functions/rndis.usb0 $G/configs/c.2/strings/0x409
echo \"RNDIS + ACM\" > $G/configs/c.2/strings/0x409/configuration
ln -s $G/functions/rndis.usb0 $G/configs/c.2/
ln -s $G/functions/acm.usb0 $G/configs/c.2/
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
        .replace("@LABEL@", shell_word("label", &g.label)?)
        .replace("@SERIAL@", shell_word("serial", &g.serial)?)
        .replace(
            "@MANUFACTURER@",
            shell_word("manufacturer", &g.manufacturer)?,
        )
        .replace("@PRODUCT@", shell_word("product", &g.product)?))
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
        };
        assert!(make(&g, DEFAULT_NET).unwrap_err().contains("serial"));
    }
}
