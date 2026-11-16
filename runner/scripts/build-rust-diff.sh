#!/usr/bin/env bash
# shell: bash
#
# build-rust-diff.sh - runs the TypeScript build script (runner/scripts/build-rust.ts) and the Rust one (scaffold-release build-rust) and
# fails if anything differs. Phase 5 of docs/ts-tooling-rust-port.md: the TypeScript version stays as the reference until Phase 6 deletes
# it, and this script goes with it.
#
# usage: runner/scripts/build-rust-diff.sh <path to the scaffold-release binary> [--version <x.y.z>] [--also-ts-twice] [--no-build]
# Run it from the repository root, with bun, cargo and rustup on the PATH, and zig too on Linux (MISE_ENV=rust-build, mise.rust-build.toml).
# It builds the runner for the target of this machine twice from scratch, so it takes about as long as two release builds.
#
# What is compared:
#   plan     for each of the three targets, with a version with and without a leading v, the plan of each tool as JSON (the cargo
#            arguments, the environment, the paths, the zig cc wrapper to the byte), and what each prints for an input it refuses;
#   build    both tools build the runner for the target of this machine, one after the other, each in its own copy of the checkout that
#            has the same path and no target folder, and the two binaries must be byte for byte the same. CARGO_TARGET_DIR is unset: a
#            build with another target directory gave other bytes, so both builds are made the same way.
# --also-ts-twice adds a third build with the TypeScript tool, to say whether the TypeScript tool is reproducible by itself when the two
# differ. It is a way to look for the cause and is not run in CI. --no-build leaves out the two builds and compares the plans only,
# which takes seconds; CI does not use it.

set -euo pipefail

tool="${1:?usage: build-rust-diff.sh <scaffold-release binary> [--version <x.y.z>] [--also-ts-twice] [--no-build]}"
shift
version="0.0.1-diff"
twice=0
build=1
while [ $# -gt 0 ]; do
  case "$1" in
    --version) version="${2:?--version needs a value}"; shift 2 ;;
    --also-ts-twice) twice=1; shift ;;
    --no-build) build=0; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
tool="$(cd "$(dirname "$tool")" && pwd)/$(basename "$tool")"
[ -x "$tool" ] || { echo "not an executable: $tool" >&2; exit 2; }
[ -f runner/scripts/build-rust.ts ] || { echo "run this from the repository root" >&2; exit 2; }
unset CARGO_TARGET_DIR
repo="$(pwd -P)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$tool" "$work/tool"
tool="$work/tool"
checked=0
failed=0

verdict() { # <label> <ok> <detail command...>
  local label="$1" ok="$2"
  shift 2
  checked=$((checked + 1))
  if [ "$ok" = 1 ]; then
    echo "same     $label"
  else
    failed=$((failed + 1))
    echo "DIFFERS  $label"
    { "$@" 2>&1 | sed "s/^/  | /" | head -30; } || true
  fi
}

# Runs a TypeScript and a Rust command, and compares stdout, stderr and the exit code. <ts command> and <rust command> are strings.
both() { # <label> <ts command> <rust command>
  local label="$1" ts_code rust_code ok=1
  set +e
  (eval "$2") > "$work/ts.out" 2> "$work/ts.err"; ts_code=$?
  (eval "$3") > "$work/rust.out" 2> "$work/rust.err"; rust_code=$?
  set -e
  [ "$ts_code" = "$rust_code" ] || ok=0
  cmp -s "$work/ts.out" "$work/rust.out" || ok=0
  cmp -s "$work/ts.err" "$work/rust.err" || ok=0
  verdict "$label (exit $rust_code)" "$ok" bash -c "echo 'typescript exit $ts_code, rust exit $rust_code'; diff '$work/ts.out' '$work/rust.out'; diff '$work/ts.err' '$work/rust.err'"
}

# --- plan ----------------------------------------------------------------------------------------------------------------------
cat > "$work/plan.ts" <<EOF
import { plan } from "$repo/runner/scripts/build-rust";
const [target, version, out, root] = Bun.argv.slice(2);
console.log(JSON.stringify(plan(target, version, out, root), null, 1));
EOF
for target in linux-x64 linux-arm64 darwin-arm64; do
  for v in 0.2.0 v0.3.0-rc.1; do
    both "plan: $target $v" "bun '$work/plan.ts' $target $v '$repo/dist' '$repo'" "'$tool' build-rust --target $target --version $v --out '$repo/dist' --print-plan"
  done
done

