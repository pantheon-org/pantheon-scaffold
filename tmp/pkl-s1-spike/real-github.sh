#!/usr/bin/env bash
# GitHub half of S.1: render the published spike releases through the real package:// URI with fresh pkl caches.
# Run from anywhere: bash real-github.sh. Needs network. Checksums are the metadata-file checksums of the releases.
set -u
W=$(mktemp -d) || exit 1
[ -d "$W" ] || exit 1
cd "$W" || exit 1
B=package://github.com/pantheon-org/pantheon-scaffold-spike/releases/download
STABLE=ff2a095f0acd979aabac8167f22cbacf4592e555014ef4a459c0f6224de93030
RC=7a100d3eca55809b9fd649bba3f3e152019b79a656441d41b57ee85817c96677
mk() { printf 'amends "%s/v%s/pantheon-scaffold-spike@%s::sha256:%s#/Config.pkl"\ntoolchain = "1.91.0"\n' "$B" "$1" "$1" "$2" > "$3.pkl"; }
mk 0.0.1 "$STABLE" stable
mk 0.0.2-rc.1 "$RC" rc
mk 0.0.1 "$(printf '0%.0s' $(seq 64))" wrong
fail=0
check() { if [ "$2" = ok ]; then echo "PASS  $1"; else echo "FAIL  $1"; fail=1; fi; }
out=$(pkl eval --cache-dir c1 -x output.text stable.pkl 2>&1); [[ $out == *"toolchain = 1.91.0"* ]] && check "stable release renders with the inline checksum, fresh cache" ok || check "stable release renders: $out" no
out=$(pkl eval --cache-dir c2 -x output.text rc.pkl 2>&1); [[ $out == *"toolchain = 1.91.0"* ]] && check "pre-release renders by exact pin, fresh cache" ok || check "pre-release renders: $out" no
out=$(pkl eval --cache-dir c3 -x output.text wrong.pkl 2>&1); [[ $out == *"checksum"* ]] && check "a wrong inline checksum is rejected" ok || check "wrong checksum rejected: $out" no
out=$(pkl eval --cache-dir c4 --allowed-modules 'pkl:,file:,package:' --allowed-resources 'package:,https:' -x output.text stable.pkl 2>&1); [[ $out == *"toolchain = 1.91.0"* ]] && check "restricted flags (modules pkl:,file:,package: and resources package:,https:) still fetch" ok || check "restricted flags: $out" no
out=$(pkl eval --cache-dir c1 --http-proxy http://127.0.0.1:9 -x output.text stable.pkl 2>&1); [[ $out == *"toolchain = 1.91.0"* ]] && check "warm cache renders with the network blocked" ok || check "warm cache offline: $out" no
out=$(pkl eval --cache-dir c5 --http-proxy http://127.0.0.1:9 -x output.text stable.pkl 2>&1); [[ $out == *"Exception when making request"* ]] && check "cold cache with the network blocked fails" ok || check "cold cache offline: $out" no
rm -rf "$W"
exit $fail
