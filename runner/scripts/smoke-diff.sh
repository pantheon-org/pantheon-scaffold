#!/usr/bin/env bash
# shell: bash
#
# smoke-diff.sh - runs the TypeScript smoke suite and the Rust one (scaffold-release smoke) against the same runner binary and fails if
# what they print or their exit code differ. Phase 4 of docs/ts-tooling-rust-port.md: the TypeScript version stays as the reference until
# Phase 6 deletes it, and this script goes with it. It also fails when the suite itself fails on the binary it is given.
#
# usage: runner/scripts/smoke-diff.sh <path to the scaffold-release binary> <path to the runner binary> [--version <x.y.z>]
# Run it from the repository root, with bun, pkl and git on the PATH. Run it on every target: the filesystem is part of the test.
#
# Besides the binary it is given, it runs both suites on two inputs made to fail, so that a difference in how they report a failure shows
# too: the right binary with the wrong version, and a wrapper around the binary that reports no drift and fails `detach`. Both must fail,
# and both must print the same thing.

set -euo pipefail

tool="${1:?usage: smoke-diff.sh <scaffold-release binary> <runner binary> [--version <x.y.z>]}"
runner="${2:?usage: smoke-diff.sh <scaffold-release binary> <runner binary> [--version <x.y.z>]}"
shift 2
version="0.0.0"
[ "${1:-}" = "--version" ] && version="${2:?--version needs a value}"
tool="$(cd "$(dirname "$tool")" && pwd)/$(basename "$tool")"
runner="$(cd "$(dirname "$runner")" && pwd)/$(basename "$runner")"
[ -x "$tool" ] || { echo "not an executable: $tool" >&2; exit 2; }
[ -x "$runner" ] || { echo "not an executable: $runner" >&2; exit 2; }
[ -f runner/scripts/smoke.ts ] || { echo "run this from the repository root" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failed=0

# both <label> <binary> <version> <expected exit code of the suite>: runs the two suites and compares them.
both() {
  local label="$1" binary="$2" claimed="$3" expected="$4" ts_code rust_code
  set +e
  bun runner/scripts/smoke.ts "$binary" --version "$claimed" > "$work/ts.out" 2> "$work/ts.err"; ts_code=$?
  "$tool" smoke "$binary" --version "$claimed" > "$work/rust.out" 2> "$work/rust.err"; rust_code=$?
  set -e
  local ok=1
  [ "$ts_code" = "$rust_code" ] || ok=0
  [ "$rust_code" = "$expected" ] || ok=0
  cmp -s "$work/ts.out" "$work/rust.out" || ok=0
  cmp -s "$work/ts.err" "$work/rust.err" || ok=0
  if [ "$ok" = 1 ]; then
    echo "same     $label (exit $rust_code, $(grep -c '^PASS' "$work/rust.out") passed, $(grep -c '^FAIL' "$work/rust.out") failed)"
  else
    failed=$((failed + 1))
    echo "DIFFERS  $label (typescript exit $ts_code, rust exit $rust_code, expected $expected)"
    { diff "$work/ts.out" "$work/rust.out" | sed "s/^/  | /" | head -40; } || true
    { diff "$work/ts.err" "$work/rust.err" | sed "s/^/  | /" | head -10; } || true
  fi
}

both "the binary as given" "$runner" "$version" 0
both "the wrong version is claimed" "$runner" "9.9.9" 1

cat > "$work/broken" <<WRAP
#!/usr/bin/env bash
case "\$1" in
  check) "$runner" "\$@"; exit 0 ;;
  detach) echo "detach is broken" >&2; exit 1 ;;
  *) exec "$runner" "\$@" ;;
esac
WRAP
chmod +x "$work/broken"
both "a binary that finds no drift and cannot detach" "$work/broken" "$version" 1

echo
[ "$failed" = 0 ] && echo "3 comparisons, 0 differ" || echo "3 comparisons, $failed differ"
[ "$failed" = 0 ]
