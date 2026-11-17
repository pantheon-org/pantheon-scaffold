#!/usr/bin/env bash
#
# open-pin-pull-request.sh
#
# Commits the pin update that apply-pin-update.sh wrote into the checkout to the branch chore/update-pins and opens the pull request, or leaves the
# open one in place. Run by the `update` job of update-pins.yml, after the apply.
#
# Env: GH_TOKEN (the App token: contents and pull-requests write, and workflows write for the rendered workflows), REPO (owner/name),
#      SUMMARY (the file that is the pull request body),
#      BRANCH (default chore/update-pins), REMOTE (default https://github.com/$REPO.git; the tests give a local repository).
#
#   - nothing changed in the checkout: say so and stop. The pins are current.
#   - the branch exists and every commit on it that main does not have is the bot's (last week's update): replace it, with a lease on what was
#     fetched, so a push that raced with another is refused and not forced over it.
#   - the branch holds a commit by someone else: leave it alone and say so. The open pull request is theirs now; the job still records the check.
#
# Only tracked files that apply-pin-update.sh changed are committed (git add -u): a file the job did not mean to change is not added.

set -euo pipefail

branch="${BRANCH:-chore/update-pins}"
remote="${REMOTE:-https://github.com/${REPO:?REPO is needed}.git}"
summary="${SUMMARY:?SUMMARY is needed}"
bot_name="pantheon-ai-bot[bot]"
bot_email="pantheon-ai-bot[bot]@users.noreply.github.com"
title="chore(deps): update pinned actions"

if git diff --quiet; then
  echo "No pin changed."
  exit 0
fi

# The token goes in a header, not in the URL, so a git error does not print it.
auth="$(printf 'x-access-token:%s' "${GH_TOKEN:?GH_TOKEN is needed}" | base64 | tr -d '\n')"
net() { git -c "http.extraheader=AUTHORIZATION: basic $auth" "$@"; }

expected=""
if net fetch -q "$remote" "refs/heads/$branch:refs/remotes/pin-update/$branch" 2> /dev/null; then
  expected="$(git rev-parse "refs/remotes/pin-update/$branch")"
  others="$(git log --format='%ae' "HEAD..refs/remotes/pin-update/$branch" | grep -vxF "$bot_email" || true)"
  if [ -n "$others" ]; then
    echo "::notice::$branch has a commit that is not the bot's, so it is left as it is and the pins are not pushed this week."
    exit 0
  fi
fi

git switch -c "$branch"
git add -u
git -c "user.name=$bot_name" -c "user.email=$bot_email" commit --message "$title"
net push --force-with-lease="$branch:$expected" "$remote" "$branch"

if [ -z "$(gh pr list --repo "$REPO" --head "$branch" --json number --jq '.[].number')" ]; then
  gh pr create --repo "$REPO" --head "$branch" --title "$title" --body-file "$summary"
fi
