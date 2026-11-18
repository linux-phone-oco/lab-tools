#!/bin/sh
# check-changelog.sh BASE HEAD - a changelog never changes without a version (release-tags-every-change).
# For every CHANGELOG.md the change touches, its package must either bump its [package] version in
# the same change, or the change must complete a version the package already carries: it adds the
# heading `## [V]` for the version V in the package's Cargo.toml, a heading the merge-base did not
# have (the follow-up release PR release-plz opens for a family member whose entry came late, #113
# after #111). Anything else - a changelog edited alone, a version named twice - is refused.
set -eu
mb=$(git merge-base "$1" "$2")
head=$2
bad=0

# the [package] version of DIR's Cargo.toml at REV, empty when there is none
version_at() {
  git show "$1:$2/Cargo.toml" 2>/dev/null | sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' | head -n 1
}

git -c core.quotepath=off diff --name-only "$mb" "$head" | grep -E '(^|/)CHANGELOG\.md$' > "${TMPDIR:-/tmp}/changed-logs.$$" || true
while IFS= read -r log; do
  dir=$(dirname "$log")
  new=$(version_at "$head" "$dir")
  if [ -z "$new" ]; then
    echo "$log changed but $dir/Cargo.toml has no literal [package] version"
    bad=1
    continue
  fi
  old=$(version_at "$mb" "$dir")
  if [ "$old" != "$new" ]; then continue; fi
  heading="## [$new]"
  if git show "$mb:$log" 2>/dev/null | grep -qF "$heading"; then
    echo "$log: version $new is already named in the changelog and no version changed"
    bad=1
  elif ! git show "$head:$log" 2>/dev/null | grep -qF "$heading"; then
    echo "$log changed and neither its version nor a heading for version $new did"
    bad=1
  fi
done < "${TMPDIR:-/tmp}/changed-logs.$$"
rm -f "${TMPDIR:-/tmp}/changed-logs.$$"
exit $bad
