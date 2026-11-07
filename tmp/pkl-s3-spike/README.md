# pkl-s3-spike

Spike S.3 of the pkl plan (decision 17): can the runner append `# <version>` to pinned `uses:` lines, and can a test and a
second check keep those comments honest? Run on 07-11-2026 with bun 1.4.2. Untracked. All GitHub access is read-only (`gh api`).

Files: `versions.ts` (append, missing-comment checker, unpinned finder), `versions.test.ts` (12 tests, `bun test`),
`roundtrip.ts` (real workflows round trip), `verify-tags.ts` (SHA belongs to tag).

| Question | Answer |
|---|---|
| Does the append work and stay idempotent? | Yes. It matches `uses: owner/repo[/path]@<40 hex>` (also quoted values and reusable workflow paths), appends ` # <version>` from a pin table, leaves a matching comment untouched, and a second run changes nothing. It runs on text, so it can run before hashing and inside `--check`. |
| Edge cases | 12 tests pass: `uses:` inside a `run: \|` block, inside a quoted string and inside a comment is ignored (needs block-scalar tracking), local actions and `docker://` digests are ignored, an unknown pin and a differing existing comment both fail loudly instead of guessing, an unpinned `@v4` is reported by a separate finder. |
| Round trip on the real workflows | `pantheon-adr`: 24 commented lines rebuilt byte for byte, 24 of 24. `pantheon-skill-auditor`: 23 of 23. (15 workflows each, 9 and 8 distinct pins.) |
| Does the missing-comment test fail when it should? | Yes. In both repos it flags 16 pinned lines with no comment, and all 16 are in `release.yml`, the file `dist generate` writes. No other workflow has an uncommented pin. The `release.yml` file is out of the generator's ownership (plan scope), so the checker must exclude it or the test fails forever. |
| Does the SHA belong to the tag? | 8 of 9 distinct pin and comment pairs are confirmed with `gh api repos/<repo>/commits/<tag>` (it resolves annotated tags to the commit). A control shows a wrong comment (v7.0.0 against the v7.0.1 pin) would be reported as a mismatch. |
| Non-tag comments | One pin has the comment `main at v0.3.1` (`pantheon-org/pantheon-skill-auditor`). It is not a tag, so the SHA check reports it as unverifiable. The plan needs a rule for pins to a branch commit (for example a separate pin kind with its own comment format, or a tag). |

Not covered: CRLF files (not supported in 0.x, decision 34), a `uses:` value split over several lines, YAML anchors, and wiring this into the real runner.
The `console.log` lines are the tools' output on purpose.
