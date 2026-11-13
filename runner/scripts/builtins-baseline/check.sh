#!/usr/bin/env bash
# Snapshot a tree and compare it with a baseline snapshot. Run from the root of the tree to check.
#
#   runner/scripts/builtins-baseline/check.sh <baseline snapshot dir> <binary> [work dir] [--api old|new]
#
# The baseline snapshot is made once, from `git archive 0b466dc` and the binary built from that commit (see README.md).
set -euo pipefail
if [ "$#" -lt 2 ]; then
  echo "usage: check.sh <baseline snapshot dir> <binary> [work dir] [--api old|new]" >&2
  exit 2
fi
baseline=$1
binary=$2
shift 2
work=${1:-}
case "$work" in --api | "") work="" ;; *) shift ;; esac
here="$(cd "$(dirname "$0")" && pwd)"
tmp=${work:-$(mktemp -d)}
rm -rf "$tmp/candidate"
command bun "$here/snapshot.ts" . "$binary" "$tmp/candidate" "$@" > "$tmp/candidate.log" 2>&1 || { tail -20 "$tmp/candidate.log"; exit 1; }
command bun "$here/compare.ts" "$baseline" "$tmp/candidate"
