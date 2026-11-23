// SPDX-License-Identifier: MIT OR Apache-2.0
//! The one step (lifted from google-walleye-lab tools/walleye-reboot): sync, then reboot(2) RESTART2 with the string given. The syscalls sit behind [`Kernel`] so a test
//! can stand in for them and see exactly what was passed.

use std::ffi::CString;

/// The strings this tool will hand to the kernel: the allow-list. The bootloader reads `bootloader` from the reboot reason; any other
/// string is refused before the kernel is asked, so a mistyped command cannot restart the phone into something unplanned.
pub const ALLOWED: [&str; 1] = ["bootloader"];

/// What the kernel is asked: flush the disks, then restart with a command.
pub trait Kernel {
    fn sync(&self);
    /// reboot(2) with LINUX_REBOOT_CMD_RESTART2; returns only when the call failed, with what it said.
    fn restart2(&self, command: &std::ffi::CStr) -> String;
}

/// The command as the kernel will read it, or why it cannot be passed.
pub fn command_string(command: &str) -> Result<CString, String> {
    if !ALLOWED.contains(&command) {
        return Err(format!(
            "the command {command:?} is not allowed: only {ALLOWED:?} is passed to reboot(2)"
        ));
    }
    CString::new(command).map_err(|_| "the command holds a NUL byte".to_string())
}

/// Sync, then restart with the command. Returns only on failure (a restart does not return).
pub fn restart_with(kernel: &dyn Kernel, command: &str) -> Result<(), String> {
    let c = command_string(command)?;
    kernel.sync();
    Err(format!(
        "reboot(RESTART2, {command:?}) returned: {}",
        kernel.restart2(&c)
    ))
}

/// The real kernel: libc's sync and the reboot syscall.
pub struct Real;

impl Kernel for Real {
    fn sync(&self) {
        // SAFETY: sync takes no arguments
        unsafe { libc::sync() };
    }

    fn restart2(&self, command: &std::ffi::CStr) -> String {
        // SAFETY: reboot(2) with RESTART2 reads the NUL-terminated string `command` points to, which outlives the call
        let rc = unsafe {
            libc::syscall(
                libc::SYS_reboot,
                libc::LINUX_REBOOT_MAGIC1,
                libc::LINUX_REBOOT_MAGIC2,
                libc::LINUX_REBOOT_CMD_RESTART2,
                command.as_ptr(),
            )
        };
        if rc == 0 {
            "returned 0".into()
        } else {
            std::io::Error::last_os_error().to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Stand {
        calls: RefCell<Vec<String>>,
    }

    impl Kernel for Stand {
        fn sync(&self) {
            self.calls.borrow_mut().push("sync".into());
        }
        fn restart2(&self, command: &std::ffi::CStr) -> String {
            self.calls
                .borrow_mut()
                .push(format!("restart2 {}", command.to_str().unwrap()));
            "stand-in".into()
        }
    }

    #[test]
    fn the_string_given_is_the_string_passed_after_a_sync() {
        let k = Stand::default();
        let e = restart_with(&k, "bootloader").unwrap_err();
        assert_eq!(*k.calls.borrow(), ["sync", "restart2 bootloader"]);
        assert!(e.contains("stand-in"), "{e}");
    }

    #[test]
    fn a_command_off_the_allow_list_calls_nothing() {
        for bad in [
            "",
            "a\0b",
            "recovery",
            "Bootloader",
            "bootloader ",
            "bootloader\0",
        ] {
            let k = Stand::default();
            assert!(restart_with(&k, bad).is_err(), "{bad:?}");
            assert!(k.calls.borrow().is_empty(), "{bad:?}");
        }
        assert!(command_string("bootloader").is_ok());
    }

    #[test]
    fn the_magic_numbers_are_the_kernels() {
        assert_eq!(libc::LINUX_REBOOT_MAGIC1 as u32, 0xfee1dead);
        assert_eq!(libc::LINUX_REBOOT_MAGIC2, 672274793);
        assert_eq!(libc::LINUX_REBOOT_CMD_RESTART2, 0xA1B2C3D4u32 as i32);
    }
}
