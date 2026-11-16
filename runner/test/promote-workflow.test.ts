import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// Properties of .github/workflows/promote.yml that make promoting safe. They are read from the file, so a change that breaks one fails here.
const path = join(import.meta.dir, "../../.github/workflows/promote.yml");
const text = readFileSync(path, "utf8");
type Step = { name?: string; uses?: string; run?: string; if?: string; env?: Record<string, string>; with?: Record<string, unknown> };
type Job = { needs?: string | string[]; if?: string; environment?: string; permissions?: Record<string, string>; env?: Record<string, string>; steps: Step[] };
const wf = Bun.YAML.parse(text) as { on: Record<string, { inputs?: Record<string, { type: string; default?: unknown; required?: boolean; options?: string[] }> }>; permissions: unknown; jobs: Record<string, Job>; concurrency?: { "cancel-in-progress"?: boolean } };
const jobs = Object.entries(wf.jobs);
const allSteps = jobs.flatMap(([name, j]) => j.steps.map((s) => ({ job: name, ...s })));

// How a run asks the tool to edit a release.
const RUST_EDIT = 'tool/scaffold-release promote promote "$TAG"';

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

test("the promote job runs only after the tool and guards jobs, and only when the run is not a dry run", () => {
  expect([wf.jobs.promote.needs].flat().sort()).toEqual(["guards", "tool"]);
  expect(wf.jobs.promote.if).toBe("inputs.dry_run == false");
});

test("the guards job checks with the release tool, and only the promote job can ask it to edit", () => {
  const guards = wf.jobs.guards.steps.map((s) => s.run ?? "").join("\n");
  expect(guards).toContain('tool/scaffold-release promote check "$TAG"');
  const edits = allSteps.filter((s) => s.run?.includes("promote promote"));
  expect(edits.map((s) => [s.job, s.run])).toEqual([["promote", RUST_EDIT]]);
});

// Phase 6 of docs/ts-tooling-rust-port.md: the TypeScript promote tool and the way back to it are gone. There is one tool, so there is no
// input that chooses one, nothing that compares two, and no step that installs bun or mise for a TypeScript run.
test("there is one tool: no input chooses it, no step compares two, and nothing installs bun or mise", () => {
  expect(Object.keys(wf.on.workflow_dispatch.inputs!)).toEqual(["tag", "dry_run"]);
  expect(text).not.toMatch(/implementation|promote\.ts|\bbun\b|\bmise\b|disagree/);
  expect(allSteps.filter((s) => s.uses?.startsWith("jdx/mise-action@"))).toEqual([]);
  const check = wf.jobs.guards.steps.find((s) => s.run?.includes("promote check"));
  expect(check?.run).toBe('tool/scaffold-release promote check "$TAG"');
});

test("the release tool is built in a job that cannot write and holds no token", () => {
  const tool = wf.jobs.tool;
  expect(tool.permissions).toEqual({ contents: "read" });
  expect(tool.env).toBeUndefined();
  for (const s of tool.steps) expect(JSON.stringify(s), s.name ?? s.uses).not.toMatch(/github\.token|secrets\./);
  expect(tool.steps.some((s) => s.run?.includes("cargo build --release --locked -p scaffold-release"))).toBe(true);
  expect(tool.steps.find((s) => s.uses?.startsWith("actions/upload-artifact@"))?.with?.name).toBe("scaffold-release");
});

test("guards and promote download the release tool and check its checksum before they run it", () => {
  for (const name of ["guards", "promote"]) {
    const job = wf.jobs[name];
    expect([job.needs].flat(), name).toContain("tool");
    const download = job.steps.findIndex((s) => s.uses?.startsWith("actions/download-artifact@") && s.with?.name === "scaffold-release");
    expect(download, `${name} downloads the tool`).toBeGreaterThanOrEqual(0);
    const verify = job.steps.findIndex((s) => s.run?.includes("sha256sum --check --strict"));
    expect(verify, `${name} checks its checksum`).toBeGreaterThan(download);
    expect(job.steps[verify].env?.EXPECTED, name).toBe("${{ needs.tool.outputs.sha256 }}");
    const firstRun = job.steps.findIndex((s) => s.run?.includes("tool/scaffold-release promote"));
    expect(firstRun, `${name} runs it`).toBeGreaterThan(verify);
  }
});

test("no job that can write runs cargo", () => {
  for (const [name, j] of jobs) {
    const writes = Object.values(j.permissions ?? {}).some((v) => v === "write");
    const runsCargo = j.steps.some((s) => /\bcargo\b/.test(s.run ?? ""));
    expect(writes && runsCargo, `job ${name}`).toBe(false);
  }
});

test("no job has a job-wide token in its environment: each step that needs one names it", () => {
  for (const [name, j] of jobs) expect(j.env?.GITHUB_TOKEN, name).toBeUndefined();
});

test("every action is pinned by a 40 character SHA", () => {
  const uses = allSteps.map((s) => s.uses).filter((u): u is string => !!u && !u.startsWith("./"));
  // checkout in three jobs, the tool upload, and the tool download in two jobs.
  expect(uses.length).toBe(6);
  for (const u of uses) expect(u, u).toMatch(/@[0-9a-f]{40}$/);
});

test("it uses no secrets and persists no checkout credentials", () => {
  expect(text).not.toMatch(/secrets\./);
  for (const s of allSteps.filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"], s.job).toBe(false);
});

test("no run script interpolates text a dispatcher controls: the tag goes through env", () => {
  for (const s of allSteps.filter((s) => s.run)) expect(s.run, `${s.job}: ${s.name}`).not.toMatch(/\$\{\{/);
  for (const s of allSteps.filter((s) => s.run?.includes("scaffold-release promote"))) expect(s.env?.TAG, `${s.job}: ${s.name}`).toBe("${{ inputs.tag }}");
});

test("runs never overlap and are never cancelled half way", () => {
  expect(wf.concurrency).toMatchObject({ "cancel-in-progress": false });
});
