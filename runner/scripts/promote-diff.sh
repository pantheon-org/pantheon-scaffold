#!/usr/bin/env bash
# shell: bash
#
# promote-diff.sh - runs the TypeScript promote tool and the Rust one (scaffold-release promote) against the same fake `gh`, on the
# same scenarios, and fails if anything they do or print differs. Phase 2 of docs/ts-tooling-rust-port.md: the TypeScript version stays
# as the reference until Phase 6 deletes it, and this script goes with it.
#
# usage: runner/scripts/promote-diff.sh <path to the scaffold-release binary>
# Run it from the repository root, with bun and jq on the PATH.
#
# The fake `gh` answers `release view`, `release download`, `attestation verify`, `run list`, `run view`, `api .../releases/latest` and
# `release edit` from a folder of files (one folder per scenario, one copy for each tool), records every call, and applies an edit to
# the folder the way GitHub would. For each scenario the script compares, between the two tools:
#   - stdout, stderr and the exit code;
#   - the calls made to gh, one argument per line, in order (the private download folder is replaced by <work>);
#   - the folder at the end, so an edit that one tool made and the other did not is a difference.
# Usage errors are compared on the exit code only, because the two name their own command in the message. Both tools read
# known-bad.json from the checkout, which is the real one here: the known-bad scenario uses a version it lists.

set -euo pipefail

binary="${1:?usage: promote-diff.sh <path to the scaffold-release binary>}"
binary="$(cd "$(dirname "$binary")" && pwd)/$(basename "$binary")"
[ -x "$binary" ] || { echo "not an executable: $binary" >&2; exit 2; }
repo="$(pwd)"
[ -f "$repo/runner/scripts/promote.ts" ] || { echo "run this from the repository root" >&2; exit 2; }
command -v jq > /dev/null || { echo "jq is needed" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
checked=0
failed=0

sha() {
  if command -v sha256sum > /dev/null 2>&1; then sha256sum | cut -d' ' -f1; else shasum -a 256 | cut -d' ' -f1; fi
}

# --- the fake gh -----------------------------------------------------------------------------------------------------------------
mkdir -p "$work/bin"
cat > "$work/bin/gh" <<'FAKE'
#!/usr/bin/env bash
# The fake gh. $GH_FAKE_DIR is the scenario folder.
d="$GH_FAKE_DIR"
{ printf '%s\n' "$@"; echo '---'; } >> "$d/gh.log"
fail_with() { [ -f "$d/fail_$1" ] && { cat "$d/fail_$1" >&2; exit 1; } || true; }
case "$1 $2" in
  "release view")
    fail_with view
    cat "$d/release.json" ;;
  "release download")
    fail_with download
    pattern=""; into=""
    shift 2
    while [ $# -gt 0 ]; do
      case "$1" in --pattern) pattern="$2" ;; --dir) into="$2" ;; esac
      shift
    done
    if [ ! -f "$d/assets/$pattern" ]; then echo "HTTP 404: no asset $pattern" >&2; exit 1; fi
    cp "$d/assets/$pattern" "$into/$pattern" ;;
  "attestation verify")
    name="$(basename "$3")"
    if [ -f "$d/attest_fail" ] && [ "$(cat "$d/attest_fail")" = "$name" ]; then echo "no valid attestation for $name" >&2; exit 1; fi ;;
  "run list")
    fail_with runlist
    cat "$d/runlist.json" ;;
  "run view")
    cat "$d/runview.json" ;;
  "api repos/o/r/releases/latest")
    [ -f "$d/latest" ] || { echo "HTTP 404" >&2; exit 1; }
    cat "$d/latest" ;;
  "release edit")
    fail_with edit
    tag="$3"; title=""; notes=""
    shift 3
    while [ $# -gt 0 ]; do
      case "$1" in --title) title="$2" ;; --notes) notes="$2" ;; esac
      shift
    done
    jq --arg t "$title" --arg n "$notes" '.isPrerelease = false | .name = $t | .body = $n' "$d/release.json" > "$d/release.json.new"
    mv "$d/release.json.new" "$d/release.json"
    printf '%s' "$tag" > "$d/latest"
    case "$(cat "$d/edit_mode" 2> /dev/null || echo normal)" in
      still_prerelease) jq '.isPrerelease = true' "$d/release.json" > "$d/release.json.new"; mv "$d/release.json.new" "$d/release.json" ;;
      not_latest) printf '%s' 'v9.9.9' > "$d/latest" ;;
      drop_latest) rm -f "$d/latest" ;;
      changed_checksums) printf '\n' >> "$d/assets/checksums.txt" ;;
    esac ;;
  *) echo "the fake gh does not know: $*" >&2; exit 3 ;;
