//! ffs-acm (Rust): a serial console for a phone through FunctionFS. The same steps as the C probe
//! build/ffs-acm/ffs-acm.c (walleye decision 2026-11-17-walleye-ffs-acm-single-interface; moved here from google-walleye-lab build/ffs-acm-rs): one interface class 2/2/1 with
//! interrupt-in, bulk-out, bulk-in and no CDC functional descriptors; descriptors then strings go to ep0 first, only then
//! the UDC name is written; no function attribute is ever written; a pty shell is bridged to the bulk endpoints by one
//! forked pump per direction. No arguments and no environment are read (every path is a constant), so no argenv.
//! Needs in the ramdisk: /dev/ffs (functionfs mounted), /dev/ptmx and /dev/pts, /bin/sh, /dev/kmsg, configfs gadget g1.
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::process::exit;

const FFS_DIR: &str = "/dev/ffs";
const UDC_LIST: &str = "/sys/class/udc";
const UDC_FILE: &str = "/sys/kernel/config/usb_gadget/g1/UDC";

const FFS_DESC_MAGIC_V2: u32 = 3;
const FFS_STR_MAGIC: u32 = 2;
const FFS_HAS_FS: u32 = 1;
const FFS_HAS_HS: u32 = 2;
const FFS_HAS_SS: u32 = 4;

const EV_BIND: u8 = 0;
const EV_UNBIND: u8 = 1;
const EV_ENABLE: u8 = 2;
const EV_DISABLE: u8 = 3;
const EV_SETUP: u8 = 4;
/// struct usb_functionfs_event: setup packet (8 bytes), type, three pad bytes.
const EV_SIZE: usize = 12;

