#!/usr/bin/env bash
#
# collect-pin-update.sh
#
# Gathers what the `pins` job of update-pins.yml changed into one tar, for the `update` job. That job holds the write token and treats the tar as
# untrusted input (apply-pin-update.sh); this job holds no token. Run from the root of the checkout the pins were updated and rendered in.
#
# usage: collect-pin-update.sh <out dir> <freshness.json> [<summary.md>]
#
# The tar holds, at its top level: `changed.txt` (the changed paths, sorted, one per line), `freshness.json`, `summary.md` when there is one,
# and `tree/` with each changed file at its path. It is written to <out dir>/pin-update.tar.
#
# The change set must be modified files only, each one of: pkl/Pins.pkl, .pkl-gen/manifest.json, .github/workflows/<name>.yml. A deleted, added,
# untracked or renamed file, a symlink, or a path outside that list stops the script with a message and writes nothing. No change at all is
# a valid answer (no pin moved): the tar then holds the freshness record alone.

set -euo pipefail
export COPYFILE_DISABLE=1

out="${1:?usage: collect-pin-update.sh <out dir> <freshness.json> [<summary.md>]}"
freshness="${2:?usage: collect-pin-update.sh <out dir> <freshness.json> [<summary.md>]}"
summary="${3:-}"

[ -f "$freshness" ] || { echo "refused: there is no freshness record at $freshness" >&2; exit 1; }

allowed='^(pkl/Pins\.pkl|\.pkl-gen/manifest\.json|\.github/workflows/[A-Za-z0-9._-]+\.yml)$'
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/tree"
: > "$stage/changed.txt"

while IFS= read -r -d '' entry; do
  status="${entry:0:2}"
  path="${entry:3}"
  case "$status" in
    ' M' | 'M ' | 'MM') ;;
    *) echo "refused: $status $path (only modified files may change)" >&2; exit 1 ;;
  esac
  [[ "$path" =~ $allowed ]] || { echo "refused: $path is outside the list of files a pin update may change" >&2; exit 1; }
  { [ -f "$path" ] && [ ! -L "$path" ]; } || { echo "refused: $path is not a regular file" >&2; exit 1; }
  mkdir -p "$stage/tree/$(dirname "$path")"
  cp "$path" "$stage/tree/$path"
  printf '%s\n' "$path" >> "$stage/changed.txt"
done < <(git status --porcelain=v1 -z --untracked-files=all)

LC_ALL=C sort -o "$stage/changed.txt" "$stage/changed.txt"
cp "$freshness" "$stage/freshness.json"
members=(changed.txt freshness.json)
if [ -n "$summary" ] && [ -f "$summary" ]; then
  cp "$summary" "$stage/summary.md"
  members+=(summary.md)
fi
mkdir -p "$out"
tar -cf "$out/pin-update.tar" -C "$stage" "${members[@]}" tree
echo "collected $(wc -l < "$stage/changed.txt" | tr -d ' ') changed file(s) into $out/pin-update.tar"
