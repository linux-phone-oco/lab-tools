// SPDX-License-Identifier: MIT OR Apache-2.0
//! The proof of layer-firmware-rule-plain-alpine, in an alpine:3.24 container: the real apk is installed, a fake
//! vendor partition (an ext4 image on a loop device, linked as /dev/disk/by-partlabel/vendor) is mounted read-only
//! by the real OpenRC service, and the package's links make the declared files reachable under /lib/firmware while an undeclared
//! one is not. Needs docker (--privileged, for the loop device), the network and nfpm, so it is ignored until asked:
//!
//!   cargo test -p lpoco-layer --test firmware_containers -- --ignored

use std::fs;
use std::process::Command;

const DEVICE: &str = r#"
[layer]
name = "xiaomi-surya"

[layer.firmware]
method = "extract-from-partitions"
compatible = "xiaomi,surya"

[[layer.firmware.source]]
partition = "vendor"
origin = "firmware"
destination = "qcom/sm7150/xiaomi/surya"
files = [{ name = "a615_zap.mdt" }, { name = "wlanmdsp.mbn", rename = "wlanmdsp.otaupdate" }]
"#;

fn run(cmd: &mut Command) -> String {
    let o = cmd.output().unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.status.success(), "{cmd:?}:\n{all}");
    all
}

#[test]
#[ignore = "needs docker (privileged), the network and nfpm"]
fn the_openrc_service_links_the_declared_files_from_a_fake_partition_and_no_others() {
    let dir = std::env::temp_dir().join(format!("lpoco-fw-ct-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("device.toml"), DEVICE).unwrap();
    run(Command::new(env!("CARGO_BIN_EXE_lpoco-layer"))
        .args(["pack", "--format", "apk", "--manifest"])
        .arg(dir.join("device.toml"))
        .arg("--out")
        .arg(dir.join("out"))
        .env("SOURCE_DATE_EPOCH", "1700000000"));
    let script = "apk add -q e2fsprogs openrc >/dev/null\n\
         truncate -s 8M /p.img && mkfs.ext4 -q /p.img\n\
         mkdir /m && mount -o loop /p.img /m\n\
         mkdir /m/firmware && echo zap > /m/firmware/a615_zap.mdt && echo wlan > /m/firmware/wlanmdsp.mbn && echo no > /m/firmware/secret.bin\n\
         umount /m\n\
         dev=$(losetup -f) && losetup $dev /p.img\n\
         trap 'umount /run/xiaomi-surya-firmware/vendor 2>/dev/null; losetup -d $dev' EXIT\n\
         mkdir -p /dev/disk/by-partlabel && ln -s $dev /dev/disk/by-partlabel/vendor\n\
         apk add -q --allow-untrusted /w/out/*.apk\n\
         rc-update show sysinit | grep -q xiaomi-surya-firmware\n\
         mkdir -p /run/openrc && touch /run/openrc/softlevel\n\
         rc-service -D xiaomi-surya-firmware start\n\
         fw=/lib/firmware/qcom/sm7150/xiaomi/surya\n\
         test \"$(cat $fw/a615_zap.mdt)\" = zap\n\
         test \"$(cat $fw/wlanmdsp.otaupdate)\" = wlan\n\
         test ! -e $fw/secret.bin\n\
         test ! -e $fw/wlanmdsp.mbn\n\
         if touch /run/xiaomi-surya-firmware/vendor/new 2>/dev/null; then echo 'the partition is writable' >&2; exit 1; fi\n\
         rc-service -D xiaomi-surya-firmware restart\n\
         test \"$(cat $fw/a615_zap.mdt)\" = zap";
    run(Command::new("docker")
        .args(["run", "--rm", "--privileged", "-v"])
        .arg(format!("{}:/w", dir.display()))
        .args(["alpine:3.24", "sh", "-exc", script]));
}
