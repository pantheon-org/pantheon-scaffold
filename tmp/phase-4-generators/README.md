# Phase 4 generators

`workflows-a.ts` writes the wave A workflow builtins into `pkl/Builtins.pkl`, between the BEGIN and END markers, from the pinned `pantheon-adr` copies in `runner/test/upstream/workflows`. The structure of each workflow is written in the script by hand. The header comment and every `run:` script are copied from the upstream file, so a script is never retyped, and the raw strings keep the backslashes in `jq` and regular expressions literal.

Run from the repository root: `bun tmp/phase-4-generators/workflows-a.ts`. It replaces the block between the markers, so running it again is safe. Edit the block through the script, not by hand, or the next run overwrites the edit.

`workflows-b.ts` does the same for the wave B builtins (`releasePlease`, `zizmor`, `agnix`, `aislop`, `dependabot` and `analysisFiles`), between its own pair of markers. A few header lines that depend on an option are written in the script with interpolation. Newlines inside the pkl text are written `\\n` in the script, because the script builds the text inside a JavaScript template literal.
