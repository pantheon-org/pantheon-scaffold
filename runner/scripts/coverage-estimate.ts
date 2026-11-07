// Estimates how much of the real workflows the typed model covers without the raw escape hatch, as a gate B input.
// Counts constructs (workflows, jobs, steps) and how many carry a key the typed classes do not have.
// Read-only: fetches workflow files from GitHub with gh api. Usage: bun runner/scripts/coverage-estimate.ts [owner/repo]
import { $ } from "bun";

const repo = Bun.argv[2] ?? "pantheon-org/pantheon-adr";
const WORKFLOW_KEYS = new Set(["name", "on", "permissions", "concurrency", "env", "jobs"]);
const JOB_KEYS = new Set(["name", "runs-on", "needs", "if", "permissions", "timeout-minutes", "env", "strategy", "steps"]);
const STEP_KEYS = new Set(["name", "id", "uses", "run", "with", "env", "if", "working-directory"]);

const names = (await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows --jq '.[].name'`.text()).trim().split("\n");
let totalConstructs = 0, totalEscapes = 0;
const rows: string[] = [];
for (const n of names) {
  const text = Buffer.from((await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows/${n} --jq .content`.text()).trim(), "base64").toString("utf8");
  const wf = Bun.YAML.parse(text) as Record<string, any>;
  let constructs = 1, escapes = 0;
  const missing: string[] = [];
  const check = (obj: Record<string, unknown>, allowed: Set<string>, label: string) => {
    const extra = Object.keys(obj).filter((k) => !allowed.has(k));
    if (extra.length) { escapes++; missing.push(`${label}:${extra.join("+")}`); }
  };
  check(wf, WORKFLOW_KEYS, "workflow");
  for (const [id, job] of Object.entries<any>(wf.jobs ?? {})) {
    constructs++;
    check(job, JOB_KEYS, `job ${id}`);
    for (const [i, step] of (job.steps ?? []).entries()) {
      constructs++;
      check(step, STEP_KEYS, `step ${id}#${i + 1}`);
    }
  }
  totalConstructs += constructs;
  totalEscapes += escapes;
  rows.push(`${n.padEnd(28)} constructs ${String(constructs).padStart(3)}  need raw ${String(escapes).padStart(2)}  ${[...new Set(missing.map((m) => m.split(":")[1]))].join(" ")}`);
}
console.log(`${repo}: ${names.length} workflows`);
console.log(rows.join("\n"));
console.log(`TOTAL constructs ${totalConstructs}, constructs needing the escape hatch ${totalEscapes} (${((100 * totalEscapes) / totalConstructs).toFixed(1)} percent)`);
