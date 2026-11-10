# Phase 3 template generators

How `pkl/assets/mise.toml.tmpl`, `pkl/assets/hk.shared.pkl.tmpl` and `pkl/assets/hk.pkl.tmpl` were made from the files in `pantheon-adr` at `1e2fd46` (the pinned copies `runner/test/upstream/mise.toml` and `runner/test/upstream/hk.pkl`), so the step and tool text is copied by a script and not retyped.

Run from the repository root: `bun tmp/phase-3-template-generators/mise-template.ts` and `bun tmp/phase-3-template-generators/hk-templates.ts`. Each overwrites its templates. `mise-template.ts` stops with an error if the pinned copy no longer contains a line it replaces. Run on 08-11-2026; the templates in `pkl/assets/` are the output.
