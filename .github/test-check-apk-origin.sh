#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# test-check-apk-origin.sh - check-apk-origin.sh passes a complete APKBUILD and fails each one that
# lacks a part, a branch tarball, or a tag URL only in a comment.
set -eu
c="$(cd "$(dirname "$0")" && pwd)/check-apk-origin.sh"
t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
good() { cat <<'E'
# Origin: somewhere
pkgname=x
url="https://example.org/x"
license="MIT"
source="x.tar.gz::https://example.org/x/archive/refs/tags/v1.tar.gz"
E
}
good > "$t/good"
"$c" "$t/good" >/dev/null
n=0
expect_fail() { # NAME, sed script over the good file
  n=$((n + 1)); good | sed "$2" > "$t/bad$n"
  if "$c" "$t/bad$n" >/dev/null 2>&1; then echo "FAIL: $1 passed" >&2; exit 1; fi
}
expect_fail "no origin" '/^# Origin/d'
expect_fail "no license" '/^license/d'
expect_fail "no url" '/^url/d'
expect_fail "branch tarball" 's|refs/tags/v1|main|'
expect_fail "tag only in a comment" 's|^source=.*|source="local.patch"\n# https://example.org/x/archive/refs/tags/v1.tar.gz|'
expect_fail "git source" 's|^source=.*|source="git+https://example.org/x#branch=main"|'
echo "ok: check-apk-origin.sh passes a complete APKBUILD and fails $n bad ones"
