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

// The two ways a run can ask a tool to edit a release, and to check only.
const RUST_EDIT = 'tool/scaffold-release promote promote "$TAG"';
const TS_EDIT = 'mise x -- bun runner/scripts/promote.ts promote "$TAG"';

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

test("both tools check in the guards job, and only the promote job can ask a tool to edit", () => {
  const guards = wf.jobs.guards.steps.map((s) => s.run ?? "").join("\n");
  expect(guards).toContain('tool/scaffold-release promote check "$TAG"');
  expect(guards).toContain('mise x -- bun runner/scripts/promote.ts check "$TAG"');
  const edits = allSteps.filter((s) => s.run?.includes("promote promote") || s.run?.includes("promote.ts promote"));
  expect(edits.map((s) => [s.job, s.run])).toEqual([["promote", RUST_EDIT], ["promote", TS_EDIT]]);
});

test("the tool that edits is chosen by an input that defaults to the Rust one, and each tool has its own steps", () => {
  expect(wf.on.workflow_dispatch.inputs!.implementation).toMatchObject({ type: "choice", options: ["rust", "typescript"], default: "rust" });
  const steps = wf.jobs.promote.steps;
  expect(steps.find((s) => s.run === RUST_EDIT)?.if).toBe("inputs.implementation == 'rust'");
  expect(steps.find((s) => s.run === TS_EDIT)?.if).toBe("inputs.implementation == 'typescript'");
  // What only the TypeScript way needs (mise and its tools) is installed only for it.
  const toolchain = steps.filter((s) => s.uses?.startsWith("jdx/mise-action@") || s.run === "mise install --locked");
  expect(toolchain).toHaveLength(2);
  for (const s of toolchain) expect(s.if, s.name ?? s.uses).toBe("inputs.implementation == 'typescript'");
});

test("the guards job fails when the two tools disagree, whatever the Rust one says", () => {
  const run = wf.jobs.guards.steps.find((s) => s.run?.includes("scaffold-release promote check"))?.run ?? "";
  expect(run).toContain('[ "$rust_code" != "$ts_code" ]');
  expect(run).toContain("cmp -s");
  expect(run).toContain("disagree");
  expect(run.trimEnd().endsWith('exit "$rust_code"')).toBe(true);
});

// Phase 2 of docs/ts-tooling-rust-port.md: the promote tool is the Rust crate crates/scaffold-release. Building it runs the build scripts
// of every crate it depends on, so it is built in a job that cannot write and has no token, and the jobs that use it only download the
// finished binary and check its checksum before they run it.
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
  // checkout in three jobs, the tool upload, the tool download in two jobs, and mise in two jobs (guards, and the TypeScript way back).
  expect(uses.length).toBe(8);
  for (const u of uses) expect(u, u).toMatch(/@[0-9a-f]{40}$/);
});

test("it uses no secrets and persists no checkout credentials", () => {
  expect(text).not.toMatch(/secrets\./);
  for (const s of allSteps.filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"], s.job).toBe(false);
});

test("no run script interpolates text a dispatcher controls: the tag goes through env", () => {
  for (const s of allSteps.filter((s) => s.run)) expect(s.run, `${s.job}: ${s.name}`).not.toMatch(/\$\{\{/);
  for (const s of allSteps.filter((s) => s.run?.includes("promote.ts") || s.run?.includes("scaffold-release promote"))) expect(s.env?.TAG, `${s.job}: ${s.name}`).toBe("${{ inputs.tag }}");
});

test("runs never overlap and are never cancelled half way", () => {
  expect(wf.concurrency).toMatchObject({ "cancel-in-progress": false });
});
