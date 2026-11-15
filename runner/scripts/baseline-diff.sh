#!/usr/bin/env bash
# shell: bash
#
# baseline-diff.sh - runs the TypeScript baseline scripts (runner/scripts/builtins-baseline/) and the Rust ones (scaffold-release baseline)
# on the same tree and the same runner binary, and fails if anything differs. Phase 4 of docs/ts-tooling-rust-port.md: the TypeScript
# versions stay as the reference until Phase 6 deletes them, and this script goes with it.
#
# usage: runner/scripts/baseline-diff.sh <path to the scaffold-release binary> <path to the runner binary> [--no-packaged]
# Run it from the repository root, with bun, pkl, unzip and jq on the PATH. --no-packaged leaves out the `packaged` comparison, which
# builds the package and takes longest.
#
# What is compared:
#   snapshot  both write a snapshot of this tree: the two folders must be byte for byte the same, and so must what they print;
#   freeze    both freeze that snapshot: the text must be the same;
#   compare   both compare the frozen baseline of the repository, and a series of snapshot folders (the same one, and copies with one
#             thing changed each), with the candidate: what they print and their exit code must be the same, so a difference is reported
#             in the same words;
#   check     check.sh and `baseline check` against the frozen baseline, each in a work folder that exists (check.sh fails when it does
#             not, and `baseline check` creates it: the one difference of behaviour, which is not a use the workflows make);
#   packaged  both build the package and compare its contract with the folder contract.

set -euo pipefail

tool="${1:?usage: baseline-diff.sh <scaffold-release binary> <runner binary> [--no-packaged]}"
runner="${2:?usage: baseline-diff.sh <scaffold-release binary> <runner binary> [--no-packaged]}"
packaged=1
[ "${3:-}" = "--no-packaged" ] && packaged=0
tool="$(cd "$(dirname "$tool")" && pwd)/$(basename "$tool")"
runner="$(cd "$(dirname "$runner")" && pwd)/$(basename "$runner")"
[ -x "$tool" ] || { echo "not an executable: $tool" >&2; exit 2; }
[ -x "$runner" ] || { echo "not an executable: $runner" >&2; exit 2; }
[ -f runner/scripts/builtins-baseline/snapshot.ts ] || { echo "run this from the repository root" >&2; exit 2; }
command -v jq > /dev/null || { echo "jq is needed" >&2; exit 2; }

dir=runner/scripts/builtins-baseline
frozen="$dir/baseline.json"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
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

# Runs both and compares the exit codes only, for a usage error: each names its own command in the usage line.
exit_codes() { # <label> <ts command> <rust command>
  local ts_code rust_code ok=0
  set +e
  (eval "$2") > /dev/null 2>&1; ts_code=$?
  (eval "$3") > /dev/null 2>&1; rust_code=$?
  set -e
  [ "$ts_code" = "$rust_code" ] && ok=1
  verdict "$1 (exit code only, exit $rust_code)" "$ok" echo "typescript exit $ts_code, rust exit $rust_code"
}

