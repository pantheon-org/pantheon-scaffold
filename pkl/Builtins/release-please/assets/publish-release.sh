#!/usr/bin/env bash
#
# publish-release.sh
#
# Publishes the pre-release for a tag. Run by the repository's own release workflow on a tag push, after it has built
# and attested the assets in DIST_DIR (the "draft" release shape of the release-please builtin; docs/adopting.md says how
# to wire it).
#   - release-please made a draft for the tag: upload the assets to it, then publish it as a pre-release, keeping the
#     draft's changelog body under RELEASE_NOTES.
#   - nothing exists for the tag (a tag pushed by hand): create the pre-release.
#   - a published release already exists: stop. A published version is never re-published.
# Env: GH_TOKEN (needs contents: write), GH_REPO (owner/name), TAG (the pushed tag),
#      DIST_DIR (the folder with the assets, default dist), RELEASE_NOTES (the text above the changelog, optional)

set -euo pipefail

dist_dir="${DIST_DIR:-dist}"
notes="${RELEASE_NOTES:-Pre-release $TAG of $GH_REPO.}"
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
  gh release upload "$TAG" "$dist_dir"/*
  gh api --method PATCH "repos/$GH_REPO/releases/$ids" -F draft=false -F prerelease=true -f name="$title" -f body="$notes"$'\n\n'"$body" > /dev/null
  echo "Published the release-please draft for $TAG as a pre-release." >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
elif gh release view "$TAG" > /dev/null 2>&1; then
  echo "A release for $TAG already exists. A published version is never re-published: use a new patch version." >&2
  exit 1
else
  gh release create "$TAG" "$dist_dir"/* --verify-tag --prerelease --title "$title" --notes "$notes"
fi
