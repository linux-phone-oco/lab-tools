#!/bin/busybox sh
# The bring-up initramfs, made by `lpoco-bringup initramfs init` from the device's [boot.gadget]: no storage, no
# display. It raises a USB gadget (ACM serial console + ECM network, and RNDIS in a second configuration for
# Windows hosts), gives the phone 172.16.42.1, hands out 172.16.42.2 by DHCP and answers on telnet (no password:
# the phone sits on a cable in the lab, and this is a try held in memory - never to be left in an image that is
# flashed). The shape follows postmarketOS's initramfs (172.16.42.1, telnet debug shell); the code is ours.
#
# Self-test (bringup.selftest=1 on the kernel command line, x86_64 QEMU only): the kernel's dummy_hcd makes a
# host side for the gadget, and the init reaches the phone's address through it (/selftest.sh).
/bin/busybox --install -s /bin
export PATH=/bin:/sbin:/usr/bin:/usr/sbin

say() { echo "@LABEL@: $*"; echo "@LABEL@: $*" > /dev/kmsg 2>/dev/null; }

mount -t proc proc /proc
mount -t sysfs sys /sys
mount -t devtmpfs dev /dev 2>/dev/null || { mount -t tmpfs dev /dev; mdev -s; }
mkdir -p /dev/pts /sys/kernel/config /sys/fs/pstore /tmp /run
mount -t devpts devpts /dev/pts
mount -t configfs none /sys/kernel/config
mount -t pstore pstore /sys/fs/pstore 2>/dev/null
exec > /dev/console 2>&1 < /dev/console
say "up, kernel $(uname -r)"

SELFTEST=0
grep -q 'bringup.selftest=1' /proc/cmdline && SELFTEST=1

# what the last boot left in pstore (a crash, a warm reset): name it, the shell can read it
for f in /sys/fs/pstore/*; do [ -f "$f" ] && say "pstore: $f"; done

if [ -f /modules.list ]; then      # the self-test kernel has the gadget stack as modules
    while read -r m; do insmod "$m" || say "insmod $m failed"; done < /modules.list
fi

G=/sys/kernel/config/usb_gadget/g1
mkdir -p $G
echo 0x1d6b > $G/idVendor          # Linux Foundation
echo 0x0104 > $G/idProduct         # Multifunction Composite Gadget
echo 0x0100 > $G/bcdDevice
echo 0x0200 > $G/bcdUSB
mkdir -p $G/strings/0x409
echo "@SERIAL@" > $G/strings/0x409/serialnumber
echo "@MANUFACTURER@" > $G/strings/0x409/manufacturer
echo "@PRODUCT@" > $G/strings/0x409/product
mkdir -p $G/functions/@CONSOLE@ $G/functions/ecm.usb0
echo "02:42:ac:10:2a:01" > $G/functions/ecm.usb0/dev_addr
echo "02:42:ac:10:2a:02" > $G/functions/ecm.usb0/host_addr
mkdir -p $G/configs/c.1/strings/0x409
echo "@CONSOLENAME@ + ECM" > $G/configs/c.1/strings/0x409/configuration
ln -s $G/functions/@CONSOLE@ $G/configs/c.1/
ln -s $G/functions/ecm.usb0 $G/configs/c.1/
@RNDIS@
@WINDOW@
@WINDOWLOOP@
# the controller may probe late (clocks, regulators, the PHY): wait for it, up to 60 s
n=0
while [ -z "$(ls /sys/class/udc 2>/dev/null)" ] && [ $n -lt 60 ]; do sleep 1; n=$((n+1)); done
UDC=$(ls /sys/class/udc 2>/dev/null | head -n1)
@BIND@

ifconfig lo up
n=0
while ! ifconfig usb0 >/dev/null 2>&1 && [ $n -lt 20 ]; do sleep 1; n=$((n+1)); done
ifconfig usb0 172.16.42.1 netmask 255.255.255.0 up && say "usb0 is 172.16.42.1" || say "usb0 did not come up"
udhcpd -S /etc/udhcpd.conf
telnetd -l /bin/sh -b 172.16.42.1
say "telnetd on 172.16.42.1:23"

if [ $SELFTEST = 1 ]; then
    sh /selftest.sh
    poweroff -f
fi

@SHELL@
say "ready"
while :; do sleep 3600; done