# --- snapshot ------------------------------------------------------------------------------------------------------------------
rm -rf "$work/ts" "$work/rust"
bun "$dir/snapshot.ts" . "$runner" "$work/ts" > "$work/ts.log" 2>&1
"$tool" baseline snapshot . "$runner" "$work/rust" > "$work/rust.log" 2>&1
snapshots=("$work/ts"/*.json)
n="${#snapshots[@]}"
ok=1
diff -r -q "$work/ts" "$work/rust" > "$work/snap.diff" 2>&1 || ok=0
diff <(sed "s#$work/ts#<out>#" "$work/ts.log") <(sed "s#$work/rust#<out>#" "$work/rust.log") > "$work/log.diff" 2>&1 || ok=0
verdict "snapshot: $n configurations, the two folders byte for byte and what they print" "$ok" cat "$work/snap.diff" "$work/log.diff"

# --- freeze --------------------------------------------------------------------------------------------------------------------
both "freeze of that snapshot" "bun $dir/freeze.ts $work/ts" "'$tool' baseline freeze $work/ts"
exit_codes "freeze with no argument (a usage error)" "bun $dir/freeze.ts" "'$tool' baseline freeze"

# --- compare -------------------------------------------------------------------------------------------------------------------
cmp_dirs() { # <label> <baseline> <candidate>
  both "compare: $1" "bun $dir/compare.ts '$2' '$3'" "'$tool' baseline compare '$2' '$3'"
}
cmp_dirs "the frozen baseline of the repository with this snapshot" "$frozen" "$work/ts"
cmp_dirs "a snapshot folder with itself" "$work/ts" "$work/ts"
cmp_dirs "the two snapshots, the TypeScript one and the Rust one" "$work/ts" "$work/rust"

first="$(basename "${snapshots[0]}")"
with_error="$(grep -l '"err": "[^"]' "$work/ts"/*.json | head -1 | xargs basename)"
mutate() { # <name> <file> <jq filter>: a copy of the snapshot with one file changed
  rm -rf "$work/mut"
  cp -R "$work/ts" "$work/mut"
  jq "$3" "$work/ts/$2" > "$work/mut/$2"
}
mutate "exit code" "$first" '.render.code = 9'
cmp_dirs "a changed exit code (folder against folder)" "$work/ts" "$work/mut"
cmp_dirs "a changed exit code (frozen baseline against folder)" "$frozen" "$work/mut"
mutate "stdout" "$first" '.check.out = (.check.out + "extra line\n")'
cmp_dirs "a changed stdout" "$work/ts" "$work/mut"
mutate "files" "$first" '.files += ["new.txt 644 abc"]'
cmp_dirs "an extra file (folder against folder)" "$work/ts" "$work/mut"
cmp_dirs "an extra file (frozen baseline against folder)" "$frozen" "$work/mut"
mutate "manifest" "$first" '.manifest = "{}"'
cmp_dirs "a changed manifest (folder against folder)" "$work/ts" "$work/mut"
cmp_dirs "a changed manifest (frozen baseline against folder)" "$frozen" "$work/mut"
mutate "message" "$with_error" '.render.err = (.render.err + "a new problem\n")'
cmp_dirs "a changed error message ($with_error)" "$work/ts" "$work/mut"
mutate "line numbers" "$with_error" '(.contract, .render, .check) |= (.err |= (gsub("line [0-9]+"; "line 999") + "  7 | an extra source line\n   ^^^^\n"))'
cmp_dirs "only the line numbers and source snippets of an error changed ($with_error)" "$work/ts" "$work/mut"
rm -rf "$work/mut"; cp -R "$work/ts" "$work/mut"; rm "$work/mut/$first"
cmp_dirs "a configuration missing from the candidate" "$work/ts" "$work/mut"
cmp_dirs "a configuration missing from the candidate (frozen baseline)" "$frozen" "$work/mut"
rm -rf "$work/mut"; cp -R "$work/ts" "$work/mut"; cp "$work/ts/$first" "$work/mut/zz-extra.json"
cmp_dirs "an extra configuration in the candidate" "$work/ts" "$work/mut"
exit_codes "compare with no arguments (a usage error)" "bun $dir/compare.ts" "'$tool' baseline compare"

# --- check ---------------------------------------------------------------------------------------------------------------------
mkdir -p "$work/c-ts" "$work/c-rust"
both "check against the frozen baseline" "bash $dir/check.sh '$frozen' '$runner' '$work/c-ts' | sed 's#$work/c-ts#<work>#g'" "'$tool' baseline check '$frozen' '$runner' '$work/c-rust' | sed 's#$work/c-rust#<work>#g'"

# --- packaged ------------------------------------------------------------------------------------------------------------------
if [ "$packaged" = 1 ]; then
  both "packaged: the package built from this tree against its folder" \
    "bun $dir/packaged.ts . '$runner' '$work/p-ts' | sed 's#$work/p-ts#<work>#g'" \
    "'$tool' baseline packaged . '$runner' '$work/p-rust' | sed 's#$work/p-rust#<work>#g'"
fi

echo
echo "$checked comparisons, $failed differ"
[ "$failed" = 0 ]
