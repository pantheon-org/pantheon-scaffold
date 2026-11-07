#!/usr/bin/env bash
# Builds the spike runner for the three plan targets. Run from this folder: bash build.sh
set -eu
rm -rf dist && mkdir dist
for t in darwin-arm64 linux-x64 linux-arm64; do
  bun build --compile --target="bun-$t" src/runner.ts --outfile "dist/pantheon-runner-$t" >/dev/null
  printf '%s  %s bytes\n' "pantheon-runner-$t" "$(wc -c < "dist/pantheon-runner-$t" | tr -d ' ')"
done
file dist/*
