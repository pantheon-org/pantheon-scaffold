#!/usr/bin/env bash
#
# apply-pin-update.sh
#
# Checks the tar that the `pins` job of update-pins.yml made (collect-pin-update.sh) and, if every check passes, writes its files into the
# checkout. Run by the `update` job, which holds the write token, from the script of the checkout of main: never from the tar. The tar is
# untrusted input: it was made by a job that ran the repository's tools over data from GitHub.
#
# usage: apply-pin-update.sh <pin-update.tar> <sha256 of the tar> <out dir> [--check-only]
#        apply-pin-update.sh --mask            (reads a workflow on stdin, writes it with its pins masked; for the tests)
#
# Nothing is written, not even <out dir>, until every check has passed, and a refusal stops the script with a message and exit 1. It never
# deletes a file. With --check-only it checks and writes nothing in the checkout (a dry run holds no token and writes nothing).
#
# What is checked, against the checkout (main):
#   - the tar is the one that was built (its sha256), holds only regular files and directories, and no name has `..` or a leading `/`;
#   - the files in tree/ are exactly the files in changed.txt (a file the upload dropped is not a silent no-op);
#   - each path is pkl/Pins.pkl, .pkl-gen/manifest.json or a workflow that main's manifest lists as owned, and is a regular file on main;
#   - a workflow equals main's with every pin masked, and every commit in it is in the new Pins.pkl or was in main's file;
#   - pkl/Pins.pkl equals main's with every sha and version masked;
#   - the manifest equals main's except for the hashes, and each hash is that of the new file (main's hash for a file that did not change);
#   - freshness.json has the shape the freshness check accepts, and no file is over its size limit.

set -euo pipefail

tab=$'\t'
cr=$'\r'
max_bytes=1048576
max_small=65536

# The same rule as `mask_pinned_uses` in crates/scaffold-core/src/pinmask.rs (a test compares the two): on a `uses:` line, the commit and the
# version comment of a pinned action become `@<pin>`.
mask_uses() {
  sed -E "s/^([ $tab]*(-[ $tab]+)?uses:[ $tab]+['\"]?[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+(\/[^@ $tab'\"]+)?)@[0-9a-f]{40}(['\"]?)([ $tab]+#[^$cr]*)?($cr?)\$/\1@<pin>\4\6/"
}

if [ "${1:-}" = "--mask" ]; then
  mask_uses
  exit 0
fi

tar_file="${1:?usage: apply-pin-update.sh <pin-update.tar> <sha256> <out dir> [--check-only]}"
expected="${2:?usage: apply-pin-update.sh <pin-update.tar> <sha256> <out dir> [--check-only]}"
out="${3:?usage: apply-pin-update.sh <pin-update.tar> <sha256> <out dir> [--check-only]}"
mode="${4:-apply}"
[ "$mode" = apply ] || [ "$mode" = --check-only ] || { echo "usage: the fourth argument is --check-only or nothing" >&2; exit 2; }

fail() { echo "refused: $*" >&2; exit 1; }

