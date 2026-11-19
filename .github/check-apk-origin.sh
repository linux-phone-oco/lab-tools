#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# check-apk-origin.sh - every lab apk (packaging/*/APKBUILD) names what AGPL-3.0 6(d) and a reader ask
# beside the object code: a license=, an https url=, a source at a public tag (a .../archive/refs/tags/
# or .../releases/download/ URL inside source="...") and an "# Origin:" line saying where the package
# comes from. Files named on the command line (relative to where you stand) are checked instead.
# Values in double quotes. Proven by .github/test-check-apk-origin.sh.
set -eu
[ "$#" -gt 0 ] || { cd "$(dirname "$0")/.."; set -- packaging/*/APKBUILD; }
fail=0
for f in "$@"; do
  bad() { echo "FAIL: $f: $*" >&2; fail=1; }
  [ -f "$f" ] || { bad "is missing"; continue; }
  grep -Eq '^license="[^"]+"' "$f" || bad "has no license= line"
  grep -Eq '^url="https://[^"]+"' "$f" || bad "has no https url= line"
  grep -Eq '^# Origin: .+' "$f" || bad "has no '# Origin:' line"
  src=$(awk '/^source="/{p=1} p{print} p && /"[ \t]*$/ && !/^source="[ \t]*$/{p=0}' "$f")
  printf '%s\n' "$src" | grep -Eq 'https://[^ "]*/(archive/refs/tags|releases/download)/[^ "]+' || bad "its source= is not at a public tag"
  ! printf '%s\n' "$src" | grep -Eq 'git\+|#branch=' || bad "its source= follows a branch or a git checkout"
done
[ "$fail" = 0 ] && echo "ok: every lab apk names its licence, url, origin and public source"
exit $fail
