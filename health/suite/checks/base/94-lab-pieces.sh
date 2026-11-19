# SPDX-License-Identifier: MIT OR Apache-2.0
# What lab pieces this phone holds, as facts the lab compares (lpoco host check, phone-pieces-drift-named):
# (and, further down, one row per declared user config)
# one row per file the manifest declares (PIECE_FILES in health.env: the flash overlays, provision installs
# and the phone's own checks, written by devlab suite put) with the file's git blob id, or "missing"; and one
# row per lab-tools package (PIECE_PACKAGES, lpoco-health when unsaid) with the version apk holds, or "missing".
# Reads only, and judges nothing: a row is a pass whatever it says - whether it is the pinned one is the lab's
# to say, from the pin. The blob id is git's (sha1 of "blob SIZE\0" and the bytes), which BusyBox can make.
. "${LPOCO_HEALTH_LIB:-/usr/lib/lpoco-health/lib.sh}"
root="${LPOCO_HEALTH_ROOT:-}"
set -f # a declared path is a word, never a pattern

blob_id() { { printf 'blob %s\0' "$(wc -c < "$1" | tr -d ' ')"; cat "$1"; } | sha1sum | cut -d' ' -f1; }

for f in ${PIECE_FILES:-}; do
	name="$f"
	if [ -f "$root$f" ]; then
		say "piece-file:$name" lab-pieces pass "$f $(blob_id "$root$f")"
	else
		say "piece-file:$name" lab-pieces pass "$f missing"
	fi
done

if ! installed=$(apk list --installed 2>/dev/null); then
	say piece-packages lab-pieces skip "apk list --installed failed: the packages are not known"
	installed=
	PIECE_PACKAGES=
fi
for p in ${PIECE_PACKAGES:-lpoco-health}; do
	# "NAME-VERSION-rN ARCH {ORIGIN} (LICENSE) [installed]": the version starts at the first "-DIGIT"
	line=$(printf '%s\n' "$installed" | awk -v p="$p" 'index($1, p "-") == 1 && substr($1, length(p) + 2) ~ /^[0-9][^-]*-r[0-9]+$/ { print $1; exit }')
	if [ -n "$line" ]; then
		say "piece-package-$p" lab-pieces pass "$p ${line#"$p"-}"
	else
		say "piece-package-$p" lab-pieces pass "$p missing"
	fi
done

# One row per user config the manifest declares (PIECE_CONFIGS: PATH, or PATH=DEFAULT when declared against the
# package default): `PATH BLOB DEFAULTBLOB` - the file's blob id or "missing", then the default's blob id, "missing",
# or "-" when the lab compares with the base's copy instead. Undeclared files are never read.
for c in ${PIECE_CONFIGS:-}; do
	f="${c%%=*}"
	dflt=-
	case "$c" in *=*)
		d="${c#*=}"
		if [ -f "$root$d" ]; then dflt=$(blob_id "$root$d"); else dflt=missing; fi
		;;
	esac
	if [ -f "$root$f" ]; then
		say "config-file:$f" lab-pieces pass "$f $(blob_id "$root$f") $dflt"
	else
		say "config-file:$f" lab-pieces pass "$f missing $dflt"
	fi
done
