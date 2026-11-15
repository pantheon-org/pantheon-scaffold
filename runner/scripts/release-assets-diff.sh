#!/usr/bin/env bash
# shell: bash
#
# release-assets-diff.sh - runs the TypeScript release-assets tool and the Rust one (scaffold-release) on the same inputs and
# fails if their output or exit code differ. Phase 1 of docs/ts-tooling-rust-port.md: the TypeScript version stays as the
# reference until Phase 6 deletes it, and this script goes with it.
#
# usage: runner/scripts/release-assets-diff.sh <path to the scaffold-release binary>
# Run it from the repository root, with bun on the PATH.
#
# What is compared: stdout, stderr and the exit code, byte for byte, and for `checksums` the checksums.txt each one wrote. Two
# kinds of case are compared on the exit code only, because the two tools word the message differently on purpose: metadata
# that is not JSON (the message is the JSON parser's own) and a usage error (the Rust tool names its own command).
# Known differences that are not compared, all deliberate: Rust refuses extra arguments after `version`, `check-tag <tag>` and
# `checksums <dir> <version>` where TypeScript ignores them, and Rust refuses a version number too large for u64 (see
# docs/ts-tooling-test-mapping.md).

set -euo pipefail

binary="${1:?usage: release-assets-diff.sh <path to the scaffold-release binary>}"
binary="$(cd "$(dirname "$binary")" && pwd)/$(basename "$binary")"
[ -x "$binary" ] || { echo "not an executable: $binary" >&2; exit 2; }
repo="$(pwd)"
[ -f "$repo/runner/scripts/release-assets.ts" ] || { echo "run this from the repository root" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
checked=0
failed=0

sha() {
  if command -v sha256sum > /dev/null 2>&1; then sha256sum | cut -d' ' -f1; else shasum -a 256 | cut -d' ' -f1; fi
}

# run <name> <command...>: writes <work>/<name>.out, .err and .code, never fails.
run() {
  local name="$1"
  shift
  set +e
  "$@" > "$work/$name.out" 2> "$work/$name.err"
  echo $? > "$work/$name.code"
  set -e
}

# compare <label> <mode>: mode is "exact" or "code". Compares the ts.* and rust.* files written by run.
compare() {
  local label="$1" mode="$2" ok=1
  checked=$((checked + 1))
  cmp -s "$work/ts.code" "$work/rust.code" || ok=0
  if [ "$mode" = exact ]; then
    cmp -s "$work/ts.out" "$work/rust.out" || ok=0
    cmp -s "$work/ts.err" "$work/rust.err" || ok=0
  else
    cmp -s "$work/ts.out" "$work/rust.out" || ok=0
  fi
  if [ "$ok" = 1 ]; then
    echo "same     $label (exit $(cat "$work/ts.code"))"
  else
    failed=$((failed + 1))
    echo "DIFFERS  $label"
    for f in code out err; do
      if ! cmp -s "$work/ts.$f" "$work/rust.$f"; then
        echo "  --- $f: typescript"; sed 's/^/  | /' "$work/ts.$f"
        echo "  --- $f: rust"; sed 's/^/  | /' "$work/rust.$f"
      fi
    done
  fi
}

# --- check-tag and version: the text of pkl/PklProject and the tag are the inputs ---------------------------------------------
ts_text() { # <function> <text> <tag>
  FN="$1" TEXT="$2" TAG="${3:-}" bun -e '
    import { checkTag, pklProjectVersion } from "./runner/src/release";
    try {
      console.log(process.env.FN === "version" ? pklProjectVersion(process.env.TEXT!) : checkTag(process.env.TAG!, process.env.TEXT!));
    } catch (e) { console.error((e as Error).message); process.exit(1); }'
}
rust_text() { # <function> <text> <tag>
  local root="$work/root"
  rm -rf "$root"
  mkdir -p "$root/pkl"
  printf '%s' "$2" > "$root/pkl/PklProject"
  if [ "$1" = version ]; then (cd "$root" && "$binary" release-assets version); else (cd "$root" && "$binary" release-assets check-tag "$3"); fi
}

texts=(
  'version = "0.2.0"'
  'package {
  name = "x"
  version = "0.2.0"
}'
  '  version   =   "0.2.0-rc.1"'
  'package {}'
  'version = "1.0.0"
version = "2.0.0"'
  '// version = "1.0.0"'
  'version = ""'
  'version = "not-a-version"'
)
tags=(v0.2.0 v0.2.0-rc.1 0.2.0 v0.2 latest runner-v0.2.0 v0.2.0+build refs/tags/v0.2.0 v9.9.9 v01.2.3 v1.2.3-01 v1.0.0 v2.0.0 "")

for text in "${texts[@]}"; do
  label="${text//$'\n'/ }"
  run ts ts_text version "$text"
  run rust rust_text version "$text"
  compare "version, PklProject: $label" exact
  for tag in "${tags[@]}"; do
    run ts ts_text check-tag "$text" "$tag"
    run rust rust_text check-tag "$text" "$tag"
    compare "check-tag '$tag', PklProject: $label" exact
  done
done

