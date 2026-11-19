# SPDX-License-Identifier: MIT OR Apache-2.0
# The hardware-test layer is quiet: a phone that booted it (the image at multi-user.target, no phone shell -
# decision 2026-11-19-hardware-test-layer) runs no shell process, so no shell's own sleep, sound or session
# can disturb a hardware test. The runner says which layer the phone booted (LPOCO_BOOTED: test, shell or
# unknown, read from /proc/cmdline and the shell that runs); a shell boot is another layer's to test, so it
# is not measured here, and an unknown boot is not known yet - never failing. Reads only.
# A shell process is found by its command name (comm, cut at 15 characters; a script run as `sh x.sh` is
# `sh`): the names are the shells main.rs's shell_running knows, as processes. A device whose shell differs
# adds its names here when it needs them.
. "${LPOCO_HEALTH_LIB:-/usr/lib/lpoco-health/lib.sh}"
root="${LPOCO_HEALTH_ROOT:-}"

found=
for c in "$root"/proc/[0-9]*/comm; do
	read -r n < "$c" 2>/dev/null || continue
	case "$n" in
	sxmo* | tinydm | phosh | gnome-shell | plasmashell | sway) in_list "$n" "$found" || found="$found $n" ;;
	esac
done
found=${found# }

case "${LPOCO_BOOTED:-unknown}" in
test)
	if [ -z "$found" ]; then
		say test-layer-quiet test-layer pass "test layer booted: no shell process runs"
	else
		say test-layer-quiet test-layer fail "test layer booted but a shell runs: $found"
	fi
	;;
shell)
	say test-layer-quiet test-layer skip "shell layer booted (running: ${found:-none seen}): the test layer's quiet is not measured"
	;;
*)
	say test-layer-quiet test-layer skip "which layer the phone booted is not known yet (running: ${found:-no shell seen})"
	;;
esac