esac
FAKE
chmod +x "$work/bin/gh"

# make_world <dir> <version>: a pre-release as the release workflow publishes it.
make_world() {
  local dir="$1" version="$2" pkg="pantheon-scaffold@$2" name
  mkdir -p "$dir/assets"
  : > "$dir/gh.log"
  local names=(checksums.txt pantheon-scaffold-darwin-arm64 pantheon-scaffold-linux-arm64 pantheon-scaffold-linux-x64 "$pkg" "$pkg.sha256" "$pkg.zip" "$pkg.zip.sha256")
  for name in "${names[@]}"; do [ "$name" = checksums.txt ] || printf 'bytes of %s' "$name" > "$dir/assets/$name"; done
  : > "$dir/assets/checksums.txt"
  for name in pantheon-scaffold-darwin-arm64 pantheon-scaffold-linux-arm64 pantheon-scaffold-linux-x64 "$pkg" "$pkg.zip"; do
    printf '%s  %s\n' "$(sha < "$dir/assets/$name")" "$name" >> "$dir/assets/checksums.txt"
  done
  local list
  list="$(printf '%s\n' "${names[@]}" | jq -R '{name: .}' | jq -s '.')"
  jq -n --arg tag "v$version" --argjson assets "$list" \
    '{tagName: $tag, name: "\($tag) (pre-release)", body: "Pre-release of the package and the runner, released together.", isPrerelease: true, isDraft: false, assets: $assets}' > "$dir/release.json"
  echo '[{"databaseId":7,"status":"completed","conclusion":"success"}]' > "$dir/runlist.json"
  echo '{"jobs":[{"name":"Attest and publish","conclusion":"success"},{"name":"Verify the published release on ubuntu-latest","conclusion":"success"}]}' > "$dir/runview.json"
  printf '%s' 'v0.0.1' > "$dir/latest"
}

# --- running both tools ------------------------------------------------------------------------------------------------------------
# run_one <name> <folder> <ref> <command...>: runs a tool in the repository root with the fake gh first on the PATH.
run_one() {
  local name="$1" folder="$2" ref="$3"
  shift 3
  set +e
  (cd "$repo" && PATH="$work/bin:$PATH" GH_FAKE_DIR="$folder" GH_REPO=o/r GITHUB_REF="$ref" "$@" > "$work/$name.out" 2> "$work/$name.err")
  echo $? > "$work/$name.code"
  set -e
}

normalise_log() {
  sed -E 's#.*/promote-[A-Za-z0-9]+/(dl|at)-[^/]*#<work>/\1-N#' "$1"
}

