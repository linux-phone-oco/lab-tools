# the signal processors are up: each one the manifest expects, running (REMOTEPROCS: name:component)
. /usr/lib/devlab/health/lib.sh
[ -n "${REMOTEPROCS:-}" ] || { say remoteprocs kernel skip "the manifest names no signal processors"; exit 0; }
for pair in $REMOTEPROCS; do
	n=${pair%%:*}; c=${pair#*:}; st=""
	for r in /sys/class/remoteproc/remoteproc*; do
		[ "$(cat "$r/name" 2>/dev/null)" = "$n" ] && st=$(cat "$r/state")
	done
	check "$n-running" "$c" "$([ "$st" = running ] && echo true)" "$n: ${st:-not found}"
done
