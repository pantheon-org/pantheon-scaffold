# p6-projen-spike

P.6 of the pkl plan: the nine behaviours of `tmp/pkl-spike` repeated for projen 0.103.21, in two styles. Run on 07-11-2026. Untracked.

- `.projenrc.ts`: the project. `STYLE=verbatim|typed`, `AGNIX=on|off`, `TOOLCHAIN` choose the render. Run with `bun .projenrc.ts` (projen's `ts-node` was blocked by this machine's package gate, so bun runs the file).
- `fetch-assets.sh <dir>`: fetches the fixtures fresh from `pantheon-org/pantheon-adr` main (read-only, `gh api`).
- `test.sh`: `bash test.sh`. Runs both styles in a throwaway copy with a symlinked `node_modules`. Needs `npm install` once in this folder.
- `compare.ts a.yml b.yml`: parsed-data comparison of two YAML files.
- Results and conclusions: `.context/research/2026-11-07-gate-a-evidence-projen-pkl-and-do-nothing.md`.

Pitfalls found: `TextFile` writes no trailing newline (the projenrc adds an empty last line), the typed API needs two `addOverride` escape hatches for this one workflow and adds step ids, and generated files are mode 444 and overwritten silently.