fn say(kmsg: &mut Option<File>, msg: &str) {
    if let Some(k) = kmsg {
        let _ = k.write_all(format!("ffs-acm: {msg}\n").as_bytes());
    }
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// Little-endian blob builder.
#[derive(Default)]
struct Blob(Vec<u8>);

impl Blob {
    fn p8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn p16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn p32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn iface(&mut self) {
        for b in [9, 4, 0, 0, 3, 2, 2, 1, 1] {
            self.p8(b);
        }
    }
    fn ep(&mut self, addr: u8, attr: u8, max: u16, interval: u8) {
        for b in [7, 5, addr, attr] {
            self.p8(b);
        }
        self.p16(max);
        self.p8(interval);
    }
    fn comp(&mut self, per: u16) {
        for b in [6, 0x30, 0, 0] {
            self.p8(b);
        }
        self.p16(per);
    }
    /// Patch the total length into bytes 4..8.
    fn finish(mut self) -> Vec<u8> {
        let len = self.0.len() as u32;
        self.0[4..8].copy_from_slice(&len.to_le_bytes());
        self.0
    }
}

fn descriptors() -> Vec<u8> {
    let mut b = Blob::default();
    b.p32(FFS_DESC_MAGIC_V2);
    b.p32(0); // length, patched
    b.p32(FFS_HAS_FS | FFS_HAS_HS | FFS_HAS_SS);
    b.p32(4);
    b.p32(4);
    b.p32(7); // descriptor counts: fs, hs, ss
    b.iface(); // full speed
    b.ep(0x81, 3, 10, 9);
    b.ep(0x02, 2, 64, 0);
    b.ep(0x83, 2, 64, 0);
    b.iface(); // high speed
    b.ep(0x81, 3, 10, 9);
    b.ep(0x02, 2, 512, 0);
    b.ep(0x83, 2, 512, 0);
    b.iface(); // super speed
    b.ep(0x81, 3, 10, 9);
    b.comp(10);
    b.ep(0x02, 2, 1024, 0);
    b.comp(0);
    b.ep(0x83, 2, 1024, 0);
    b.comp(0);
    b.finish()
}

fn strings() -> Vec<u8> {
    let mut b = Blob::default();
    b.p32(FFS_STR_MAGIC);
    b.p32(0); // length, patched
    b.p32(1); // one string
    b.p32(1); // one language
    b.p16(0x0409);
    for c in b"walleye console\0" {
        b.p8(*c);
    }
    b.finish()
}

/// Open a pty, start /bin/sh on its slave in its own session; returns the master and the shell's pid.
fn start_shell(kmsg: &mut Option<File>) -> Option<(OwnedFd, libc::pid_t)> {
    // SAFETY: plain libc calls on descriptors and strings owned here; the child only calls async-signal-safe
    // functions before exec (this process is single-threaded).
    unsafe {
        let m = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
        if m < 0 || libc::grantpt(m) < 0 || libc::unlockpt(m) < 0 {
            say(kmsg, &format!("pty failed errno {}", errno()));
            return None;
        }
        let master = OwnedFd::from_raw_fd(m);
        let mut buf = [0 as libc::c_char; 128];
        if libc::ptsname_r(m, buf.as_mut_ptr(), buf.len()) != 0 {
            say(kmsg, &format!("ptsname failed errno {}", errno()));
            return None;
        }
        let name = std::ffi::CStr::from_ptr(buf.as_ptr()).to_owned();
        let sh = CString::new("/bin/sh").unwrap();
        let argv = [sh.as_ptr(), std::ptr::null()];
        let path = CString::new("PATH=/bin:/sbin:/usr/bin:/usr/sbin").unwrap();
        let term = CString::new("TERM=dumb").unwrap();
        let envp = [path.as_ptr(), term.as_ptr(), std::ptr::null()];
        let pid = libc::fork();
        if pid == 0 {
            libc::setsid();
            let s = libc::open(name.as_ptr(), libc::O_RDWR);
            if s < 0 {
                libc::_exit(127);
            }
            libc::ioctl(s, libc::TIOCSCTTY, 0);
            libc::dup2(s, 0);
            libc::dup2(s, 1);
            libc::dup2(s, 2);
            if s > 2 {
                libc::close(s);
            }
            libc::close(m);
            // an ignored signal survives execve: the shell gets SIGPIPE back
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
            libc::execve(sh.as_ptr(), argv.as_ptr(), envp.as_ptr());
            libc::_exit(127);
        }
        say(
            kmsg,
            &format!("shell pid {} on {}", pid, name.to_string_lossy()),
        );
        if pid < 0 {
            return None;
        }
        Some((master, pid))
    }
}

/// One forwarding child per direction: a blocked bulk-in write (no host reading) never blocks the other direction.
fn pump(from: RawFd, to: RawFd) -> libc::pid_t {
    // SAFETY: fork in a single-threaded process; the child only reads, writes and _exits.
    unsafe {
        let p = libc::fork();
        if p != 0 {
            return p;
        }
        let mut b = [0u8; 1024];
        loop {
            let n = libc::read(from, b.as_mut_ptr().cast(), b.len());
            if n < 0 && errno() == libc::EINTR {
                continue;
            }
            if n <= 0 {
                libc::_exit(0);
            }
            let mut o = 0isize;
            while o < n {
                let w = libc::write(to, b.as_ptr().offset(o).cast(), (n - o) as usize);
                if w < 0 && errno() == libc::EINTR {
                    continue;
                }
                if w <= 0 {
                    libc::_exit(0);
                }
                o += w;
            }
        }
    }
}

#[derive(Default)]
struct Bridge {
    rd: libc::pid_t,
    wr: libc::pid_t,
    eps: Vec<File>,
}

impl Bridge {
    fn stop(&mut self) {
        // SAFETY: signalling and reaping our own children.
        unsafe {
            for p in [self.rd, self.wr] {
                if p > 0 {
                    libc::kill(p, libc::SIGKILL);
                }
            }
            // the two pumps by pid (they were just killed), never the shell
            for p in [self.rd, self.wr] {
                if p > 0 {
                    libc::waitpid(p, std::ptr::null_mut(), 0);
                }
            }
        }
        self.rd = 0;
        self.wr = 0;
        self.eps.clear();
    }

    fn start(&mut self, kmsg: &mut Option<File>, master: RawFd) {
        self.stop();
        let open = |n: &str, w: bool| {
            OpenOptions::new()
                .read(!w)
                .write(w)
                .open(format!("{FFS_DIR}/{n}"))
        };
        let (e1, e2, e3) = match (open("ep1", true), open("ep2", false), open("ep3", true)) {
            (Ok(a), Ok(b), Ok(c)) => (a, b, c),
            _ => {
                say(kmsg, &format!("opening ep1..ep3 failed errno {}", errno()));
                return;
            }
        };
        self.rd = pump(e2.as_raw_fd(), master);
        self.wr = pump(master, e3.as_raw_fd());
        self.eps = vec![e1, e2, e3];
        say(
            kmsg,
            &format!(
                "bridge up (host to pty pid {}, pty to host pid {})",
                self.rd, self.wr
            ),
        );
    }
}

fn setup(kmsg: &mut Option<File>, ep0: &mut File, ev: &[u8]) {
    let rt = ev[0];
    let req = ev[1];
    let len = u16::from_le_bytes([ev[6], ev[7]]) as usize;
    let input = rt & 0x80 != 0;
    let mut b = [0u8; 64];
    if rt & 0x7f == 0x21 && req == 0x20 && !input {
        // SET_LINE_CODING: take the data
        if ep0.read(&mut b[..len.min(64)]).is_err() {
            say(kmsg, &format!("SET_LINE_CODING errno {}", errno()));
        }
    } else if rt == 0xa1 && req == 0x21 {
        // GET_LINE_CODING: 115200 8N1
        let lc = [0x00u8, 0xc2, 0x01, 0x00, 0, 0, 8];
        if ep0.write(&lc[..len.min(7)]).is_err() {
            say(kmsg, &format!("GET_LINE_CODING errno {}", errno()));
        }
    } else if rt == 0x21 && req == 0x22 {
        // SET_CONTROL_LINE_STATE: ack
        if ep0.read(&mut b[..0]).is_err() {
            say(kmsg, &format!("SET_CONTROL_LINE_STATE errno {}", errno()));
        }
    } else {
        // anything else: stall by the wrong-direction call
        say(kmsg, &format!("setup {rt:02x} {req:02x} stalled"));
        if input {
            let _ = ep0.read(&mut b[..0]);
        } else {
            let _ = ep0.write(&b[..0]);
        }
    }
}

fn main() {
    let mut kmsg = OpenOptions::new().write(true).open("/dev/kmsg").ok();
    // SAFETY: ignoring SIGPIPE touches no memory.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
    say(&mut kmsg, "start");
    let mut ep0 = match OpenOptions::new()
        .read(true)
        .write(true)
        .open(format!("{FFS_DIR}/ep0"))
    {
        Ok(f) => f,
        Err(_) => {
            say(&mut kmsg, &format!("open ep0 failed errno {}", errno()));
            exit(1);
        }
    };
    for blob in [descriptors(), strings()] {
        if ep0.write(&blob).ok() != Some(blob.len()) {
            say(
                &mut kmsg,
                &format!("descriptors write failed errno {}", errno()),
            );
            exit(1);
        }
    }
    say(&mut kmsg, "descriptors and strings written");
    let Some((master, _shell)) = start_shell(&mut kmsg) else {
        exit(1)
    };
    // now the bind
    let udc = fs::read_dir(UDC_LIST)
        .ok()
        .and_then(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .find(|n| !n.starts_with('.'))
        })
        .unwrap_or_default();
    if udc.is_empty() {
        say(&mut kmsg, "NO UDC");
        exit(1);
    }
    say(&mut kmsg, &format!("about to bind {udc}"));
    let bound = OpenOptions::new()
        .write(true)
        .open(UDC_FILE)
        .and_then(|mut u| u.write_all(udc.as_bytes()));
    if bound.is_err() {
        say(&mut kmsg, &format!("UDC bind failed errno {}", errno()));
        exit(1);
    }
    say(&mut kmsg, &format!("gadget bound to {udc}"));
    let mut bridge = Bridge::default();
    let mut buf = [0u8; EV_SIZE * 4];
    loop {
        let r = match ep0.read(&mut buf) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                say(&mut kmsg, &format!("ep0 read errno {}", errno()));
                break;
            }
        };
        for ev in buf[..r].as_chunks::<EV_SIZE>().0 {
            match ev[8] {
                EV_ENABLE => {
                    say(&mut kmsg, "enable");
                    bridge.start(&mut kmsg, master.as_raw_fd());
                }
                EV_DISABLE => {
                    say(&mut kmsg, "disable");
                    bridge.stop();
                }
                EV_UNBIND => {
                    say(&mut kmsg, "unbind");
                    bridge.stop();
                }
                EV_BIND => say(&mut kmsg, "bind"),
                EV_SETUP => setup(&mut kmsg, &mut ep0, ev),
                _ => {}
            }
        }
    }
    exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The descriptors and strings are the C probe's, byte for byte (tests/c-probe-blobs.bin was written by ffs-acm.c).
    #[test]
    fn blobs_are_the_c_probes() {
        let mut v = descriptors();
        v.extend(strings());
        assert_eq!(v, include_bytes!("../tests/c-probe-blobs.bin"));
    }
}
