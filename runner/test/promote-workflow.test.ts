import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// Properties of .github/workflows/promote.yml that make promoting safe. They are read from the file, so a change that breaks one fails here.
const path = join(import.meta.dir, "../../.github/workflows/promote.yml");
const text = readFileSync(path, "utf8");
type Step = { name?: string; uses?: string; run?: string; env?: Record<string, string>; with?: Record<string, unknown> };
type Job = { needs?: string; if?: string; environment?: string; permissions?: Record<string, string>; steps: Step[] };
const wf = Bun.YAML.parse(text) as { on: Record<string, { inputs?: Record<string, { type: string; default?: unknown; required?: boolean }> }>; permissions: unknown; jobs: Record<string, Job>; concurrency?: { "cancel-in-progress"?: boolean } };
const jobs = Object.entries(wf.jobs);
const allSteps = jobs.flatMap(([name, j]) => j.steps.map((s) => ({ job: name, ...s })));

test("it runs by hand only, never on a push, a pull request or pull_request_target", () => {
  expect(Object.keys(wf.on)).toEqual(["workflow_dispatch"]);
  expect(text).not.toContain("pull_request_target");
});

test("it takes a tag and is a dry run unless told otherwise", () => {
  expect(wf.on.workflow_dispatch.inputs!.tag).toMatchObject({ type: "string", required: true });
  expect(wf.on.workflow_dispatch.inputs!.dry_run).toMatchObject({ type: "boolean", default: true });
});

test("nothing has permissions by default and every job asks for its own", () => {
  expect(wf.permissions).toEqual({});
  for (const [name, j] of jobs) expect(j.permissions, `job ${name}`).toBeDefined();
});

test("only the promote job can write, and it writes only contents", () => {
  for (const [name, j] of jobs) {
    const writes = Object.entries(j.permissions ?? {}).filter(([, v]) => v === "write");
    expect(writes, `job ${name}`).toEqual(name === "promote" ? [["contents", "write"]] : []);
  }
});

test("the promote job sits behind the protected environment, and the dry-run job never touches it", () => {
  expect(wf.jobs.promote.environment).toBe("promote");
  expect(wf.jobs.guards.environment).toBeUndefined();
});

test("the promote job runs only after the guards job, and only when the run is not a dry run", () => {
  expect(wf.jobs.promote.needs).toBe("guards");
  expect(wf.jobs.promote.if).toBe("inputs.dry_run == false");
});

test("both jobs run the script, and the promote job is the only one that asks it to edit", () => {
  const runs = (job: string): string[] => wf.jobs[job].steps.map((s) => s.run ?? "").filter((r) => r.includes("promote.ts"));
  expect(runs("guards")).toEqual(['mise x -- bun runner/scripts/promote.ts check "$TAG"']);
  expect(runs("promote")).toEqual(['mise x -- bun runner/scripts/promote.ts promote "$TAG"']);
});

test("every action is pinned by a 40 character SHA", () => {
  const uses = allSteps.map((s) => s.uses).filter((u): u is string => !!u && !u.startsWith("./"));
  expect(uses.length).toBe(4);
  for (const u of uses) expect(u, u).toMatch(/@[0-9a-f]{40}$/);
});

test("it uses no secrets and persists no checkout credentials", () => {
  expect(text).not.toMatch(/secrets\./);
  for (const s of allSteps.filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"], s.job).toBe(false);
});

test("no run script interpolates text a dispatcher controls: the tag goes through env", () => {
  for (const s of allSteps.filter((s) => s.run)) expect(s.run, `${s.job}: ${s.name}`).not.toMatch(/\$\{\{/);
  for (const s of allSteps.filter((s) => s.run?.includes("promote.ts"))) expect(s.env?.TAG, s.job).toBe("${{ inputs.tag }}");
});

test("runs never overlap and are never cancelled half way", () => {
  expect(wf.concurrency).toMatchObject({ "cancel-in-progress": false });
});
