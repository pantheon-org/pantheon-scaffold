#!/usr/bin/env bash
# Fetches the pinned YAML test suite data (yaml/yaml-test-suite, commit 6ad3d2c, 2022-01-17) into the directory given as $1.
# The directory must not exist yet or be empty. Read-only (gh api). Usage: bash fetch-suite.sh <dir>
set -eu
D="${1:?usage: bash fetch-suite.sh <dir>}"
mkdir -p "$D"
[ -z "$(ls -A "$D")" ] || { echo "$D is not empty" >&2; exit 2; }
env -u GH_TOKEN gh api repos/yaml/yaml-test-suite/tarball/6ad3d2c62885d82fc349026c136ef560838fdf3d > "$D/suite.tar.gz"
tar -xzf "$D/suite.tar.gz" -C "$D" --strip-components=1
echo "cases: $(find "$D" -name in.yaml | wc -l)"
