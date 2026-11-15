#!/usr/bin/env bash
# shell: bash
#
# update-pins-diff.sh - runs the TypeScript update-pins tool and the Rust one (scaffold-release update-pins) against the same fake
# GitHub API, on the same scenarios, and fails if anything they do or print differs. Phase 3 of docs/ts-tooling-rust-port.md: the
# TypeScript version stays as the reference until Phase 6 deletes it, and this script goes with it.
#
# usage: runner/scripts/update-pins-diff.sh <path to the scaffold-release binary>
# Run it from the repository root, with bun and jq on the PATH.
#
# For each scenario, each tool gets its own folder with the same Pins.pkl and its own copy of the fake API
# (runner/scripts/fake-github-api.ts), and is run with --pins, --freshness-out and --summary. The script compares, between the two:
#   - stdout, stderr and the exit code;
#   - Pins.pkl, summary.md and freshness.json after the run (the time in freshness.json is replaced by <time>), and whether
#     each exists, so a file that one tool wrote and the other did not is a difference;
#   - every request the tool made, in order, with the Accept header and the Authorization header (the tokens here are fake test values).
# The token comes from GITHUB_TOKEN or GH_TOKEN, and four scenarios vary which of them is set and whether it is empty.
# Usage errors and an unreadable Pins.pkl are compared on the exit code only: the two name their own command in the usage line and
# word a file error differently. A connection that cannot be made is not a scenario: the two clients word it differently.

set -euo pipefail

binary="${1:?usage: update-pins-diff.sh <path to the scaffold-release binary>}"
binary="$(cd "$(dirname "$binary")" && pwd)/$(basename "$binary")"
[ -x "$binary" ] || { echo "not an executable: $binary" >&2; exit 2; }
repo="$(pwd)"
[ -f "$repo/runner/scripts/update-pins.ts" ] || { echo "run this from the repository root" >&2; exit 2; }
command -v jq > /dev/null || { echo "jq is needed" >&2; exit 2; }

work="$(mktemp -d)"
server_pid=""
stop_server() { [ -n "$server_pid" ] && kill "$server_pid" 2> /dev/null || true; server_pid=""; }
trap 'stop_server; rm -rf "$work"' EXIT
checked=0
failed=0
# The environment the tools get for the token: a token in GITHUB_TOKEN and none in GH_TOKEN, unless a scenario says otherwise.
TOKEN_ARGS=(-u GH_TOKEN GITHUB_TOKEN=token)

rep() { printf "%${2}s" '' | tr ' ' "$1"; }
SHA_A="$(rep a 40)"; SHA_B="$(rep b 40)"; SHA_C="$(rep c 40)"; SHA_D="$(rep d 40)"; SHA_E="$(rep e 40)"; SHA_F="$(rep f 40)"

# --- the inputs ------------------------------------------------------------------------------------------------------------------
pin() { printf '%s: Pin = new { action = "%s"; sha = "%s"; version = "%s" }\n' "$1" "$2" "$3" "$4"; }
pins_file() { printf 'module x\nclass Pin { action: String }\n'; cat; }

# The routes of a healthy repository: <repo> <latest tag> <commit of that tag>.
latest() { printf '"/repos/%s/releases/latest": [{"status":200,"body":{"tag_name":"%s"}}]' "$1" "$2"; }
tag_ref() { printf '"/repos/%s/git/ref/tags/%s": [{"status":200,"body":{"object":{"type":"commit","sha":"%s"}}}]' "$1" "$2" "$3"; }
published() { printf '"/repos/%s/releases/tags/%s": [{"status":200,"body":{"draft":false}}]' "$1" "$2"; }
commit() { printf '"/repos/%s/commits/%s": [{"status":200,"body":{}}]' "$1" "$2"; }
answer() { printf '"%s": %s' "$1" "$2"; }
routes() { local IFS=,; printf '{"routes": {%s}}\n' "$*"; }

# --- running both tools --------------------------------------------------------------------------------------------------------------
start_server() { # <scenario file> <log file>
  rm -f "$work/port"
  : > "$2"
  bun "$repo/runner/scripts/fake-github-api.ts" "$1" "$work/port" "$2" &
  server_pid=$!
  for _ in $(seq 1 100); do [ -s "$work/port" ] && return; sleep 0.1; done
  echo "the fake API did not start" >&2; exit 2
}

