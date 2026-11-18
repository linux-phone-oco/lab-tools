#!/bin/sh
# The host side of the gadget, in the same kernel (dummy_hcd): find the other usb interface, move it into a
# network namespace (else the kernel answers 172.16.42.1 from its own table), get a lease from the init's
# udhcpd, ping and speak telnet to 172.16.42.1. Prints SELFTEST lines; the last says PASS.
# Uses the host's iproute2 copied to /opt/ip (busybox's ip cannot move an interface between namespaces).
IP=/opt/ip/ip
fail() { echo "SELFTEST FAIL: $*"; exit 1; }
sleep 3
H=
for p in /sys/class/net/*; do
    i=${p##*/}
    case $i in lo|usb0) ;; *) [ -e /sys/class/net/$i/device ] && H=$i ;; esac
done
[ -n "$H" ] || { ls /sys/class/net; fail "no host-side usb network interface"; }
echo "SELFTEST host side is $H"
unshare -n sleep 600 &
NS=$!
sleep 1
[ "$(readlink /proc/$NS/ns/net)" != "$(readlink /proc/self/ns/net)" ] || fail "no separate network namespace"
INNS() { nsenter -t $NS -n -- "$@"; }
$IP link set "$H" netns $NS || fail "cannot move $H into a namespace"
INNS $IP link set lo up
INNS $IP link set "$H" up
INNS udhcpc -i "$H" -n -q -s /udhcpc.script 2>&1 | tail -2
INNS $IP addr show "$H" | grep -q "inet 172.16.42.2/" || fail "no lease"
echo "SELFTEST lease 172.16.42.2 on $H"
INNS ping -c 2 -W 3 172.16.42.1 >/dev/null || fail "no ping"
echo "SELFTEST ping 172.16.42.1 answers"
R=$( (echo "echo SELFTEST-telnet-says-\$((6*7))"; sleep 2; echo exit) | INNS nc -w 5 172.16.42.1 23 2>&1 | tr -d '\000-\010\013-\037\377' | grep -o 'SELFTEST-telnet-says-42' | tail -1)
[ -n "$R" ] || fail "telnet gave no answer"
echo "SELFTEST telnet answered: $R"
[ -c /dev/ttyGS0 ] && echo "SELFTEST ttyGS0 exists (ACM console)" || fail "no /dev/ttyGS0"
echo "SELFTEST PASS"
