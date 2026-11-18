//! The bring-up init: the shell script the phone's initramfs runs as /init, made from the device's
//! `[boot.gadget]` (serial, product, label, whether RNDIS is offered) and the gadget address. Replaces the
//! init files each phone's lab kept for itself (walleye carried about twenty variants).

use crate::initramfs::{rewrite_net, DEFAULT_NET};
use lpoco_manifest::BootGadget;

const TEMPLATE: &str = include_str!("init.sh");

const RNDIS: &str = "mkdir -p $G/functions/rndis.usb0 $G/configs/c.2/strings/0x409
echo \"RNDIS + ACM\" > $G/configs/c.2/strings/0x409/configuration
ln -s $G/functions/rndis.usb0 $G/configs/c.2/
ln -s $G/functions/acm.usb0 $G/configs/c.2/
";

/// A value that goes inside double quotes in the script: nothing that would end or expand them.
fn quoted(what: &str, v: &str) -> Result<String, String> {
    if v.is_empty()
        || v.chars()
            .any(|c| matches!(c, '"' | '$' | '`' | '\\' | '\n' | '@'))
    {
        return Err(format!(
            "[boot.gadget] {what} = {v:?}: not empty, and none of \" $ ` \\ @ or a line break"
        ));
    }
    Ok(v.to_string())
}

/// The init text for this gadget at 172.16.`net`.1. The address is rewritten in the template first, the
/// device's own words after, so a serial that looks like an address comes out as written.
pub fn make(g: &BootGadget, net: u8) -> Result<String, String> {
    let template = if net == DEFAULT_NET {
        TEMPLATE.to_string()
    } else {
        let bytes = rewrite_net(TEMPLATE.as_bytes().to_vec(), net, "init")?;
        String::from_utf8(bytes).map_err(|e| e.to_string())?
    };
    Ok(template
        .replace("@RNDIS@\n", if g.rndis { RNDIS } else { "" })
        .replace("@LABEL@", &quoted("label", &g.label)?)
        .replace("@SERIAL@", &quoted("serial", &g.serial)?)
        .replace("@MANUFACTURER@", &quoted("manufacturer", &g.manufacturer)?)
        .replace("@PRODUCT@", &quoted("product", &g.product)?))
}