# scenario <label> <ref> <mode> <tag> <setup function>: builds the world for <tag>, lets the function change it, runs both tools on a copy each.
scenario() {
  local label="$1" ref="$2" mode="$3" tag="$4" setup="$5" ok=1
  rm -rf "$work/w-ts" "$work/w-rust"
  make_world "$work/w-ts" "${tag#v}"
  "$setup" "$work/w-ts"
  cp -R "$work/w-ts" "$work/w-rust"
  run_one ts "$work/w-ts" "$ref" bun runner/scripts/promote.ts "$mode" "$tag"
  run_one rust "$work/w-rust" "$ref" "$binary" promote "$mode" "$tag"
  checked=$((checked + 1))
  cmp -s "$work/ts.code" "$work/rust.code" || ok=0
  cmp -s "$work/ts.out" "$work/rust.out" || ok=0
  cmp -s "$work/ts.err" "$work/rust.err" || ok=0
  normalise_log "$work/w-ts/gh.log" > "$work/ts.log"
  normalise_log "$work/w-rust/gh.log" > "$work/rust.log"
  cmp -s "$work/ts.log" "$work/rust.log" || ok=0
  diff -r -q --exclude=gh.log "$work/w-ts" "$work/w-rust" > "$work/state.diff" 2>&1 || ok=0
  if [ "$ok" = 1 ]; then
    echo "same     $label (exit $(cat "$work/ts.code"), $(grep -c '^---$' "$work/ts.log") gh calls)"
  else
    failed=$((failed + 1))
    echo "DIFFERS  $label"
    for f in code out err log; do
      if ! cmp -s "$work/ts.$f" "$work/rust.$f"; then
        echo "  --- $f: typescript"; sed 's/^/  | /' "$work/ts.$f"
        echo "  --- $f: rust"; sed 's/^/  | /' "$work/rust.$f"
      fi
    done
    if [ -s "$work/state.diff" ]; then
      echo "  --- the folders differ at the end:"
      sed "s/^/  | /" "$work/state.diff"
    fi
  fi
}

# --- the scenarios: each one changes a world that would otherwise pass -------------------------------------------------------------
none() { :; }
stable() { jq '.isPrerelease = false' "$1/release.json" > "$1/x" && mv "$1/x" "$1/release.json"; }
draft() { jq '.isDraft = true' "$1/release.json" > "$1/x" && mv "$1/x" "$1/release.json"; }
missing_asset() { jq '.assets |= map(select(.name != "pantheon-scaffold-linux-x64"))' "$1/release.json" > "$1/x" && mv "$1/x" "$1/release.json"; }
extra_asset() { jq '.assets += [{name: "surprise.bin"}]' "$1/release.json" > "$1/x" && mv "$1/x" "$1/release.json"; }
bad_attestation() { printf '%s' checksums.txt > "$1/attest_fail"; }
tampered_binary() { printf 'tampered' > "$1/assets/pantheon-scaffold-darwin-arm64"; }
ghost_in_checksums() { printf '%s  ghost-file\n' "$(printf 0 | sha)" >> "$1/assets/checksums.txt"; }
garbage_in_checksums() { printf 'garbage\n' >> "$1/assets/checksums.txt"; }
repeated_in_checksums() { local first; first="$(head -1 "$1/assets/checksums.txt")"; printf "%s\n" "$first" >> "$1/assets/checksums.txt"; }
no_run() { echo '[]' > "$1/runlist.json"; }
run_not_finished() { echo '[{"databaseId":7,"status":"in_progress","conclusion":""}]' > "$1/runlist.json"; }
run_failed() { echo '[{"databaseId":7,"status":"completed","conclusion":"failure"}]' > "$1/runlist.json"; }
job_failed() { echo '{"jobs":[{"name":"Attest and publish","conclusion":"success"},{"name":"Verify the published release on ubuntu-latest","conclusion":"failure"}]}' > "$1/runview.json"; }
no_verify_job() { echo '{"jobs":[{"name":"Attest and publish","conclusion":"success"}]}' > "$1/runview.json"; }
gh_view_fails() { echo 'HTTP 502: bad gateway' > "$1/fail_view"; }
gh_run_list_fails() { echo 'HTTP 500' > "$1/fail_runlist"; }
asset_not_downloadable() { rm "$1/assets/pantheon-scaffold-linux-x64"; }
edit_fails() { echo 'HTTP 403: forbidden' > "$1/fail_edit"; }
edit_leaves_prerelease() { echo still_prerelease > "$1/edit_mode"; }
edit_not_latest() { echo not_latest > "$1/edit_mode"; }
edit_no_latest() { echo drop_latest > "$1/edit_mode"; }
edit_changes_checksums() { echo changed_checksums > "$1/edit_mode"; }
no_latest_before() { rm "$1/latest"; }

