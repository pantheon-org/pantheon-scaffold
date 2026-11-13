# migrate --accept-builtins: design record

Written on 13-11-2026 for follow-up 1 of the refocus, before any scenario or code. It decides how `migrate` replaces files that have drifted from the Builtins only because the repository is older than the package. The evidence is `docs/migrate-dry-runs.md` (PR 106): 20 drifted files across the four `pantheon-*` repositories, 19 of them stale and 1 caused by a missing option (`features = "all"`, a follow-up of its own). The plan was reviewed by three reviewers on 13-11-2026 and amended; the owner answered the two open choices (D1 and D2).

## The problem

`migrate --write` refuses while any file has drifted, and `--as-presets` keeps each drifted file as a Preset. For drift that is only age, neither is wanted: the wanted outcome is to replace the file with the Builtin's render and be able to see what was replaced.

## Decisions

| # | Decision | Why |
|---|---|---|
| D1 | `--accept-builtins` replaces **every** drifted file or none. There is no `--only`. | `apply` refuses any existing file that differs and is not on the adopt list, so replacing some files while another stays drifted can never succeed. Letting `apply` keep left-out files was rejected as a bigger change to the write path. Revisit when a real adoption has to accept some files and keep one as a Preset. |
| D2 | `--write` with the flag needs a **git work tree whose tracked tree is clean, in which every drifted file is tracked**. Otherwise it is refused (exit 3) before anything is written. | `apply` writes files one at a time and does not restore an overwritten file if a later write fails, and a non-YAML difference is printed as at most 6 lines compared by position. Git holds the old content, so the git diff of the adoption commit is the full record and the way back. The printed lines are a summary. The check is the one `delete_guard` already makes for deletes, and shares its `git` helper. Revisit when a repository has to be adopted outside git. |
| D3 | A drifted YAML file that does not parse is replaced like any other. | Its content is in git (D2), which the printed `not valid YAML` line does not show. |
| D4 | The flag is for first adoption. It does not apply to files that already have a manifest entry. | `apply` refuses a hand-edited owned file whatever is on the adopt list, so the flag cannot reach it. This keeps the flag from being reused for the older-pin case (follow-up 3) by accident. |
| D5 | The flag cannot be combined with `--as-presets`, and `init` does not take it. Both are usage errors (exit 2), found before pkl is evaluated. The exclusion is reversible. | A mixed repository (accept some files, keep one as a Preset) is the likely next request, and it needs its own design (see D1). |
| D6 | With nothing drifted the flag changes nothing: the normal report, exit 0, and a line saying nothing was replaced. | The flag is a permission, not an action. |
| D7 | A dry run runs the git check too and says what a write would refuse. It writes nothing. | The dry run must show the same outcome as the write. |
| D8 | After a successful write, one block on stdout names the replaced files and says the git diff is the full record. Nothing is printed as replaced when the write was refused or failed. The differences themselves are the ones `migrate` already prints for every drifted file on every run, and are not printed twice. | A commit message must not claim a replacement that did not happen. |

## Where it lives

- `crates/scaffold-core/src/guard.rs`: `replace_guard`, beside `delete_guard`. Both share the git checks, and each keeps its own wording.
- `crates/scaffold-core/src/migrate.rs`: `Report::drifted_paths` and `Report::adopt_accepting_drifted`, which adds the drifted files to the set `apply` may overwrite. `apply` itself is unchanged.
- `crates/scaffold-cli/src/lib.rs`: the switch, the two usage errors in `prepare` (before pkl is evaluated), and `accept_drifted`, which runs the git check and prints the dry-run lines.
- Tests: scenarios in `features/migrate.feature`, and CLI tests in `crates/scaffold-cli/tests/migrate.rs` that use a real git repository, because mutation testing runs only a package's own tests and so does not run the scenarios.

## Limits, recorded and not fixed

- **A missing option is not stale drift.** A file that drifts because `--features`, `--composite-action` or `--git-dependency-ignore` was not given would be replaced with the wrong render. Set every option first. The dev-kit's `rust-ci.yml` is the known case: give it `--features all` (added on 13-11-2026) before accepting anything.
- `migrate` cannot tell stale drift from a deliberate change. The owner reads the git diff before committing.
- A difference in line endings only looks identical in the printed lines. A non-UTF-8 file is printed lossily but replaced byte for byte.
- The executable bit follows the render, as for any written file.
