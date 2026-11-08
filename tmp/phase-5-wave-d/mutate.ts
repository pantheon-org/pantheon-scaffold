// Mutation check for Phase 5 wave D: each entry breaks one property of dist-check.yml, scaffold-check.yml or dist-workspace.toml and the
// wave D and wave A tests must then fail. Run from the repository root: bun tmp/phase-5-wave-d/mutate.ts [text]
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const M: [string, string, string, string][] = [
  ["pkl/Tools.pkl", `const dist: String = "0.32.0"`, `const dist: String = "0.31.0"`, "the dist version is the one dist-workspace.toml names"],
  [B, `-- dist generate --check"`, `-- dist --version"`, "dist-check runs dist generate --check"],
  [B, `  when (opts.binary) { ["dist-check"] = distCheck() }`, `  ["dist-check"] = distCheck()`, "dist-check belongs to binary"],
  [B, `  ["scaffold-check"] = scaffoldCheck()\n`, ``, "scaffold-check is part of the standard workflows"],
  [B, `run = "mise x -- pantheon-scaffold check pantheon.pkl --attempts 3"`, `run = "mise x -- pantheon-scaffold check pantheon.pkl --attempts 1"`, "scaffold-check retries three times"],
  [B, `  permissions = readOnly\n  concurrency = concurrencyPerRef\n  jobs {\n    ["dist-check"]`, `  permissions = new Mapping { ["contents"] = "write" }\n  concurrency = concurrencyPerRef\n  jobs {\n    ["dist-check"]`, "dist-check reads contents only"],
  [B, `  "dist-workspace.toml"\n  ".github/workflows/release.yml"\n  "Cargo.toml"`, `  "Cargo.toml"`, "dist-check runs when dist-workspace.toml or release.yml change"],
  [B, `          run = "mise install --locked"`, `          run = "mise install"`, "scaffold-check installs from the lock file"],
  ["pkl/assets/dist-workspace.toml", `"actions/upload-artifact" = "cf430e030ddbb5b0abf93d22962f4752f3646cd9"`, `"actions/upload-artifact" = "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"`, "dist-workspace.toml names the upload-artifact commit release.yml uses"],
  ["pkl/assets/dist-workspace.toml", `"actions/download-artifact" = "9000827ccba6bdab643e8b6fd33ac0654aef8333"`, `"actions/download-artifact" = "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c"`, "dist-workspace.toml names the download-artifact commit release.yml uses"],
  ["pkl/assets/dist-workspace.toml", `installers = ["shell"]`, `installers = ["shell", "msi"]`, "dist-workspace.toml is otherwise the repositories' file"],
];
const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
const only = Bun.argv[2];
for (const [file, from, to, what] of M) {
  if (only && !what.includes(only)) continue;
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/phase5-dist.test.ts test/phase5-verbatim.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
