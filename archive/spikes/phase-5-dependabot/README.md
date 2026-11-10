# Decision 86: Dependabot ignores the two artifact actions of release.yml

`mutate.ts` breaks five properties of the dependabot builtin (only a repository with a binary ignores the two actions, each of the two entries, the header note, and the unchanged seven day cooldown of the github-actions entry) and fails if `runner/test/phase5-dependabot.test.ts` and `workflows-b.test.ts` stay green for any. Run from the repository root: `bun tmp/phase-5-dependabot/mutate.ts`. It takes about a minute.

Run on 09-11-2026: 5 of 5 caught the first time. Files restored.
