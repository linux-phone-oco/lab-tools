// SPDX-License-Identifier: MIT OR Apache-2.0
//! devlab's structured door, over the Model Context Protocol (stdio): the phone for any agent, without
//! a person and without a shell's quoting layer. Each tool calls the same function the command line
//! does - one door each, two doors to one thing (agent access, aa-two-doors).
//!
//!   claude mcp add --scope user phone -- devlab mcp            (the manifest: found from where it runs)
//!   claude mcp add --scope user phone -- devlab --device PATH/device.toml mcp
//!
//! Nothing here writes to standard output but the protocol: what the functions say is returned.

use crate::device::Device;
use crate::{kernel, outcome, phone, suite};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use serde::Deserialize;

#[derive(Clone)]
pub struct Door {
    device: Device,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct Run {
    /// The commands, as a shell script (run with sh on the phone; no quoting layer)
    pub script: String,
    /// Run as root (default: as the phone's user)
    #[serde(default)]
    pub root: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct Put {
    /// A file on the lab machine
    pub local: String,
    /// Where it goes on the phone (a place the phone's user may write, such as /tmp/...)
    pub remote: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct Get {
    /// A file on the phone (read as root)
    pub remote: String,
    /// Where it goes on the lab machine
    pub local: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct KernelInstall {
    /// The entry's name: letters, digits and dashes
    pub name: String,
    /// The kernel image (EFI) on the lab machine
    pub vmlinuz: String,
    /// The device tree on the lab machine
    pub dtb: String,
    /// A tar.gz holding lib/modules/RELEASE
    pub modules: String,
    /// Extra kernel options for this entry only
    #[serde(default)]
    pub cmdline: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SwitchOff {
    /// The test to switch off (health_switches says them)
    pub test: String,
    /// Why it is off: never switched off without one
    pub reason: String,
    /// Who asks (the agent or person; "devlab" if unsaid)
    pub by: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SwitchOn {
    /// The test to switch on again
    pub test: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct KernelName {
    /// The test kernel entry's name, as installed
    pub name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct Image {
    /// A boot image on the lab machine
    pub image: String,
    /// The device manifest to count it against (default: device.toml here or above)
    pub device: Option<String>,
}

/// The family's program `lpoco-NAME`: beside this one, else on PATH.
fn family_program(name: &str) -> std::path::PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .filter(|p| p.is_file());
    beside.unwrap_or_else(|| name.into())
}

/// Runs a family program; what it printed, and whether it succeeded.
fn family_run(name: &str, args: &[&str]) -> Result<String, String> {
    let o = std::process::Command::new(family_program(name))
        .args(args)
        .output()
        .map_err(|e| format!("cannot run {name}: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    Ok(format!(
        "{}{text}",
        if o.status.success() {
            ""
        } else {
            "(the program failed)\n"
        }
    ))
}

fn said<T>(r: anyhow::Result<T>) -> Result<T, String> {
    r.map_err(|e| format!("{e:#}"))
}

impl Door {
    fn phone(&self) -> Result<phone::Phone, String> {
        said(phone::Phone::find(&self.device))
    }
}

#[tool_router]
impl Door {
    pub fn new(device: Device) -> Self {
        Door {
            device,
            tool_router: Self::tool_router(),
        }
    }

    /// Run commands on the phone, as its user or as root; what they printed, and whether they succeeded.
    #[tool]
    async fn run(&self, Parameters(r): Parameters<Run>) -> Result<String, String> {
        let _held = if r.root {
            Some(said(crate::hold::hold(&self.device))?)
        } else {
            None
        };
        let p = self.phone()?;
        let (ok, out) = said(if r.root {
            p.root(&r.script)
        } else {
            p.sh(&r.script)
        })?;
        Ok(format!(
            "{}{out}",
            if ok { "" } else { "(the commands failed)\n" }
        ))
    }

    /// Copy a file from the lab machine to the phone.
    #[tool]
    async fn put(&self, Parameters(a): Parameters<Put>) -> Result<String, String> {
        let _held = said(crate::hold::hold(&self.device))?;
        said(self.phone()?.put(&a.local, &a.remote))?;
        Ok(format!("put {} at {}", a.local, a.remote))
    }

    /// Copy a file from the phone (read as root) to the lab machine.
    #[tool]
    async fn get(&self, Parameters(a): Parameters<Get>) -> Result<String, String> {
        let bytes = said(
            self.phone()?
                .root_bytes(&format!("cat '{}'\n", a.remote.replace('\'', "'\\''"))),
        )?;
        said(std::fs::write(&a.local, &bytes).map_err(anyhow::Error::from))?;
        Ok(format!(
            "{} bytes from {} at {}",
            bytes.len(),
            a.remote,
            a.local
        ))
    }

    /// The phone's health as its latest report says it (written after every boot by the phone itself).
    #[tool]
    async fn health_report(&self) -> Result<String, String> {
        if let Some(said) = suite::not_applicable(&self.device) {
            return Ok(said);
        }
        let report = said(suite::latest(&self.phone()?))?;
        let rows = suite::rows(&report);
        if rows.is_empty() {
            return Ok(
                "no report on the phone yet - is the suite on it? (lpoco-lab suite put now)".into(),
            );
        }
        let (_, verdict) = said(
            self.phone()?
                .root("journalctl -t lpoco-health -o cat -n 1 --no-pager\n"),
        )?;
        Ok(format!(
            "{}the phone says: {}",
            suite::said(&rows),
            verdict.trim()
        ))
    }

    /// The phone's tests with their switches (off, with reason, who and when) and the conditions it finds
    /// out about itself right now (no SIM card, no network, no modem).
    #[tool]
    async fn health_switches(&self) -> Result<String, String> {
        if let Some(said) = suite::not_applicable(&self.device) {
            return Ok(said);
        }
        said(suite::list(&self.phone()?))
    }

    /// Switch a test off on the phone, with a reason: the phone's own suite keeps the switch, and reports the
    /// test as not measured until it is switched on. Not for what the phone finds out by itself (no SIM).
    #[tool]
    async fn health_switch_off(
        &self,
        Parameters(a): Parameters<SwitchOff>,
    ) -> Result<String, String> {
        if let Some(said) = suite::not_applicable(&self.device) {
            return Err(said);
        }
        said(suite::off(
            &self.phone()?,
            &a.test,
            &a.reason,
            a.by.as_deref().unwrap_or("devlab"),
        ))
    }

    /// Switch a test on again on the phone.
    #[tool]
    async fn health_switch_on(
        &self,
        Parameters(a): Parameters<SwitchOn>,
    ) -> Result<String, String> {
        if let Some(said) = suite::not_applicable(&self.device) {
            return Err(said);
        }
        said(suite::on(&self.phone()?, &a.test))
    }

    /// Measure the phone's health now (its own suite, on the phone): every test, and the phone's verdict.
    #[tool]
    async fn health_run(&self) -> Result<String, String> {
        if let Some(said) = suite::not_applicable(&self.device) {
            return Err(said); // the suite did not run: not a pass
        }
        let ran = said(suite::run(&self.phone()?))?;
        Ok(format!(
            "{}the phone says: {}",
            suite::said(&suite::rows(&ran.report)),
            ran.verdict
        ))
    }

    /// Install a test kernel as a second boot entry with one try; the stock kernel stays the default.
    #[tool]
    async fn kernel_install(
        &self,
        Parameters(k): Parameters<KernelInstall>,
    ) -> Result<String, String> {
        let _held = said(crate::hold::hold(&self.device))?;
        let p = self.phone()?;
        said(kernel::install(
            &p,
            &k.name,
            std::path::Path::new(&k.vmlinuz),
            std::path::Path::new(&k.dtb),
            std::path::Path::new(&k.modules),
            &k.cmdline,
        ))?;
        Ok(format!("installed pmos-{} with one try", k.name))
    }

    /// Boot an installed test kernel once (it falls back to stock by itself), say what came of it, and
    /// the health of the boot it came to. Takes minutes.
    #[tool]
    async fn kernel_try(&self, Parameters(k): Parameters<KernelName>) -> Result<String, String> {
        let _held = said(crate::hold::hold(&self.device))?;
        let came = said(outcome::try_and_report(
            &self.device,
            &self.phone()?,
            &k.name,
        ))?;
        let (_, health) = said(suite::after_change(&self.phone()?, true))?;
        Ok(format!("{came}\n{health}"))
    }

    /// Which kernel runs, which boot entries there are, and whether a timed-out try left a log.
    #[tool]
    async fn kernel_status(&self) -> Result<String, String> {
        let (_, out) = said(self.phone()?.root(
            "echo \"running: $(uname -r)\"; echo entries:; ls /boot/loader/entries/ | sed 's/^/  /'; \
             [ -f /boot/test-boot.log ] && echo 'a timed-out try left its log (lpoco-lab kernel log)'; true\n",
        ))?;
        Ok(out)
    }

    /// Reboot the phone, wait until it answers again, and say the health of the new boot. Takes minutes.
    #[tool]
    async fn reboot(&self) -> Result<String, String> {
        let _held = said(crate::hold::hold(&self.device))?;
        let back = said(phone::reboot(&self.device, &self.phone()?))?;
        let (_, health) = said(suite::after_change(&back, true))?;
        Ok(format!("back at {}\n{health}", back.target))
    }
    /// The facts of a boot image's header (lpoco-bringup image info), as a [boot] table.
    #[tool]
    async fn bringup_image_info(&self, Parameters(a): Parameters<Image>) -> Result<String, String> {
        family_run("lpoco-bringup", &["image", "info", &a.image])
    }

    /// Count a boot image against the device manifest's [boot] (lpoco-bringup image check): each fact ok,
    /// failed or not measured.
    #[tool]
    async fn bringup_image_check(
        &self,
        Parameters(a): Parameters<Image>,
    ) -> Result<String, String> {
        let mut args = vec!["image", "check", a.image.as_str()];
        if let Some(d) = a.device.as_deref() {
            args.extend(["--device", d]);
        }
        family_run("lpoco-bringup", &args)
    }
}

/// The family's tools under their program's name: devlab's own tools (run, put, kernel_try ...) are
/// the lab's, so they also answer as lab_run, lab_put, lab_kernel_try ...; the names they had before stay.
/// The health and bringup tools already carry their program's name.
fn lab_name(name: &str) -> Option<String> {
    const LAB: &[&str] = &[
        "run",
        "put",
        "get",
        "kernel_install",
        "kernel_try",
        "kernel_status",
        "reboot",
    ];
    LAB.contains(&name).then(|| format!("lab_{name}"))
}

#[tool_handler(
    router = self.tool_router,
    name = "lpoco",
    instructions = "One door for the lpoco family. lab_* (and the older bare names run, put, get, kernel_*, reboot): the phone in this lab, by its manifest (device.toml). health_*: its health suite. bringup_*: boot images, read and checked against the manifest. The phone in this lab, by its manifest (device.toml): run commands on it, move files, read and measure its health, try a test kernel once (it falls back to stock by itself), reboot and wait. A failure in a component the phone says is a known gap is not ill health: read the phone's verdict."
)]
impl ServerHandler for Door {
    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        let mut tools = self.tool_router.list_all();
        let extra: Vec<_> = tools
            .iter()
            .filter_map(|t| {
                lab_name(&t.name).map(|n| {
                    let mut c = t.clone();
                    c.name = n.into();
                    c
                })
            })
            .collect();
        tools.extend(extra);
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(rmcp::model::ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        mut request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        // lab_run is run: the one function behind both names
        if let Some(old) = request.name.strip_prefix("lab_") {
            if lab_name(old).is_some() {
                request.name = old.to_string().into();
            }
        }
        let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }
}

/// Serves the door on standard input and output until the agent goes.
pub fn serve(device: Device) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let service = Door::new(device).serve(rmcp::transport::stdio()).await?;
        service.waiting().await?;
        Ok(())
    })
}
