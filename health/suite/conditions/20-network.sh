# no-network: no interface carries a default route (neither Wi-Fi, USB networking nor mobile data)
if ! { ip route; ip -6 route; } 2>/dev/null | grep -q '^default '; then
	echo "no-network no default route: nothing to reach the internet with"
fi