# run_tool <name> <folder> <scenario file> <args...>: both tools write into <folder> and are told where it is.
run_tool() {
  local name="$1" folder="$2" scenario="$3"
  shift 3
  start_server "$scenario" "$folder/requests.log"
  set +e
  (cd "$repo" && env "${TOKEN_ARGS[@]}" PANTHEON_SCAFFOLD_GITHUB_API="http://127.0.0.1:$(cat "$work/port")" "$@" > "$work/$name.out" 2> "$work/$name.err")
  echo $? > "$work/$name.code"
  set -e
  stop_server
}

show_file() { if [ -f "$1" ]; then sed "s/^/  | /" "$1"; else echo "  | (absent)"; fi; }

normalise() { # <folder>: the time in the freshness record is not the same for the two runs
  [ -f "$1/freshness.json" ] && sed -E 's/"lastSuccessfulPinCheck": "[^"]*"/"lastSuccessfulPinCheck": "<time>"/' "$1/freshness.json" > "$1/freshness.json.norm" || true
}

# scenario <label> <mode> <pins file> <scenario file> <extra args...>; mode is "exact" or "code" (exit code only).
scenario() {
  local label="$1" mode="$2" pins="$3" routes_file="$4" ok=1
  shift 4
  rm -rf "$work/ts" "$work/rust"
  mkdir -p "$work/ts" "$work/rust"
  cp "$pins" "$work/ts/Pins.pkl"
  cp "$pins" "$work/rust/Pins.pkl"
  local f
  for f in ts rust; do
    local args=(--pins "$work/$f/Pins.pkl" --freshness-out "$work/$f/freshness.json" --summary "$work/$f/summary.md" "$@")
    if [ "$f" = ts ]; then run_tool ts "$work/ts" "$routes_file" bun runner/scripts/update-pins.ts "${args[@]}"
    else run_tool rust "$work/rust" "$routes_file" "$binary" update-pins "${args[@]}"; fi
    normalise "$work/$f"
  done
  checked=$((checked + 1))
  cmp -s "$work/ts.code" "$work/rust.code" || ok=0
  if [ "$mode" = exact ]; then
    # The folders are named in some messages, so the folder names are replaced before the text is compared.
    sed "s#$work/ts#<dir>#g" "$work/ts.out" > "$work/ts.out.norm"; sed "s#$work/rust#<dir>#g" "$work/rust.out" > "$work/rust.out.norm"
    sed "s#$work/ts#<dir>#g" "$work/ts.err" > "$work/ts.err.norm"; sed "s#$work/rust#<dir>#g" "$work/rust.err" > "$work/rust.err.norm"
    cmp -s "$work/ts.out.norm" "$work/rust.out.norm" || ok=0
    cmp -s "$work/ts.err.norm" "$work/rust.err.norm" || ok=0
  fi
  cmp -s "$work/ts/Pins.pkl" "$work/rust/Pins.pkl" || ok=0
  for f in summary.md freshness.json.norm; do
    if [ -f "$work/ts/$f" ] || [ -f "$work/rust/$f" ]; then cmp -s "$work/ts/$f" "$work/rust/$f" || ok=0; fi
  done
  cmp -s "$work/ts/requests.log" "$work/rust/requests.log" || ok=0
  if [ "$ok" = 1 ]; then
    echo "same     $label (exit $(cat "$work/ts.code"), $(wc -l < "$work/ts/requests.log" | tr -d ' ') requests)"
  else
    failed=$((failed + 1))
    echo "DIFFERS  $label"
    for f in code out err; do
      if ! cmp -s "$work/ts.$f" "$work/rust.$f" && { [ "$mode" = exact ] || [ "$f" = code ]; }; then
        echo "  --- $f: typescript"; sed 's/^/  | /' "$work/ts.$f"
        echo "  --- $f: rust"; sed 's/^/  | /' "$work/rust.$f"
      fi
    done
    for f in Pins.pkl summary.md freshness.json.norm requests.log; do
      if ! cmp -s "$work/ts/$f" "$work/rust/$f"; then
        echo "  --- $f: typescript"; show_file "$work/ts/$f"
        echo "  --- $f: rust"; show_file "$work/rust/$f"
      fi
    done
  fi
}

