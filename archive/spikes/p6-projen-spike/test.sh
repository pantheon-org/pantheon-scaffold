#!/usr/bin/env bash
# P.6 projen spike: the nine assertions of tmp/pkl-spike/test.sh, run for both styles in a throwaway copy.
# Upstream files are read from pantheon-org/pantheon-adr main with gh api (read-only). Usage: bash test.sh
set -u
SPIKE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO=pantheon-org/pantheon-adr
up() { env -u GH_TOKEN gh api "repos/$REPO/contents/$1" --jq .content | base64 -d; }
[ -d "$SPIKE/node_modules/projen" ] || { echo "run npm install in $SPIKE first" >&2; exit 2; }
TOOLCHAIN="$(up rust-toolchain.toml | sed -n 's/^channel = "\(.*\)"/\1/p')"; echo "pantheon-adr main: $(env -u GH_TOKEN gh api repos/$REPO/commits/main --jq .sha[0:7])"
echo "upstream toolchain: $TOOLCHAIN, projen $(node -p "require('$SPIKE/node_modules/projen/package.json').version")"
failures=0
ok() { echo "PASS $1"; }
no() { echo "FAIL $1"; failures=$((failures + 1)); }

for style in verbatim typed; do
  echo "=== style: $style"
  W="$(mktemp -d)"; [ -d "$W" ] || exit 2
  cp -R "$SPIKE/.projenrc.ts" "$SPIKE/package.json" "$W/"; bash "$SPIKE/fetch-assets.sh" "$W/assets" > /dev/null; ln -s "$SPIKE/node_modules" "$W/node_modules"
  cd "$W" || exit 2
  synth() { STYLE=$style AGNIX=${1:-off} TOOLCHAIN=$TOOLCHAIN bun .projenrc.ts > /dev/null 2>&1; }
  tree() { find . -type f -not -path './node_modules/*' -not -path './assets/*' | sort | xargs shasum -a 256 | shasum -a 256; }

  synth off
  for p in rust-toolchain.toml deny.toml .github/workflows/cargo-deny.yml; do
    if diff -q <(up "$p") "$p" > /dev/null; then ok "1 identical: $p"; else
      no "1 identical: $p"
      echo "   byte diff lines: $(diff <(up "$p") "$p" | grep -c '^[<>]')"
      [ "${p##*.}" = yml ] && { echo "   parsed-data differences:"; bun "$SPIKE/compare.ts" <(up "$p") "$p" | sed 's/^/     /'; }
    fi
  done

  echo "   modes: $(stat -c '%a' deny.toml) deny.toml, $(stat -c '%a' .github/workflows/cargo-deny.yml) cargo-deny.yml (generated files are read-only if 444)"
  echo "   first line of cargo-deny.yml: $(head -1 .github/workflows/cargo-deny.yml)"
  echo "   projen files created: $(find . -type f -not -path './node_modules/*' -not -path './assets/*' -path './.projen/*' | sort | tr '\n' ' ')"

  echo "user edit" >> README.md
  chmod u+w deny.toml; echo "# tamper" >> deny.toml
  synth off
  if [ "$(tail -1 README.md)" = "user edit" ]; then ok "2 seeded README kept"; else no "2 seeded README kept"; fi
  if diff -q <(up deny.toml) deny.toml > /dev/null; then ok "2 owned file restored (hand edit silently overwritten, no refusal)"; else no "2 owned file restored"; fi

  synth on
  if [ -x .github/scripts/agnix-introduced-errors.sh ]; then ok "3 toggle on adds file, 4 executable"; else no "3/4 toggle on"; fi
  synth off
  if [ ! -e .github/scripts/agnix-introduced-errors.sh ]; then ok "3 toggle off deletes orphan"; else no "3 toggle off deletes orphan"; fi
  if [ -x .github/scripts/cargo-deny.sh ] && [ ! -x deny.toml ]; then ok "4 modes correct"; else no "4 modes correct"; fi

  before="$(tree)"; synth off; after="$(tree)"
  if [ "$before" = "$after" ]; then ok "5 second run is a no-op"; else no "5 second run is a no-op"; fi
  cd "$SPIKE" && rm -rf "$W"
done
echo "failures: $failures (a failure in the typed style's byte identity is a measured difference, see README)"
exit "$failures"
