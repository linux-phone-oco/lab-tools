#!/usr/bin/env rust-script
//! Physical acts as a lane writes them: the step names the act, a bench does it. Nothing here declares a rig, so a person
//! does each act; `--part rig` puts a stand-in relay on the bench that claims unplug_usb and none other, `--part rig-power`
//! one that also claims press_power.
//!
//! ```cargo
//! [dependencies]
//! lpoco-attended = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-attended-v0.29.0" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_attended::prelude::*;

/// A relay on the bench that can pull the USB cable and nothing else.
struct Relay {
    power: bool,
}

impl Bench for Relay {
    fn name(&self) -> &str {
        "relay"
    }
    fn claims(&self, act: &Act) -> bool {
        matches!(act.kind, ActKind::UnplugUsb)
            || (self.power && matches!(act.kind, ActKind::PressPower))
    }
    fn do_act(&self, act: &Act) -> Result<(), String> {
        println!("the relay did: {}", act.to_do());
        Ok(())
    }
}

fn main() {
    let _run = procedure("press, plug and look", "xiaomi-surya");
    match part().as_deref() {
        Some("rig") => bench(Relay { power: false }),
        Some("rig-power") => bench(Relay { power: true }),
        _ => {}
    }
    press_power("the logo shows");
    hold_button("volume down", "the fastboot screen shows");
    unplug_usb("the host no longer lists the phone");
    replug_usb("the host lists the phone again");
    touch("the Settings icon", "Settings opens");
    swipe("up from the bottom edge", "the home screen shows");
    look_for("the boot logo");
    listen_for("the ringtone");
    confirm("shake the phone / nothing rattles");
}