# --- the scenarios -----------------------------------------------------------------------------------------------------------------
two_pins() { { pin checkout actions/checkout "$SHA_A" v1.0.0; pin setup actions/setup-node "$SHA_B" v2.0.0; } | pins_file > "$work/pins.pkl"; }
healthy_setup() { latest actions/setup-node v2.0.0; printf ','; tag_ref actions/setup-node v2.0.0 "$SHA_B"; }
current_checkout() { latest actions/checkout v1.0.0; printf ','; tag_ref actions/checkout v1.0.0 "$SHA_A"; }
moves_checkout() { # <to tag> <to sha>
  latest actions/checkout "$1"; printf ','; published actions/checkout "$1"; printf ','; tag_ref actions/checkout "$1" "$2"; printf ','; commit actions/checkout "$2"
}
r() { routes "$(cat)" > "$work/routes.json"; }

two_pins
{ current_checkout; printf ','; healthy_setup; } | r
scenario "every pin is current: the record is written, the pins are not" exact "$work/pins.pkl" "$work/routes.json"
scenario "the same with --no-write: nothing is written" exact "$work/pins.pkl" "$work/routes.json" --no-write

{ moves_checkout v1.1.0 "$SHA_C"; printf ','; healthy_setup; } | r
scenario "a minor move is written with the record and the summary" exact "$work/pins.pkl" "$work/routes.json"
scenario "a minor move with --no-write: listed, nothing written" exact "$work/pins.pkl" "$work/routes.json" --no-write

{ moves_checkout v2.0.0 "$SHA_C"; printf ','; healthy_setup; } | r
scenario "a major move is flagged in the listing and the summary" exact "$work/pins.pkl" "$work/routes.json"

{ moves_checkout v2.0.0-rc.1 "$SHA_C"; printf ','; healthy_setup; } | r
scenario "a move to a pre-release tag with a dot in it" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout v1.1.0; printf ','; answer /repos/actions/checkout/releases/tags/v1.1.0 '[{"status":404,"body":{}}]'; printf ','; tag_ref actions/checkout v1.1.0 "$SHA_C"; printf ','; healthy_setup; } | r
scenario "the latest tag is not a published release" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout v1.1.0; printf ','; answer /repos/actions/checkout/releases/tags/v1.1.0 '[{"status":200,"body":{"draft":true}}]'; printf ','; healthy_setup; } | r
scenario "the latest release is a draft" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout v1.1.0; printf ','; published actions/checkout v1.1.0; printf ','; tag_ref actions/checkout v1.1.0 "$SHA_C"; printf ','; answer "/repos/actions/checkout/commits/$SHA_C" '[{"status":404,"body":{}}]'; printf ','; healthy_setup; } | r
scenario "the commit does not belong to the repository" exact "$work/pins.pkl" "$work/routes.json"

{ answer /repos/actions/checkout/releases/latest '[{"status":404,"body":{}}]'; printf ','; healthy_setup; } | r
scenario "a repository with no release" exact "$work/pins.pkl" "$work/routes.json"

{ answer /repos/actions/checkout/releases/latest '[{"status":503,"body":{}},{"status":200,"body":{"tag_name":"v1.0.0"}}]'; printf ','; tag_ref actions/checkout v1.0.0 "$SHA_A"; printf ','; healthy_setup; } | r
scenario "a 503 is retried and the next answer is used" exact "$work/pins.pkl" "$work/routes.json"

{ answer /repos/actions/checkout/releases/latest '[{"status":502,"body":{}}]'; printf ','; healthy_setup; } | r
scenario "a 502 that never goes away fails after three attempts and writes nothing" exact "$work/pins.pkl" "$work/routes.json"

