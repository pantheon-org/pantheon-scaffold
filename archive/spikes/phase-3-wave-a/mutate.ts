// Mutation check for Phase 3 wave A: each entry breaks one property of the verbatim tooling files and the tooling and mode tests must
// then fail. Run from the repository root: bun tmp/phase-3-wave-a/mutate.ts
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const M: [string, string, string, string][] = [
  [B, `[".github/scripts/sync-labels.sh"] = new Config.GenFile { content = read("assets/sync-labels.sh").text; executable = true }`, `[".github/scripts/sync-labels.sh"] = new Config.GenFile { content = read("assets/sync-labels.sh").text }`, "all nine scripts are executable"],
  [B, `[".rumdl.toml"] = new Config.GenFile { content = read("assets/rumdl.toml").text }`, `[".rumdl.toml"] = new Config.GenFile { content = read("assets/rumdl.toml").text; executable = true }`, "configs are not executable"],
  [B, `  [".claude/settings.json"] = new Config.GenFile { content = read("assets/claude-settings.json").text }\n`, ``, "the Claude settings are rendered"],
  [B, `[".github/scripts/auto-rebase.sh"] = new Config.GenFile { content = read("assets/auto-rebase.sh").text; executable = true }`, `[".github/scripts/auto-rebase.sh"] = new Config.GenFile { content = read("assets/sync-labels.sh").text; executable = true }`, "each script renders its own asset"],
  ["pkl/assets/auto-rebase.sh", "#!", "\n#!", "an asset is byte for byte the pinned upstream file"],
  ["pkl/assets/rumdl.toml", "[global]", "[global] ", "the rumdl config is byte for byte the pinned upstream file"],
  ["runner/src/check.ts", `if (wantExec !== isExec)`, `if (false)`, "check reports a mode that differs from the committed one"],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/tooling.test.ts test/mode-index.test.ts test/check.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