# --- the real pkl/PklProject of this checkout, through the real command lines --------------------------------------------------
run ts bun runner/scripts/release-assets.ts version
run rust "$binary" release-assets version
compare "version, the real PklProject" exact
real="$(cat "$work/ts.out")"
for tag in "v$real" "v$real-x" v9.9.9; do
  run ts bun runner/scripts/release-assets.ts check-tag "$tag"
  run rust "$binary" release-assets check-tag "$tag"
  compare "check-tag '$tag', the real PklProject" exact
done

# --- checksums: a release folder is the input, and what is written is compared too ---------------------------------------------
version=0.2.0
pkg="pantheon-scaffold@$version"

# make_dir <dir> <meta version>: a consistent release folder, with the metadata saying <meta version>.
make_dir() {
  local dir="$1" meta_version="$2"
  mkdir -p "$dir"
  printf 'zip bytes' > "$dir/$pkg.zip"
  local zipsum
  zipsum="$(sha < "$dir/$pkg.zip")"
  printf '{"version":"%s","packageZipChecksums":{"sha256":"%s"}}' "$meta_version" "$zipsum" > "$dir/$pkg"
  sha < "$dir/$pkg" > "$dir/$pkg.sha256"
  printf '%s\n' "$zipsum" > "$dir/$pkg.zip.sha256"
  for target in linux-x64 linux-arm64 darwin-arm64; do printf 'binary for %s\n' "$target" > "$dir/pantheon-scaffold-$target"; done
}

# a case is a name and a function that damages a freshly made folder in <dir>
damage_none() { :; }
damage_missing_binary() { rm "$1/pantheon-scaffold-linux-arm64"; }
damage_missing_two() { rm "$1/pantheon-scaffold-linux-arm64" "$1/$pkg.zip"; }
damage_missing_all() { rm "$1"/pantheon-scaffold-* "$1/$pkg"*; }
damage_tampered_zip() { printf 'tampered' > "$1/$pkg.zip"; }
damage_bad_sha256() { printf '%064d' 0 > "$1/$pkg.sha256"; }
damage_bad_zip_sha256() { printf '%064d' 0 > "$1/$pkg.zip.sha256"; }
damage_other_version() {
  printf '{"version":"9.9.9","packageZipChecksums":{"sha256":"%s"}}' "$(sha < "$1/$pkg.zip")" > "$1/$pkg"
  sha < "$1/$pkg" > "$1/$pkg.sha256"
}
damage_no_version() { printf '{}' > "$1/$pkg"; sha < "$1/$pkg" > "$1/$pkg.sha256"; }
damage_no_zip_checksum() { printf '{"version":"%s"}' "$version" > "$1/$pkg"; sha < "$1/$pkg" > "$1/$pkg.sha256"; }
damage_sha256_with_spaces() { printf '  %s  \n\n' "$(sha < "$1/$pkg")" > "$1/$pkg.sha256"; }

for damage in none missing_binary missing_two missing_all tampered_zip bad_sha256 bad_zip_sha256 other_version no_version no_zip_checksum sha256_with_spaces; do
  make_dir "$work/dir-ts" "$version"
  damage_"$damage" "$work/dir-ts"
  rm -rf "$work/dir-rust"
  cp -R "$work/dir-ts" "$work/dir-rust"
  run ts bun runner/scripts/release-assets.ts checksums "$work/dir-ts" "$version"
  run rust "$binary" release-assets checksums "$work/dir-rust" "$version"
  compare "checksums, $damage" exact
  if [ -e "$work/dir-ts/checksums.txt" ] || [ -e "$work/dir-rust/checksums.txt" ]; then
    checked=$((checked + 1))
    if cmp -s "$work/dir-ts/checksums.txt" "$work/dir-rust/checksums.txt"; then
      echo "same     checksums.txt written, $damage"
    else
      failed=$((failed + 1))
      echo "DIFFERS  checksums.txt written, $damage"
    fi
  fi
  rm -rf "$work/dir-ts" "$work/dir-rust"
done

# metadata that is not JSON: the exit code and the empty stdout are compared, and not the parser's message
make_dir "$work/dir-ts" "$version"
printf 'not json' > "$work/dir-ts/$pkg"
rm -rf "$work/dir-rust"
cp -R "$work/dir-ts" "$work/dir-rust"
run ts bun runner/scripts/release-assets.ts checksums "$work/dir-ts" "$version"
run rust "$binary" release-assets checksums "$work/dir-rust" "$version"
compare "checksums, metadata that is not JSON (exit code only)" code
rm -rf "$work/dir-ts" "$work/dir-rust"

# --- usage errors: the exit code is compared, and not the usage line ------------------------------------------------------------
usage_cases=("" "checksums only-one" "check-tag" "nope")
for args in "${usage_cases[@]}"; do
  # shellcheck disable=SC2086
  run ts bun runner/scripts/release-assets.ts $args
  # shellcheck disable=SC2086
  run rust "$binary" release-assets $args
  : > "$work/ts.out"; : > "$work/rust.out"
  compare "usage error: '$args' (exit code only)" code
done

echo
echo "$checked comparisons, $failed differ"
[ "$failed" = 0 ]
