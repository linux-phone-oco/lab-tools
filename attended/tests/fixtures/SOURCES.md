# Captured text the install-step tests read

Text a phone printed, copied (not typed; the mounts file is an excerpt) from google-walleye-lab at its main:

- `slot-suffix-on-b.txt`: what `getprop ro.boot.slot_suffix` prints on stock Android on slot b; the same value is the line
  `[ro.boot.slot_suffix]: [_b]` of research/walleye-bootimage/2026-11-23/android-b-cmdline-and-boot-props.txt.
- `proc-mounts-lower-ro.txt`: the first four lines of the `/proc/mounts` read in tools/walleye-tries/fixtures/rootfs-boots-run3-read.txt
  (lines 9-12); `/dev/sda22 /lower ext4 ro,relatime,norecovery` is a read-only mount, `overlay /` a read-write one.
