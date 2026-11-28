// SPDX-License-Identifier: MIT OR Apache-2.0
//! install.toml: how one packed image reaches a device, read and turned into the commands that do it.
//! The skeleton knows one way: boot an image from RAM over fastboot (nothing written), then the declared way
//! back. What it does not allow (a write to the device, an unknown transport) does not parse: a refused plan
//! fails at `read`, naming the value (a later item, osfact-install-flash, adds the writes).

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub install: Install,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Install {
    pub device: String,
    /// The environment variable that holds the device's serial; every fastboot call names it.
    pub serial_env: String,
    pub image: Vec<Image>,
    pub step: Vec<Step>,
    pub way_back: WayBack,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub name: String,
    pub file: String,
    /// What the pack measured; the run that sends the image checks it (a later item).
    pub sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    #[serde(rename = "do")]
    pub action: StepAction,
    pub write: String,
    pub via: Transport,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepAction {
    BootFromRam,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    Fastboot,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WayBack {
    pub via: Transport,
    #[serde(rename = "do")]
    pub action: WayBackAction,
    pub wait: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WayBackAction {
    Reboot,
}

pub fn read(text: &str) -> Result<Plan, String> {
    let plan: Plan =
        toml::from_str(text).map_err(|e| format!("osfact refuses this install.toml: {e}"))?;
    if plan.install.step.is_empty() {
        return Err("osfact refuses this install.toml: it has no boot step".into());
    }
    Ok(plan)
}

/// The commands of a plan in order, the way back last.
pub fn commands(plan: &Plan) -> Result<Vec<String>, String> {
    let i = &plan.install;
    let mut out = Vec::new();
    for s in &i.step {
        let image = i.image.iter().find(|m| m.name == s.write).ok_or_else(|| {
            format!(
                "step writes image '{}', which install.toml does not list",
                s.write
            )
        })?;
        out.push(format!("fastboot -s ${} boot {}", i.serial_env, image.file));
    }
    out.push(format!(
        "way back: fastboot -s ${} reboot (wait {} s for {})",
        i.serial_env, i.way_back.wait, i.device
    ));
    Ok(out)
}
