#!/bin/sh
# check-sxmo-utils-apk.sh - the sxmo-utils lab apk (packaging/sxmo-utils) is what decision
# 2026-11-18-upstream-forks says: Alpine's APKBUILD with every subpackage, its source our fork's
# lab tag, a version above Alpine's own (so `apk upgrade` keeps it), every checksum true.
# Set OFFLINE=1 to skip the one download (the tag's tarball and what it carries).
set -eu
d=packaging/sxmo-utils
fail=0
bad() { echo "FAIL: $*" >&2; fail=1; }

[ -f "$d/APKBUILD" ] || { echo "FAIL: $d/APKBUILD is missing" >&2; exit 1; }
sh -n "$d/APKBUILD" || bad "APKBUILD does not parse"

# shellcheck disable=SC1090
( CARCH=aarch64; srcdir=/nonexistent; . "./$d/APKBUILD"
  [ "$pkgname" = sxmo-utils ] || bad "pkgname is $pkgname"
  case "$pkgver" in 1.18.1_p20[0-9][0-9][0-9][0-9][0-9][0-9]) ;; *) bad "pkgver $pkgver is not 1.18.1_pYYYYMMDD" ;; esac
  case "$source" in *github.com/grenudi/sxmo-utils/archive/refs/tags/lab-*) ;; *) bad "source is not a grenudi lab tag" ;; esac
  # every subpackage Alpine ships, so -common's exact-version depends hold
  for s in doc common::noarch x11::noarch wayland::noarch dwm::noarch i3::noarch sway::noarch \
    modem:modem:noarch modem-mms:modem_mms:noarch modem-vvm:modem_vvm:noarch \
    audio-pipewire:audio_pipewire:noarch audio-pulseaudio:audio_pulseaudio:noarch \
    audio-bluetooth:audio_bluetooth:noarch audio-bluetooth-pipewire:audio_bluetooth_pipewire:noarch \
    audio-bluetooth-pulseaudio:audio_bluetooth_pulseaudio:noarch udev; do
    sp=$(echo $subpackages)
    case " $sp " in *" $pkgname-$s "*|*"$pkgname-$s "*) ;; *) bad "subpackage $s is missing" ;; esac
  done
  # the checksums of the files beside it
  for f in $source; do
    case "$f" in *::*|*://*) continue ;; esac
    want=$(printf '%s\n' "$sha512sums" | sed -n "s/^\([0-9a-f]*\)  $f\$/\1/p")
    have=$(sha512sum "$d/$f" | cut -d' ' -f1)
    [ -n "$want" ] && [ "$want" = "$have" ] || bad "checksum of $f"
  done
  for f in $install; do
    case "$f" in *river*) continue ;; esac
    f=$(printf '%s' "$f" | sed "s/\$pkgname/$pkgname/")
    [ -f "$d/$f" ] || bad "install script $f is missing"
  done
  if [ -z "${OFFLINE:-}" ]; then
    t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
    url=$(echo "$source" | tr -s " \t\n" "\n\n\n" | sed -n "s/^.*:://p;" | head -n1)
    curl -sfL "$url" -o "$t/src.tar.gz" || bad "cannot fetch $url"
    want=$(printf '%s\n' "$sha512sums" | sed -n "s/^\([0-9a-f]*\)  $pkgname-$pkgver.tar.gz\$/\1/p")
    [ "$want" = "$(sha512sum "$t/src.tar.gz" | cut -d' ' -f1)" ] || bad "checksum of the lab tag's tarball"
    tar -xzf "$t/src.tar.gz" -C "$t"
    grep -rq SXMO_SUSPEND_RETRY_MAX "$t"/sxmo-utils-*/scripts || bad "the tag lacks the suspend backoff (SXMO_SUSPEND_RETRY_MAX)"
  fi
  exit $fail
) || fail=1
[ "$fail" = 0 ] && echo "ok: the sxmo-utils lab apk declaration holds"
exit $fail
