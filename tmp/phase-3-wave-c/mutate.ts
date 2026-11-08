// Mutation check for Phase 3 wave C: each entry breaks one property of the hook files and the hk tests must then fail.
// Run from the repository root: bun tmp/phase-3-wave-c/mutate.ts
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const SH = "pkl/assets/hk.shared.pkl.tmpl";
const SEED = "pkl/assets/hk.pkl.tmpl";
const M: [string, string, string, string][] = [
  [SH, `rumdl check --config .rumdl.toml {{files}}`, `rumdl check {{files}}`, "the pre-commit steps are the repositories' steps"],
  [SH, `cargo clippy --workspace --all-targets @@FEATURES@@--locked`, `cargo clippy --workspace --all-targets --locked`, "the clippy step takes the features option"],
  [SH, `cargo test --workspace @@FEATURES@@--locked`, `cargo test --workspace --locked`, "the test step takes the features option"],
  [B, `if (opts.features == "all") "--all-features " else ""`, `""`, "all features reach the pre-push steps"],
  [B, `\`class\` = "seed"\n    content = read("assets/hk.pkl.tmpl")`, `\`class\` = "owned"\n    content = read("assets/hk.pkl.tmpl")`, "hk.pkl is seeded, not owned"],
  ["pkl/Tools.pkl", `const hkConfigVersion: String = "1.49.0"`, `const hkConfigVersion: String = "1.48.0"`, "the hk config package version is the one the repositories use"],
  [SEED, `        steps = shared.preCommit()\n    }\n    ["check"]`, `        steps = shared.prePush()\n    }\n    ["check"]`, "the seeded hooks use the right shared steps"],
  [SEED, `import "hk.shared.pkl" as shared\n`, ``, "the seeded hk.pkl imports the shared module"],
  [SEED, `// This file is seeded once`, `// This file is generated`, "the seed says it is yours"],
  [SH, `// Rendered by pantheon-scaffold from pantheon.pkl. Do not edit`, `// Hand written. Edit`, "the shared module says the runner owns it"],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/hk.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