# A target this machine cannot build is refused before anything is run, and so is anything else that is wrong with the arguments.
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) host=linux-x64; other=linux-arm64 ;;
  Linux-aarch64) host=linux-arm64; other=linux-x64 ;;
  Darwin-arm64) host=darwin-arm64; other=linux-x64 ;;
  *) echo "this machine builds no release target" >&2; exit 2 ;;
esac
ts_build="bun $repo/runner/scripts/build-rust.ts"
rs_build="'$tool' build-rust"
both "a target of another machine ($other)" "$ts_build --target $other" "$rs_build --target $other"
both "an unknown target" "$ts_build --target windows-x64" "$rs_build --target windows-x64"
both "a version that is not one" "$ts_build --target $host --version latest" "$rs_build --target $host --version latest"
both "no target" "$ts_build" "$rs_build"
both "a target with no value" "$ts_build --target" "$rs_build --target"
both "a version with no value" "$ts_build --target $host --version" "$rs_build --target $host --version"

# --- build ---------------------------------------------------------------------------------------------------------------------
if [ "$build" = 0 ]; then
  echo
  echo "$checked comparisons, $failed differ (no build: the plans only)"
  [ "$failed" = 0 ]
  exit
fi
# Each build runs in a copy of the checkout at the same path, with no target folder, so that both start from nothing and nothing in a
# build can differ because of where it ran.
tree="$work/tree"
fresh() {
  rm -rf "$tree"
  mkdir -p "$tree"
  tar --exclude=./target --exclude=./.git --exclude=./.context --exclude=./.claude --exclude=./node_modules -cf - . | tar -x -C "$tree"
}
build_with() { # <label> <out folder> <command...>
  local label="$1" out="$2"
  shift 2
  fresh
  echo "building ($label) ..." >&2
  set +e
  (cd "$tree" && "$@" --target "$host" --version "$version" --out "$out") > "$work/$label.log" 2>&1
  local code=$?
  set -e
  echo "$code" > "$work/$label.code"
  echo "  $label: exit $code, last line: $(tail -n 1 "$work/$label.log")" >&2
}
build_with ts "$work/out-ts" bun runner/scripts/build-rust.ts
build_with rust "$work/out-rust" "$tool" build-rust
asset="pantheon-scaffold-$host"

ok=1
[ "$(cat "$work/ts.code")" = 0 ] && [ "$(cat "$work/rust.code")" = 0 ] || ok=0
verdict "build: both tools build $host and exit 0" "$ok" bash -c "tail -n 5 '$work/ts.log' '$work/rust.log'"

ok=1
diff <(tail -n 1 "$work/ts.log" | sed "s#$work/out-ts#<out>#") <(tail -n 1 "$work/rust.log" | sed "s#$work/out-rust#<out>#") > "$work/line.diff" 2>&1 || ok=0
verdict "build: what each prints at the end" "$ok" cat "$work/line.diff"

ok=1
[ -f "$work/out-ts/$asset" ] && [ -f "$work/out-rust/$asset" ] || ok=0
[ "$(ls "$work/out-ts")" = "$(ls "$work/out-rust")" ] || ok=0
verdict "build: the same files in the out folder ($asset)" "$ok" bash -c "ls -l '$work/out-ts' '$work/out-rust'"

ok=1
ts_sha="$(shasum -a 256 "$work/out-ts/$asset" 2> /dev/null | cut -d' ' -f1)"
rust_sha="$(shasum -a 256 "$work/out-rust/$asset" 2> /dev/null | cut -d' ' -f1)"
[ -n "$ts_sha" ] && [ "$ts_sha" = "$rust_sha" ] || ok=0
verdict "build: the two binaries are byte for byte the same (sha256 ${ts_sha:0:16})" "$ok" echo "typescript $ts_sha, rust $rust_sha"
mode() { stat -c %a "$1" 2> /dev/null || stat -f %Lp "$1"; }
ok=0
[ "$(mode "$work/out-ts/$asset")" = "$(mode "$work/out-rust/$asset")" ] && ok=1
verdict "build: the same file mode" "$ok" ls -l "$work/out-ts/$asset" "$work/out-rust/$asset"

if [ "$twice" = 1 ]; then
  build_with ts2 "$work/out-ts2" bun runner/scripts/build-rust.ts
  ts2_sha="$(shasum -a 256 "$work/out-ts2/$asset" | cut -d' ' -f1)"
  ok=0
  [ "$ts_sha" = "$ts2_sha" ] && ok=1
  verdict "control: the TypeScript tool twice gives the same binary" "$ok" echo "first $ts_sha, second $ts2_sha"
fi

echo
echo "$checked comparisons, $failed differ"
[ "$failed" = 0 ]
