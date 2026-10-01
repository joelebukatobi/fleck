#!/usr/bin/env bash
# The release notes for the tag that is checked out: every commit since the
# previous tag, as it was written. Used both for the GitHub release and for the
# CHANGELOG the apt repository publishes.
set -euo pipefail

version="${GITHUB_REF_NAME:-$(git describe --tags --always)}"
version="${version#v}"
previous="$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || true)"

printf '## %s - %s\n\n' "${version}" "$(date -u +%Y-%m-%d)"
if [ -n "${previous}" ]; then
    git log --no-merges --reverse --pretty='- %s' "${previous}..HEAD"
else
    git log --no-merges --reverse --pretty='- %s'
fi
