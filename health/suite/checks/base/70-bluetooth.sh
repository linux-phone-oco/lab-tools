# SPDX-License-Identifier: MIT OR Apache-2.0
# Bluetooth: a controller, powered
. /usr/lib/lpoco-health/lib.sh
absent bluetooth && { say bt-controller bluetooth skip "the manifest says this phone has no Bluetooth"; exit 0; }
check bt-controller bluetooth "$([ -e /sys/class/bluetooth/hci0 ] && echo true)" "$(ls /sys/class/bluetooth 2>/dev/null | tr '\n' ' ')"
command -v bluetoothctl >/dev/null || { say bt-powered bluetooth skip "no bluetoothctl"; exit 0; }
pw=$(timeout 10 bluetoothctl show 2>/dev/null | awk '/Powered:/{print $2; exit}')
check bt-powered bluetooth "$([ "$pw" = yes ] && echo true)" "adapter powered: ${pw:-no adapter}"
