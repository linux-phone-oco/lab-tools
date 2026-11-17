//! lpoco-bringup: the host side of bringing a phone up (run as `lpoco bringup ...`). It makes files on the
//! host; devlab puts them on the phone. See cli.rs for the invocation surface.

mod cli;

use argenv_resolve::{InputResolveExt, OnProblems};
use cli::{Contract, Verb};
use lpoco_bringup::boot::Boot;
use lpoco_bringup::image::{self, Verdict};
use std::fs;
use std::path::Path;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let r = Contract::parse_and_lint(OnProblems::FailOnError);
    if Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-bringup {VERSION}");
        return;
    }
    let code = match run(&r) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("lpoco-bringup: {e}");
            2
        }
    };
    std::process::exit(code);
}

fn need(v: Option<String>, what: &str) -> Result<String, String> {
    v.ok_or_else(|| format!("{what} is needed"))
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn run(r: &argenv_resolve::Resolution) -> Result<i32, String> {
    let Some(verb) = Contract::VERB.get_from(r) else {
        return Err(
            "usage: lpoco bringup image info|unpack|pack|check ... (lpoco-bringup --version)"
                .into(),
        );
    };
    let device = Contract::DEVICE
        .get_from(r)
        .unwrap_or_else(|| "device.toml".into());
    match verb {
        Verb::Info => {
            let bytes = read(&need(Contract::IMAGE.get_from(r), "an image (IMG)")?)?;
            print!("{}", image::read(&bytes)?.info());
            Ok(0)
        }
        Verb::Unpack => {
            let bytes = read(&need(Contract::IMAGE.get_from(r), "an image (IMG)")?)?;
            let out = need(Contract::OUT.get_from(r), "--out DIR")?;
            let img = image::read(&bytes)?;
            fs::create_dir_all(&out).map_err(|e| format!("cannot make {out}: {e}"))?;
            let put = |name: &str, data: &[u8]| {
                fs::write(Path::new(&out).join(name), data)
                    .map_err(|e| format!("cannot write {out}/{name}: {e}"))
            };
            put("kernel", img.kernel)?;
            put("ramdisk", img.ramdisk)?;
            if !img.second.is_empty() {
                put("second", img.second)?;
            }
            if let Some(dtb) = img.dtb() {
                put("appended.dtb", dtb)?;
            }
            put("boot.toml", img.info().as_bytes())?;
            Ok(0)
        }
        Verb::Pack => {
            let boot = Boot::read(Path::new(&device))?;
            let mut kernel = read(&need(Contract::KERNEL.get_from(r), "--kernel FILE")?)?;
            if let Some(dtb) = Contract::DTB.get_from(r) {
                kernel.extend(read(&dtb)?);
            }
            let ramdisk = read(&need(Contract::RAMDISK.get_from(r), "--ramdisk FILE")?)?;
            let out = need(Contract::OUT.get_from(r), "--out IMG")?;
            let img = image::pack(&boot, &kernel, &ramdisk)?;
            if let Some(max) = boot.partition_size {
                if img.len() as u64 > max {
                    return Err(format!(
                        "the image is {} bytes; the boot partition holds {max}",
                        img.len()
                    ));
                }
            }
            fs::write(&out, &img).map_err(|e| format!("cannot write {out}: {e}"))?;
            Ok(0)
        }
        Verb::Check => {
            let path = need(Contract::IMAGE.get_from(r), "an image (IMG)")?;
            let bytes = read(&path)?;
            let boot = Boot::read(Path::new(&device))?;
            let findings = image::check(&bytes, &boot);
            let (mut ok, mut fail) = (0, 0);
            for f in &findings {
                println!("{}", f.line());
                match f.verdict {
                    Verdict::Ok(_) => ok += 1,
                    Verdict::Fail(_) => fail += 1,
                    Verdict::NotMeasured(_) => {}
                }
            }
            println!("{path}: {ok} ok, {fail} failed");
            Ok(if fail == 0 { 0 } else { 1 })
        }
    }
}
