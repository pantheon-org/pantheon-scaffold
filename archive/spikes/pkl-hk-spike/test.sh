#!/usr/bin/env bash
# Tests the hk-style config in a throwaway copy. Usage: bash tmp/pkl-hk-spike/test.sh
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
git -C "$ADR" show "$REF":.github/workflows/cargo-deny.yml > "$W/adr-cargo-deny.yml"

cp "$SPIKE"/{Config.pkl,Builtins.pkl,pantheon.pkl,runner.ts,compare.ts} "$W/"
cd "$W"
mkdir target

failures=0
ok() { echo "PASS $1"; }
no() { echo "FAIL $1"; failures=$((failures + 1)); }
run() { bun runner.ts pantheon.pkl target; }
tree() { (cd target && find . -type f -not -path './.pkl-gen/*' | sort | xargs shasum -a 256 | shasum -a 256); }

run > /dev/null

# 1. Builtin equals pantheon-adr semantically, with exactly one known difference:
#    upstream has `workflow_dispatch:` (null), pkl's YamlRenderer cannot emit it, so we render `{}`.
set +e
diff_out="$(bun compare.ts adr-cargo-deny.yml target/.github/workflows/cargo-deny.yml)"
diff_count=$?
set -e
if [ "$diff_count" -eq 1 ] && [ "$diff_out" = 'DIFF $.on.workflow_dispatch: expected null, got {}' ]; then
  ok "1 builtin cargoDeny equals pantheon-adr except the known workflow_dispatch null vs {}"
else
  no "1 builtin cargoDeny differs: $diff_out"
fi

# 2. Custom workflow is rendered.
if [ -f target/.github/workflows/docs-check.yml ] && grep -q '^name: Docs Check' target/.github/workflows/docs-check.yml; then
  ok "2 custom workflow rendered"
else
  no "2 custom workflow rendered"
fi

# 3. Builtin override: a different toolchain flows into the rendered workflow.
sed 's/local toolchain = "1.96.1"/local toolchain = "1.97.0"/' pantheon.pkl > pantheon.pkl.new && mv pantheon.pkl.new pantheon.pkl
run > /dev/null
if grep -q 'rustup toolchain install 1.97.0' target/.github/workflows/cargo-deny.yml && ! grep -q '1.96.1' target/.github/workflows/cargo-deny.yml; then
  ok "3 builtin override applied"
else
  no "3 builtin override applied"
fi

# 4. Seed once.
echo "user edit" >> target/README.md
run > /dev/null
if [ "$(tail -1 target/README.md)" = "user edit" ]; then ok "4 seeded README kept"; else no "4 seeded README kept"; fi

# 5. Second run is a no-op.
before="$(tree)"
out="$(run)"
after="$(tree)"
if [ "$before" = "$after" ] && echo "$out" | grep -q '"written":0,.*"deleted":0'; then ok "5 second run is a no-op"; else no "5 second run is a no-op"; fi

# 6. Removing the custom workflow from the config deletes its file.
cat > pantheon.pkl <<'EOF'
amends "Config.pkl"

import "Builtins.pkl"

workflows {
  ["cargo-deny"] = Builtins.cargoDeny("1.97.0")
}
EOF
run > /dev/null
if [ ! -e target/.github/workflows/docs-check.yml ] && [ -f target/.github/workflows/cargo-deny.yml ]; then
  ok "6 removed custom workflow is deleted, builtin stays"
else
  no "6 removed custom workflow is deleted, builtin stays"
fi

echo "failures: $failures"
exit "$failures"
