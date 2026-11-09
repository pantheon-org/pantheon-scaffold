// Mutation check for Phase 7 wave A: each entry loosens or breaks one rule of the categoriser and proof-categories.test.ts must then fail.
// Run from the repository root: bun tmp/phase-7-wave-a/mutate.ts [text]
import { $ } from "bun";

const F = "runner/scripts/proof-categories.ts";
const M: [string, string, string][] = [
  [`const NEW_FILES = ["hk.shared.pkl", ".mise/conf.d/repo.toml", ".github/workflows/dist-check.yml", ".github/workflows/scaffold-check.yml"];`, `const NEW_FILES = ["hk.shared.pkl", ".mise/conf.d/repo.toml", ".github/workflows/dist-check.yml"];`, "scaffold-check.yml is a known new file"],
  [`i.path === ".github/scripts/sync-labels.sh"`, `i.path.endsWith(".sh")`, "only sync-labels.sh may change mode alone"],
  [`n.added === 10 && n.removed === 4`, `n.added >= 10 && n.removed >= 4`, "mise.toml is exactly 10 added and 4 removed"],
  [`l === ".pkl-gen/cache/" || l === "target/"`, `l.length > 0`, ".gitignore may only gain target/ and .pkl-gen/cache/"],
  [`l === "/target"`, `l.length > 0`, ".gitignore may only lose /target"],
  [`/^"actions\\/(upload|download)-artifact" = /.test(a[i]) && /^"actions\\/(upload|download)-artifact" = /.test(b[i])`, `true`, "dist-workspace.toml may only change the two artifact commits"],
  [`d.startsWith("$.updates[1].ignore")`, `d.startsWith("$.updates")`, "dependabot.yml may only gain the ignore list of the github-actions entry"],
  [`if (repo !== "pantheon-skill-auditor") return undefined;`, ``, "only the skill auditor may differ in SECURITY.md"],
  [`l.startsWith("- The ")`, `l.length > 0`, "SECURITY.md may differ in scope bullets only"],
  [`return moved.length ? result("pin moved"`, `return false ? result("pin moved"`, "a moved pin is reported as a moved pin"],
  [`compareDirectives(i.theirs, i.mine).length === 0) {`, `true) {`, "a lost directive comment is not a comment difference"],
  [`if (i.path === "hk.pkl") return`, `if (false) return`, "hk.pkl replaced by the seeded one is the migration"],
];
const before = await Bun.file(F).text();
let missed = 0;
const only = Bun.argv[2];
for (const [from, to, what] of M) {
  if (only && !what.includes(only)) continue;
  const original = await Bun.file(F).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(F, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/proof-categories.test.ts`.quiet().nothrow();
  await Bun.write(F, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const restored = (await Bun.file(F).text()) === before;
console.log(restored ? "restored: the file is as it was" : "RESTORE FAILED: the file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
