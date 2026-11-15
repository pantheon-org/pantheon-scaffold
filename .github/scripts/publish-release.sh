#!/usr/bin/env bash
#
# publish-release.sh
#
# Publishes the pre-release for a tag. Run by the release job of release.yml on a tag push,
# after the assets are attested.
#   - release-please made a draft for the tag: upload the assets to it, then publish it as a
#     pre-release, keeping the draft's changelog body under the usual notes.
#   - nothing exists for the tag (a tag pushed by hand): create the pre-release, as before.
#   - a published release already exists: stop. A published version is never re-published.
# Env: GH_TOKEN (needs contents: write), GH_REPO (owner/name), TAG (the pushed tag)

set -euo pipefail

notes="Pre-release of the package and the runner, released together. checksums.txt lists every file and carries a build provenance attestation: gh attestation verify checksums.txt --repo $GH_REPO. What breaks when you upgrade, and the order to change it in, is under Upgrading: https://github.com/$GH_REPO/blob/$TAG/docs/adopting.md#upgrading"
title="$TAG (pre-release)"

# The draft is looked up by tag in the list of releases and then used by its id.
ids="$(gh api "repos/$GH_REPO/releases" --paginate --jq ".[] | select(.draft and .tag_name == \"$TAG\") | .id")"
count="$(printf '%s' "$ids" | grep -c . || true)"
if [ "$count" -gt 1 ]; then
  echo "More than one draft release exists for $TAG: $(echo "$ids" | tr '\n' ' '). Delete the extra ones and re-run." >&2
  exit 1
fi

if [ "$count" -eq 1 ]; then
  body="$(gh api "repos/$GH_REPO/releases/$ids" --jq .body)"
  gh release upload "$TAG" dist/*
  gh api --method PATCH "repos/$GH_REPO/releases/$ids" -F draft=false -F prerelease=true -f name="$title" -f body="$notes"$'\n\n'"$body" > /dev/null
  echo "Published the release-please draft for $TAG as a pre-release." >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
elif gh release view "$TAG" > /dev/null 2>&1; then
  echo "A release for $TAG already exists. A published version is never re-published: use a new patch version." >&2
  exit 1
else
  gh release create "$TAG" dist/* --verify-tag --prerelease --title "$title" --notes "$notes"
fi
