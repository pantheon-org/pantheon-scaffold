#!/usr/bin/env bash
# Phase 3 wave C spike: does hk accept a seeded hk.pkl that imports a rendered local module (decision 31)?
# Needs hk, pkl and network for the hk package on first use. rumdl is run through mise (rumdl@0.2.77 is used if installed).
set -euo pipefail
dir="$(mktemp -d "${TMPDIR:-/tmp}/hk-spike-XXXXXX")"
trap 'rm -rf "$dir"' EXIT
cd "$dir"
git init -q -b main
HK=package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0
cat > hk.shared.pkl <<PKL
module pantheon.hk.shared
import "$HK#/Config.pkl" as hk
function preCommit(): Mapping<String, hk.Step> = new Mapping<String, hk.Step> {
    ["rustfmt"] { glob = List("*.rs"); check = "cargo fmt --all --check"; fix = "cargo fmt --all" }
    ["rumdl"] { glob = List("*.md"); check = "rumdl check --config .rumdl.toml {{files}}"; fix = "rumdl check --fix --config .rumdl.toml {{files}}" }
}
PKL
cat > hk.pkl <<PKL
amends "$HK#/Config.pkl"
import "hk.shared.pkl" as shared
local mine = new Mapping<String, Step> { ["my-local-step"] { glob = List("*.txt"); check = "true" } }
hooks {
    ["check"] { steps { ...shared.preCommit(); ...mine } }
}
PKL
printf '# Title\n\nText.\n' > README.md
printf '[global]\ndisable = ["MD013"]\n' > .rumdl.toml
git add -A
echo "--- hk validate"; hk validate
echo "--- plan: the steps hk will run, shared and local"; hk check --all --plan 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E "rustfmt|rumdl|my-local-step"
echo "--- run"; mise x rumdl@0.2.77 -- hk check --all 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | tail -3
