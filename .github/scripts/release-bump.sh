#!/usr/bin/env bash
#
# release-bump.sh
#
# After a release is published and verified, sets `version` in pkl/PklProject to the next patch,
# through a pull request that the App token opens and, once its checks are green, squash-merges.
# Without it the first pull request that changes anything under pkl/ would fail the package build
# ("was already published with different contents ... bump the packages version").
# Run by .github/workflows/release-bump.yml from the checkout of main.
# Env: GH_TOKEN (App token), GH_REPO (owner/name), TAG (the released tag), TAG_SHA (the commit the run was for), POLL_SECONDS (optional, default 30)
# Every early stop is announced in the step summary and as a notice or warning, never silent.

set -euo pipefail

poll_seconds="${POLL_SECONDS:-30}"

say() { echo "$1" | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"; }
version_of() { sed -n 's/^[[:space:]]*version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' pkl/PklProject; }

if ! [[ "$TAG" =~ ^v([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
  say "::notice::$TAG is not a plain vX.Y.Z release tag; no version bump."
  exit 0
fi
released="${TAG#v}"
next="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.$((BASH_REMATCH[3] + 1))"

# The tag must point at the commit the release run was for, so a branch that happens to be named v1.2.3 cannot start a bump.
ref_json="$(gh api "repos/$GH_REPO/git/ref/tags/$TAG")"
object_sha="$(jq -r .object.sha <<<"$ref_json")"
[ "$(jq -r .object.type <<<"$ref_json")" = tag ] && object_sha="$(gh api "repos/$GH_REPO/git/tags/$object_sha" --jq .object.sha)"
if [ "$object_sha" != "$TAG_SHA" ]; then
  echo "::error::$TAG points at $object_sha, not at $TAG_SHA, the commit of the release run. No bump." >&2
  exit 1
fi

current="$(version_of)"
if [ "$current" = "$released" ]; then
  :
elif [ "$(printf '%s\n%s\n' "$current" "$released" | sort -V | tail -n 1)" = "$current" ]; then
  say "::notice::pkl/PklProject on main is already at $current, above the released $released. No bump needed."
  exit 0
else
  echo "::error::pkl/PklProject on main is at $current, below the released $released. Fix it by hand." >&2
  exit 1
fi

branch="chore/start-$next"
if [ -n "$(gh pr list --repo "$GH_REPO" --head "$branch" --state open --json number --jq '.[].number')" ]; then
  say "::notice::A pull request for $branch is already open. No second one."
  exit 0
fi

main_sha="$(gh api "repos/$GH_REPO/git/ref/heads/main" --jq .object.sha)"
gh api --method POST "repos/$GH_REPO/git/refs" -f ref="refs/heads/$branch" -f sha="$main_sha" > /dev/null
file_json="$(gh api "repos/$GH_REPO/contents/pkl/PklProject?ref=$branch")"
blob_sha="$(jq -r .sha <<<"$file_json")"
content="$(jq -r .content <<<"$file_json" | base64 -d | sed "s/^\([[:space:]]*version[[:space:]]*=[[:space:]]*\)\"$released\"/\1\"$next\"/" | base64 | tr -d '\n')"
gh api --method PUT "repos/$GH_REPO/contents/pkl/PklProject" -f message="chore(pkl): start $next" -f content="$content" -f sha="$blob_sha" -f branch="$branch" > /dev/null
pr_url="$(gh pr create --repo "$GH_REPO" --base main --head "$branch" --title "chore(pkl): start $next" \
  --body "Sets the package version to $next after $TAG was released and verified, so the next change under pkl/ does not fail the package build. Opened and merged by release-bump.yml once its checks pass.")"
say "Opened $pr_url"

# Wait for the checks to register and finish: at least one check, the same number on two polls in a row, none pending.
previous=-1
for _ in $(seq 1 60); do
  # gh exits non-zero while checks are pending or failing, and says "no checks reported" before any exists: read the output either way.
  checks="$(gh pr checks "$pr_url" --json bucket --jq '[.[].bucket]' 2> /dev/null || true)"
  [ -n "$checks" ] || checks='[]'
  total="$(jq 'length' <<<"$checks")"
  pending="$(jq '[.[] | select(. == "pending")] | length' <<<"$checks")"
  failed="$(jq '[.[] | select(. == "fail" or . == "cancel")] | length' <<<"$checks")"
  if [ "$failed" -gt 0 ]; then
    echo "::error::$failed check(s) failed on $pr_url. It stays open for you." >&2
    exit 1
  fi
  if [ "$total" -gt 0 ] && [ "$pending" -eq 0 ] && [ "$total" -eq "$previous" ]; then
    gh pr merge "$pr_url" --squash --delete-branch
    say "Merged $pr_url: pkl/PklProject is now at $next."
    exit 0
  fi
  previous="$total"
  sleep "$poll_seconds"
done
echo "::error::The checks on $pr_url did not finish within 30 minutes. It stays open for you." >&2
exit 1
