# Wi-Fi: an interface, joined with an address, and the gateway answers
. /usr/lib/devlab/health/lib.sh
absent wifi && { say wifi-interface wifi skip "the manifest says this phone has no Wi-Fi"; exit 0; }
w=$(ls /sys/class/net | grep -E '^wl' | head -n1)
check wifi-interface wifi "$([ -n "$w" ] && echo true)" "interface ${w:-none}"
[ -n "$w" ] || exit 0
st=$(cat "/sys/class/net/$w/operstate" 2>/dev/null); ad=$(ip -4 addr show "$w" 2>/dev/null | awk '/inet /{print $2; exit}')
if [ "$st" = up ] && [ -n "$ad" ]; then say wifi-connected wifi pass "$w up, address $ad"
elif [ ! -s /etc/NetworkManager/system-connections/* ] 2>/dev/null && ! ls /etc/NetworkManager/system-connections/* >/dev/null 2>&1; then
	say wifi-connected wifi skip "no network is configured on the phone"
else say wifi-connected wifi fail "$w ${st:-absent}, address ${ad:-none}"; fi
gw=$(ip route 2>/dev/null | awk -v w="$w" '/^default/ && $0 ~ w {print $3; exit}')
if [ -n "$gw" ]; then check wifi-reaches-gateway wifi "$(ping -c 1 -W 2 "$gw" >/dev/null 2>&1 && echo true)" "gateway $gw"
else say wifi-reaches-gateway wifi skip "no route through $w"; fi
