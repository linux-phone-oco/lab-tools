// SPDX-License-Identifier: MIT OR Apache-2.0
//! `[[host.gadget]] usb_id`: the vendor:product the phone shows on the host's USB bus, so a procedure reads it
//! from the device instead of carrying it.

use lpoco_manifest::Manifest;

fn with(usb_id: &str) -> String {
    format!(
        "[device]\ncodename = \"x\"\n[[host.gadget]]\nserial = \"s\"\naccess = [\"tty\"]\nusb_id = \"{usb_id}\"\n"
    )
}

#[test]
fn a_gadget_declares_the_usb_id_it_shows_and_it_is_kept_lowercase() {
    let m = Manifest::parse(&with("18D1:4ee0")).unwrap();
    assert_eq!(m.host.gadget[0].usb_id.as_deref(), Some("18d1:4ee0"));
}

#[test]
fn a_usb_id_that_is_not_vendor_colon_product_in_four_hex_digits_is_refused_naming_it() {
    for bad in [
        "18d1",
        "18d1:4ee",
        "18d1:4ee00",
        "18d1-4ee0",
        "18dg:4ee0",
        ":",
        "",
    ] {
        let e = Manifest::parse(&with(bad)).unwrap_err();
        assert!(
            e.contains("usb_id") && e.contains(&format!("{bad:?}")),
            "{bad}: {e}"
        );
    }
}