{ answer /repos/actions/checkout/releases/latest '[{"status":403,"body":{"message":"rate limit"}}]'; printf ','; healthy_setup; } | r
scenario "a 403 is not retried and is a problem" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout v1.0.0; printf ','; answer /repos/actions/checkout/git/ref/tags/v1.0.0 '[{"status":404,"body":{}}]'; printf ','; healthy_setup; } | r
scenario "the tag of an up to date pin cannot be found" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout v1.0.0; printf ','; tag_ref actions/checkout v1.0.0 "$SHA_F"; printf ','; healthy_setup; } | r
scenario "an up to date pin whose commit is not its tag's commit" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout v0.9.0; printf ','; tag_ref actions/checkout v1.0.0 "$SHA_A"; printf ','; healthy_setup; } | r
scenario "the latest release is older than the pin: nothing moves" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout v1.1.0; printf ','; published actions/checkout v1.1.0; printf ','; answer /repos/actions/checkout/git/ref/tags/v1.1.0 '[{"status":200,"body":{"object":{"type":"tag","sha":"t1"}}}]'; printf ','; answer /repos/actions/checkout/git/tags/t1 '[{"status":200,"body":{"object":{"type":"tag","sha":"t2"}}}]'; printf ','; answer /repos/actions/checkout/git/tags/t2 "[{\"status\":200,\"body\":{\"object\":{\"type\":\"commit\",\"sha\":\"$SHA_D\"}}}]"; printf ','; commit actions/checkout "$SHA_D"; printf ','; healthy_setup; } | r
scenario "an annotated tag is followed to its commit" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout v1.1.0; printf ','; published actions/checkout v1.1.0; printf ','; answer /repos/actions/checkout/git/ref/tags/v1.1.0 '[{"status":200,"body":{"object":{"type":"tree","sha":"x"}}}]'; printf ','; healthy_setup; } | r
scenario "a tag that points to a tree" exact "$work/pins.pkl" "$work/routes.json"

{ latest actions/checkout nightly; printf ','; healthy_setup; } | r
scenario "the latest tag is not a version" exact "$work/pins.pkl" "$work/routes.json"

{ pin checkout actions/checkout "$SHA_A" main; pin setup actions/setup-node "$SHA_B" v2.0.0; } | pins_file > "$work/pins-main.pkl"
{ latest actions/checkout v1.0.0; printf ','; healthy_setup; } | r
scenario "the pinned version is not a version" exact "$work/pins-main.pkl" "$work/routes.json"

{ moves_checkout v1.1.0 "$SHA_C"; printf ','; answer /repos/actions/setup-node/releases/latest '[{"status":404,"body":{}}]'; } | r
scenario "one pin moves and another has a problem: nothing is written" exact "$work/pins.pkl" "$work/routes.json"

{ pin nested owner/repo/.github/workflows/x.yml "$SHA_A" v3.0.0; pin setup actions/setup-node "$SHA_B" v2.0.0; } | pins_file > "$work/pins-nested.pkl"
{ latest owner/repo v3.1.0; printf ','; published owner/repo v3.1.0; printf ','; tag_ref owner/repo v3.1.0 "$SHA_E"; printf ','; commit owner/repo "$SHA_E"; printf ','; healthy_setup; } | r
scenario "an action with a workflow path in its name uses the repository part" exact "$work/pins-nested.pkl" "$work/routes.json"

printf 'module x\n' > "$work/pins-none.pkl"
routes > "$work/routes-empty.json"
scenario "a Pins.pkl with no pins checks zero and writes the record" exact "$work/pins-none.pkl" "$work/routes-empty.json"

{ current_checkout; printf ','; healthy_setup; } | r
TOKEN_ARGS=(-u GITHUB_TOKEN -u GH_TOKEN)
scenario "no token at all: no Authorization header is sent" exact "$work/pins.pkl" "$work/routes.json"
TOKEN_ARGS=(-u GITHUB_TOKEN GH_TOKEN=from-gh-token)
scenario "GH_TOKEN alone is used" exact "$work/pins.pkl" "$work/routes.json"
TOKEN_ARGS=(GITHUB_TOKEN= GH_TOKEN=from-gh-token)
scenario "an empty GITHUB_TOKEN ends the search: no header, and GH_TOKEN is not tried" exact "$work/pins.pkl" "$work/routes.json"
TOKEN_ARGS=(GITHUB_TOKEN=first GH_TOKEN=second)
scenario "both set: GITHUB_TOKEN is used" exact "$work/pins.pkl" "$work/routes.json"
TOKEN_ARGS=(-u GH_TOKEN GITHUB_TOKEN=token)

# --- usage errors and an unreadable file: the exit code is compared ---------------------------------------------------------------
scenario "usage: an unknown option (exit code only)" code "$work/pins.pkl" "$work/routes-empty.json" --nope
scenario "usage: --summary with no value (exit code only)" code "$work/pins.pkl" "$work/routes-empty.json" --summary
printf 'module x\nbad: Pin = new { action = "a/b"; sha = "short"; version = "v1" }\n' > "$work/pins-bad.pkl"
scenario "a line of Pins.pkl that cannot be read (exit code only)" code "$work/pins-bad.pkl" "$work/routes-empty.json"

echo
echo "$checked comparisons, $failed differ"
[ "$failed" = 0 ]
