import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// Properties of .github/workflows/governance-drift.yml that keep a scheduled, public workflow harmless. They are read from the file.
const path = join(import.meta.dir, "../../.github/workflows/governance-drift.yml");
const text = readFileSync(path, "utf8");
type Step = { name?: string; uses?: string; run?: string; if?: string; env?: Record<string, string>; with?: Record<string, unknown>; "continue-on-error"?: boolean };
type Job = { permissions?: Record<string, string>; env?: Record<string, string>; steps: Step[] };
const wf = Bun.YAML.parse(text) as { on: Record<string, unknown>; permissions: unknown; jobs: Record<string, Job>; concurrency?: { "cancel-in-progress"?: boolean } };
const job = wf.jobs.drift;

test("it runs weekly or by hand, never on a push, a pull request or pull_request_target", () => {
  expect(Object.keys(wf.on).sort()).toEqual(["schedule", "workflow_dispatch"]);
  expect(wf.on.schedule).toEqual([{ cron: "23 6 * * 1" }]);
  expect(text).not.toContain("pull_request");
});

test("a manual run is a dry run unless told otherwise", () => {
  expect((wf.on.workflow_dispatch as { inputs: Record<string, unknown> }).inputs.dry_run).toMatchObject({ type: "boolean", default: true });
});

test("nothing has permissions by default, and the one job can read the contents and write issues, and nothing else", () => {
  expect(wf.permissions).toEqual({});
  expect(Object.keys(wf.jobs)).toEqual(["drift"]);
  expect(job.permissions).toEqual({ contents: "read", issues: "write" });
});

test("it uses no secret and no App: the only token is the run's own", () => {
  expect(text).not.toMatch(/secrets\./);
  expect(text).not.toContain("create-github-app-token");
  expect(job.env?.GH_TOKEN).toBe("${{ github.token }}");
});

test("every action is pinned by a 40 character SHA, and the checkout keeps no credentials", () => {
  const uses = job.steps.map((s) => s.uses).filter((u): u is string => !!u);
  expect(uses.length).toBe(2);
  for (const u of uses) expect(u, u).toMatch(/@[0-9a-f]{40}$/);
  expect(job.steps.find((s) => s.uses?.startsWith("actions/checkout@"))?.with?.["persist-credentials"]).toBe(false);
});

test("the issue is touched only by the step that is told to, and a dry run never reaches it", () => {
  const issueSteps = job.steps.filter((s) => s.run?.includes("--issue"));
  expect(issueSteps.length).toBe(1);
  expect(issueSteps[0].if).toBe("github.event_name == 'schedule' || !inputs.dry_run");
  const dry = job.steps.filter((s) => s.run?.includes("governance.ts drift") && !s.run.includes("--issue"));
  expect(dry.length).toBe(1);
  expect(dry[0].if).toBe("github.event_name == 'workflow_dispatch' && inputs.dry_run");
  // Drift (exit 4) is a result of a dry run. Any other failure is an error and must fail the run, so nothing is allowed to continue on error.
  expect(dry[0].run).toBe("mise x -- bun runner/scripts/governance.ts drift || [ $? -eq 4 ]");
  expect(text).not.toContain("continue-on-error");
});

test("no run script interpolates anything: nothing a dispatcher controls reaches a shell", () => {
  for (const s of job.steps.filter((s) => s.run)) expect(s.run, s.name).not.toMatch(/\$\{\{/);
});

test("runs never overlap and are never cancelled half way", () => {
  expect(wf.concurrency).toMatchObject({ "cancel-in-progress": false });
});
