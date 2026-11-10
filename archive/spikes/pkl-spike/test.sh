#!/usr/bin/env bash
# Runs the pkl spike assertions in a throwaway copy. Usage: bash tmp/pkl-spike/test.sh
# Compares against pantheon-adr at the pinned commit 060531b (the state the spike was written against), set UPSTREAM_REF=origin/main to compare with the current main (expected to fail once upstream has moved), and ADR_REPO to its clone path if not the default.
set -euo pipefail

SPIKE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADR="${ADR_REPO:-$HOME/Projects/github/pantheon-org/pantheon-adr}"

W="$(mktemp -d)"
[ -d "$W" ] || { echo "mktemp failed" >&2; exit 2; }
trap 'rm -rf "$W"' EXIT

REF="${UPSTREAM_REF:-060531b}"
git -C "$ADR" fetch origin --prune -q
echo "pantheon-adr at $REF: $(git -C "$ADR" rev-parse --short "$REF")"

cp -R "$SPIKE/template.pkl" "$SPIKE/pantheon.pkl" "$SPIKE/runner.ts" "$SPIKE/assets" "$W/"
cd "$W"
mkdir target

failures=0
ok() { echo "PASS $1"; }
no() { echo "FAIL $1"; failures=$((failures + 1)); }
run() { bun runner.ts pantheon.pkl target; }
tree() { (cd target && find . -type f -not -path './.pkl-gen/*' | sort | xargs shasum -a 256 | shasum -a 256); }
set_agnix() { sed "s/agnix = $1/agnix = $2/" pantheon.pkl > pantheon.pkl.new && mv pantheon.pkl.new pantheon.pkl; }
upstream() { git -C "$ADR" show "$REF:$1"; }

run > /dev/null
for p in rust-toolchain.toml deny.toml .github/workflows/cargo-deny.yml; do
  if diff -q <(upstream "$p") "target/$p" > /dev/null; then ok "1 identical: $p"; else no "1 identical: $p"; fi
done

echo "user edit" >> target/README.md
echo "# tamper" >> target/deny.toml
run > /dev/null
if [ "$(tail -1 target/README.md)" = "user edit" ]; then ok "2 seeded README kept"; else no "2 seeded README kept"; fi
if diff -q <(upstream deny.toml) target/deny.toml > /dev/null; then ok "2 owned file restored"; else no "2 owned file restored"; fi

set_agnix false true
run > /dev/null
if [ -x target/.github/scripts/agnix-introduced-errors.sh ]; then ok "3 toggle on adds file, 4 executable"; else no "3/4 toggle on"; fi

set_agnix true false
run > /dev/null
if [ ! -e target/.github/scripts/agnix-introduced-errors.sh ]; then ok "3 toggle off deletes orphan"; else no "3 toggle off deletes orphan"; fi

if [ -x target/.github/scripts/cargo-deny.sh ] && [ ! -x target/deny.toml ]; then ok "4 modes correct"; else no "4 modes correct"; fi

before="$(tree)"
out="$(run)"
after="$(tree)"
if [ "$before" = "$after" ] && echo "$out" | grep -q '"written":0,.*"deleted":0'; then ok "5 second run is a no-op"; else no "5 second run is a no-op"; fi

echo "failures: $failures"
exit "$failures"
