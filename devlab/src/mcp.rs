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
pub struct KernelName {
    /// The test kernel entry's name, as installed
    pub name: String,
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
        let report = said(suite::latest(&self.phone()?))?;
        let rows = suite::rows(&report);
        if rows.is_empty() {
            return Ok(
                "no report on the phone yet - is the suite on it? (devlab suite put now)".into(),
            );
        }
        let (_, verdict) = said(
            self.phone()?
                .root("journalctl -t devlab-health -o cat -n 1 --no-pager\n"),
        )?;
        Ok(format!(
            "{}the phone says: {}",
            suite::said(&rows),
            verdict.trim()
        ))
    }

    /// Measure the phone's health now (its own suite, on the phone): every test, and the phone's verdict.
    #[tool]
    async fn health_run(&self) -> Result<String, String> {
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
             [ -f /boot/test-boot.log ] && echo 'a timed-out try left its log (devlab kernel log)'; true\n",
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
}

#[tool_handler(
    router = self.tool_router,
    name = "devlab",
    instructions = "The phone in this lab, by its manifest (device.toml): run commands on it, move files, read and measure its health, try a test kernel once (it falls back to stock by itself), reboot and wait. A failure in a component the phone says is a known gap is not ill health: read the phone's verdict."
)]
impl ServerHandler for Door {}

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