sha256() {
  if command -v sha256sum > /dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

for tool in tar jq; do command -v "$tool" > /dev/null || fail "$tool is needed on the PATH"; done
manifest=.pkl-gen/manifest.json
[ -f "$manifest" ] || fail "no $manifest in the checkout: run this from the root of main"
[ -f "$tar_file" ] || fail "no tar at $tar_file"
[ "$(sha256 "$tar_file")" = "$expected" ] || fail "the tar is not the one that was built (its sha256 differs)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Members: only regular files and directories, with names of the shape collect-pin-update.sh writes.
types="$(tar -tvf "$tar_file" | cut -c1)"
if printf '%s\n' "$types" | grep -qv '^[-d]$'; then fail "the tar holds a member that is not a regular file or a directory (a link or a device)"; fi
name_ok='^(changed\.txt|freshness\.json|summary\.md|tree|tree(/[A-Za-z0-9._-]+)+)/?$'
while IFS= read -r name; do
  case "$name" in *..* | /*) fail "the tar has a name with .. or a leading /: $name" ;; esac
  [[ "$name" =~ $name_ok ]] || fail "the tar has a member outside its layout: $name"
done < <(tar -tf "$tar_file")

mkdir "$work/x"
tar -xf "$tar_file" -C "$work/x" --no-same-owner --no-same-permissions
if find "$work/x" -mindepth 1 ! -type f ! -type d -print | grep -q .; then fail "the tar holds something that is not a regular file or a directory"; fi
while IFS= read -r -d '' found; do
  rel="${found#"$work"/x/}"
  [[ "$rel" =~ $name_ok ]] || fail "the tar holds a name outside its layout: $rel"
done < <(find "$work/x" -mindepth 1 -print0)

tree="$work/x/tree"
[ -d "$tree" ] || fail "the tar has no tree/"
[ -f "$work/x/changed.txt" ] || fail "the tar has no changed.txt"
[ -f "$work/x/freshness.json" ] || fail "the tar has no freshness.json: an empty or half-built tar is a failure"

(cd "$tree" && find . -type f -print | sed 's|^\./||' | LC_ALL=C sort) > "$work/found.txt"
LC_ALL=C sort "$work/x/changed.txt" > "$work/listed.txt"
cmp -s "$work/found.txt" "$work/listed.txt" || fail "the files in tree/ are not the files in changed.txt (an upload may have dropped hidden files)"

# freshness.json: the shape the freshness check accepts.
[ "$(wc -c < "$work/x/freshness.json")" -le 1024 ] || fail "freshness.json is too large"
jq -e '(keys == ["lastSuccessfulPinCheck", "schemaVersion"]) and .schemaVersion == 1 and (.lastSuccessfulPinCheck | type == "string" and test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"))' \
  "$work/x/freshness.json" > /dev/null 2>&1 || fail "freshness.json is not a record of schema 1 with a UTC time to the second"
if [ -f "$work/x/summary.md" ]; then
  [ "$(wc -c < "$work/x/summary.md")" -le "$max_small" ] || fail "summary.md is too large"
fi

new_pins="$tree/pkl/Pins.pkl"
pin_shas() { grep -oE '[0-9a-f]{40}' "$1" | LC_ALL=C sort -u || true; }
uses_shas() { grep -E "^[ $tab]*(-[ $tab]+)?uses:" "$1" | grep -oE '@[0-9a-f]{40}' | cut -c2- | LC_ALL=C sort -u || true; }

# Each changed path.
while IFS= read -r path; do
  [ -n "$path" ] || continue
  case "$path" in
    pkl/Pins.pkl | .pkl-gen/manifest.json) ;;
    .github/workflows/*.yml)
      [[ "$path" =~ ^\.github/workflows/[A-Za-z0-9._-]+\.yml$ ]] || fail "$path is not a workflow name this script accepts"
      jq -e --arg p "$path" '.files[$p].class == "owned"' "$manifest" > /dev/null || fail "$path is not a workflow that main's manifest lists as owned (a hand-written workflow, or a new one)" ;;
    *) fail "$path is outside the list of files a pin update may change" ;;
  esac
  { [ -f "$path" ] && [ ! -L "$path" ] && [ ! -x "$path" ]; } || fail "$path is not a regular, non-executable file on main"
  [ "$(wc -c < "$tree/$path")" -le "$max_bytes" ] || fail "$path is larger than $max_bytes bytes"
  case "$path" in
    pkl/Pins.pkl)
      cmp -s <(sed -E 's/sha = "[0-9a-f]{40}"; version = "[^"]*"/sha = "<pin>"; version = "<pin>"/' "$tree/$path") \
        <(sed -E 's/sha = "[0-9a-f]{40}"; version = "[^"]*"/sha = "<pin>"; version = "<pin>"/' "$path") \
        || fail "pkl/Pins.pkl differs from main in more than the sha and version of its pins" ;;
    .pkl-gen/manifest.json)
      cmp -s <(jq -S 'del(.files[].hash)' "$tree/$path") <(jq -S 'del(.files[].hash)' "$path") \
        || fail "the manifest differs from main in more than its hashes" ;;
    .github/workflows/*)
      cmp -s <(mask_uses < "$tree/$path") <(mask_uses < "$path") || fail "$path differs from main in more than the pins of its uses: lines"
      allowed_shas="$( { [ -f "$new_pins" ] && pin_shas "$new_pins"; uses_shas "$path"; } | LC_ALL=C sort -u)"
      while IFS= read -r sha; do
        [ -n "$sha" ] || continue
        printf '%s\n' "$allowed_shas" | grep -qx "$sha" || fail "$path uses the commit $sha, which is neither in the new pkl/Pins.pkl nor in main's file"
      done < <(uses_shas "$tree/$path") ;;
  esac
done < "$work/listed.txt"

# The manifest the checkout will have after the apply: the new one, or main's. Each hash in it is the hash of the file as it will be.
effective="$manifest"
[ ! -f "$tree/$manifest" ] || effective="$tree/$manifest"
while IFS="$tab" read -r path hash; do
  if [ -f "$tree/$path" ]; then
    [ "$hash" = "$(sha256 "$tree/$path")" ] || fail "the manifest hash of $path is not the hash of the new file"
  elif [ -f "$path" ]; then
    [ "$hash" = "$(sha256 "$path")" ] || fail "the manifest hash of $path is not the hash of the file on main"
  fi
done < <(jq -r '.files | to_entries[] | select(.value.class == "owned" and .value.hash != null) | "\(.key)\t\(.value.hash)"' "$effective")

if [ "$mode" = "--check-only" ]; then
  echo "check only: $(wc -l < "$work/listed.txt" | tr -d ' ') file(s) would be applied"
else
  while IFS= read -r path; do
    [ -n "$path" ] || continue
    install -m 644 "$tree/$path" "$path"
  done < "$work/listed.txt"
  echo "applied $(wc -l < "$work/listed.txt" | tr -d ' ') file(s)"
fi

mkdir -p "$out"
cp "$work/x/freshness.json" "$out/freshness.json"
if [ -f "$work/x/summary.md" ]; then cp "$work/x/summary.md" "$out/summary.md"; fi