MAIN=refs/heads/main
scenario "check: every guard passes" $MAIN check v0.1.5 none
scenario "check: a release that is already stable" $MAIN check v0.1.5 stable
scenario "check: a draft" $MAIN check v0.1.5 draft
scenario "check: an asset is missing from the release" $MAIN check v0.1.5 missing_asset
scenario "check: an extra asset" $MAIN check v0.1.5 extra_asset
scenario "check: checksums.txt has no valid attestation" $MAIN check v0.1.5 bad_attestation
scenario "check: an asset does not match checksums.txt" $MAIN check v0.1.5 tampered_binary
scenario "check: checksums.txt lists a file the release does not carry" $MAIN check v0.1.5 ghost_in_checksums
scenario "check: checksums.txt has a line that is not a checksum" $MAIN check v0.1.5 garbage_in_checksums
scenario "check: checksums.txt names a file twice" $MAIN check v0.1.5 repeated_in_checksums
scenario "check: a version on the known-bad list" $MAIN check v0.1.0 none
scenario "check: no run of the release workflow" $MAIN check v0.1.5 no_run
scenario "check: the release run is not finished" $MAIN check v0.1.5 run_not_finished
scenario "check: the release run failed" $MAIN check v0.1.5 run_failed
scenario "check: a job of the release run failed" $MAIN check v0.1.5 job_failed
scenario "check: the release run has no verify job" $MAIN check v0.1.5 no_verify_job
scenario "check: run from a branch" refs/heads/feat/x check v0.1.5 none
scenario "check: run from a tag" refs/tags/v0.1.5 check v0.1.5 none
scenario "check: no ref at all" "" check v0.1.5 none
scenario "check: a tag that is not a release tag" $MAIN check latest none
scenario "check: gh release view fails" $MAIN check v0.1.5 gh_view_fails
scenario "check: gh run list fails" $MAIN check v0.1.5 gh_run_list_fails
scenario "check: an asset cannot be downloaded" $MAIN check v0.1.5 asset_not_downloadable
scenario "promote: every guard passes and the release is edited" $MAIN promote v0.1.5 none
scenario "promote: the same, with no latest release before" $MAIN promote v0.1.5 no_latest_before
scenario "promote: a guard refuses, nothing is edited" $MAIN promote v0.1.5 tampered_binary
scenario "promote: run from a branch" refs/heads/feat/x promote v0.1.5 none
scenario "promote: gh release edit fails" $MAIN promote v0.1.5 edit_fails
scenario "promote: the release is still a pre-release after the edit" $MAIN promote v0.1.5 edit_leaves_prerelease
scenario "promote: the release is not the latest after the edit" $MAIN promote v0.1.5 edit_not_latest
scenario "promote: there is no latest release after the edit" $MAIN promote v0.1.5 edit_no_latest
scenario "promote: checksums.txt changed in the edit" $MAIN promote v0.1.5 edit_changes_checksums

# --- usage errors: the exit code is compared, and not the usage line --------------------------------------------------------------
usage_cases=("" "check" "promote" "nope v0.1.5")
for args in "${usage_cases[@]}"; do
  make_world "$work/w-ts" 0.1.5
  # shellcheck disable=SC2086
  run_one ts "$work/w-ts" "$MAIN" bun runner/scripts/promote.ts $args
  # shellcheck disable=SC2086
  run_one rust "$work/w-ts" "$MAIN" "$binary" promote $args
  checked=$((checked + 1))
  if cmp -s "$work/ts.code" "$work/rust.code" && [ ! -s "$work/ts.out" ] && [ ! -s "$work/rust.out" ]; then
    echo "same     usage error: '$args' (exit code only) (exit $(cat "$work/ts.code"))"
  else
    failed=$((failed + 1))
    echo "DIFFERS  usage error: '$args' (typescript exit $(cat "$work/ts.code"), rust exit $(cat "$work/rust.code"))"
  fi
  rm -rf "$work/w-ts"
done

echo
echo "$checked comparisons, $failed differ"
[ "$failed" = 0 ]
