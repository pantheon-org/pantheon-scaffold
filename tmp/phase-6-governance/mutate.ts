// Mutation check of runner/src/governance/*.ts: each mutation removes one behaviour, and the governance tests must then fail. Files are always restored.
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const f = (n: string) => resolve(root, `runner/src/governance/${n}.ts`);
const mutations: [string, string, string, string][] = [
  ["compare: enforcement", "ruleset", "if (d.enforcement !== l.enforcement) out.push(", "if (false) out.push("],
  ["compare: target", "ruleset", "if (d.target !== l.target) out.push(", "if (false) out.push("],
  ["compare: conditions", "ruleset", "if (!same(d.conditions, l.conditions)) out.push(", "if (false) out.push("],
  ["compare: bypass actors", "ruleset", "if (!same(d.bypass_actors, l.bypass_actors)) out.push(", "if (false) out.push("],
  ["compare: rule missing in live", "ruleset", "else if (!lr) out.push(", "else if (false) out.push("],
  ["compare: rule extra in live", "ruleset", "if (!dr) out.push(", "if (false) out.push("],
  ["compare: rule parameters", "ruleset", "else if (!same(dr, lr)) {", "else if (false) {"],
  ["convert: a field GET adds is kept", "ruleset", "    name: body.name as string,", "    ...body,\n    name: body.name as string,"],
  ["convert: required checks not sorted", "ruleset", "[...(parameters.required_status_checks as unknown[])].sort(byJson)", "[...(parameters.required_status_checks as unknown[])]"],
  ["convert: nulls kept", "ruleset", ".filter(([, x]) => x !== null && x !== undefined)", ".filter(() => true)"],
  ["live: missing ruleset", "live", "if (!l) problems.push(", "if (false) problems.push("],
  ["live: extra ruleset", "live", "if (!wanted.some((w) => w.name === l.name)) problems.push(", "if (false) problems.push("],
  ["live: slug check", "live", "if (!SLUG.test(owner) || !SLUG.test(repo)) throw", "if (false) throw"],
  ["live: answer is not a list", "live", "if (!Array.isArray(list)) throw", "if (false) throw"],
  ["checks: reusable workflow", "checks", "if (job.uses) throw", "if (false) throw"],
  ["checks: unknown expression", "checks", "if (!/^matrix\\.[A-Za-z0-9_-]+$/.test(e)) throw", "if (false) throw"],
  ["checks: matrix include", "checks", '"include" in m || "exclude" in m', "false"],
  ["checks: missing required", "checks", "missing: [...new Set(required)].filter((r) => !offered.includes(r)).sort()", "missing: [] as string[]"],
  ["checks: unrequired", "checks", "unrequired: [...new Set(offered)].filter((o) => !required.includes(o)).sort()", "unrequired: [] as string[]"],
];
const originals = new Map(["ruleset", "live", "checks"].map((n) => [n, readFileSync(f(n), "utf8")]));
let caught = 0;
try {
  for (const [name, file, from, to] of mutations) {
    const text = originals.get(file)!;
    if (!text.includes(from)) throw new Error(`mutation "${name}": text not found`);
    writeFileSync(f(file), text.replace(from, to));
    const r = Bun.spawnSync(["bun", "test", "test/governance-ruleset.test.ts", "test/governance-checks.test.ts"], { cwd: resolve(root, "runner"), stdout: "pipe", stderr: "pipe" });
    const failed = r.exitCode !== 0;
    if (failed) caught++;
    console.log(`${failed ? "caught  " : "MISSED  "} ${name}`);
    writeFileSync(f(file), text);
  }
} finally {
  for (const [n, t] of originals) writeFileSync(f(n), t);
}
console.log(`${caught} of ${mutations.length} caught`);
process.exit(caught === mutations.length ? 0 : 1);
