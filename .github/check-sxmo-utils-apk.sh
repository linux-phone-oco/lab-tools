#!/bin/sh
# check-sxmo-utils-apk.sh - the sxmo-utils lab apk (packaging/sxmo-utils) is what decision
# 2026-11-18-upstream-forks says: Alpine's APKBUILD with every subpackage, its source our fork's
# lab tag, a version above Alpine's own (so `apk upgrade` keeps it), every checksum true, the
# builddir the tag unpacks to. OFFLINE=1 skips the one download (the tag's tarball, its checksum
# and what it carries); only the online run proves those.
set -eu
cd "$(dirname "$0")/.."
d=packaging/sxmo-utils
fail=0

[ -f "$d/APKBUILD" ] || { echo "FAIL: $d/APKBUILD is missing" >&2; exit 1; }
sh -n "$d/APKBUILD" || fail=1

# check ARCH: the river subpackage and its install script exist on aarch64 and x86_64 only
check_arch() (
  CARCH=$1; srcdir=/nonexistent
  # shellcheck disable=SC1090,SC2154
  . "./$d/APKBUILD"
  bad() { echo "FAIL ($CARCH): $*" >&2; fail=1; }
  [ "$pkgname" = sxmo-utils ] || bad "pkgname is $pkgname"
  case "$pkgver" in 1.18.1_p20[0-9][0-9][0-9][0-9][0-9][0-9]) ;; *) bad "pkgver $pkgver is not 1.18.1_pYYYYMMDD" ;; esac
  tag=lab-${pkgver#*_p}.1
  case "$source" in *github.com/grenudi/sxmo-utils/archive/refs/tags/"$tag".tar.gz*) ;; *) bad "source is not the grenudi tag $tag" ;; esac
  [ "${builddir##*/}" = "sxmo-utils-$tag" ] || bad "builddir ${builddir##*/} is not what the tag unpacks to"
  sp=$(echo $subpackages)
  want="doc common::noarch x11::noarch wayland::noarch dwm::noarch i3::noarch sway::noarch
    modem:modem:noarch modem-mms:modem_mms:noarch modem-vvm:modem_vvm:noarch
    audio-pipewire:audio_pipewire:noarch audio-pulseaudio:audio_pulseaudio:noarch
    audio-bluetooth:audio_bluetooth:noarch audio-bluetooth-pipewire:audio_bluetooth_pipewire:noarch
    audio-bluetooth-pulseaudio:audio_bluetooth_pulseaudio:noarch udev"
  case "$CARCH" in aarch64|x86_64) want="$want river" ;; esac
  for s in $want; do
    case " $sp " in *" $pkgname-$s "*) ;; *) bad "subpackage $s is missing" ;; esac
  done
  case "$CARCH" in aarch64|x86_64) ;; *) case " $sp " in *" $pkgname-river "*) bad "river on $CARCH" ;; esac ;; esac
  for f in $install; do [ -f "$d/$f" ] || bad "install script $f is missing"; done
  case "$CARCH" in aarch64|x86_64) case " $install " in *"$pkgname-river.post-install"*) ;; *) bad "river install script not named" ;; esac ;; esac
  # the checksums of the files beside it, and no checksum without a file
  for f in $source; do
    case "$f" in *::*) continue ;; esac
    w=$(printf '%s\n' "$sha512sums" | sed -n "s/^\([0-9a-f]*\)  $f\$/\1/p")
    h=$(sha512sum "$d/$f" | cut -d' ' -f1)
    [ -n "$w" ] && [ "$w" = "$h" ] || bad "checksum of $f"
  done
  n=$(printf '%s\n' "$sha512sums" | grep -c .); m=$(echo $source | wc -w)
  [ "$n" = "$m" ] || bad "$n checksums for $m sources"
  if [ -z "${OFFLINE:-}" ] && [ "$CARCH" = aarch64 ]; then
    t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
    url=$(echo "$source" | tr -s ' \t\n' '\n' | sed -n 's/^.*:://p' | head -n 1)
    curl -sfL "$url" -o "$t/src.tar.gz" || bad "cannot fetch $url"
    w=$(printf '%s\n' "$sha512sums" | sed -n "s/^\([0-9a-f]*\)  $pkgname-$pkgver.tar.gz\$/\1/p")
    [ "$w" = "$(sha512sum "$t/src.tar.gz" | cut -d' ' -f1)" ] || bad "checksum of the lab tag's tarball"
    tar -xzf "$t/src.tar.gz" -C "$t"
    [ -d "$t/${builddir##*/}" ] || bad "the tag does not unpack to ${builddir##*/}"
    grep -rq SXMO_SUSPEND_RETRY_MAX "$t/${builddir##*/}/scripts" || bad "the tag lacks the suspend backoff (SXMO_SUSPEND_RETRY_MAX)"
  fi
  exit $fail
)
for a in aarch64 x86_64 armv7; do check_arch $a || fail=1; done
[ "$fail" = 0 ] && echo "ok: the sxmo-utils lab apk declaration holds"
exit $fail
