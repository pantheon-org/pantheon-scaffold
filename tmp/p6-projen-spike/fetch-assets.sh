#!/usr/bin/env bash
# Fetches the spike's fixtures fresh from pantheon-org/pantheon-adr main (read-only, gh api) into the directory given as $1,
# replacing the toolchain version with @@TOOLCHAIN@@ in the two files that carry it. Prints the toolchain and the commit.
set -eu
OUT="$1"; mkdir -p "$OUT"
REPO=pantheon-org/pantheon-adr
up() { env -u GH_TOKEN gh api "repos/$REPO/contents/$1" --jq .content | base64 -d; }
T="$(up rust-toolchain.toml | sed -n 's/^channel = "\(.*\)"/\1/p')"
up rust-toolchain.toml | sed "s/$T/@@TOOLCHAIN@@/g" > "$OUT/rust-toolchain.toml.tmpl"
up .github/workflows/cargo-deny.yml | sed "s/$T/@@TOOLCHAIN@@/g" > "$OUT/cargo-deny.yml.tmpl"
up deny.toml > "$OUT/deny.toml"
up .github/scripts/cargo-deny.sh > "$OUT/cargo-deny.sh"
up .github/scripts/agnix-introduced-errors.sh > "$OUT/agnix-introduced-errors.sh"
echo "$T $(env -u GH_TOKEN gh api repos/$REPO/commits/main --jq .sha[0:7])"
