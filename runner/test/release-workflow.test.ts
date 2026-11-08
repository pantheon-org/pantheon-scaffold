import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// Properties of .github/workflows/release.yml that make publishing safe. They are read from the file, so a change that breaks one fails here.
const path = join(import.meta.dir, "../../.github/workflows/release.yml");
const text = readFileSync(path, "utf8");
type Step = { name?: string; uses?: string; run?: string; if?: string; env?: Record<string, string>; with?: Record<string, unknown> };
type Job = { needs?: string | string[]; permissions?: Record<string, string>; steps: Step[]; strategy?: { matrix?: unknown }; if?: string };
const wf = Bun.YAML.parse(text) as { on: Record<string, any>; permissions: unknown; jobs: Record<string, Job>; concurrency?: { "cancel-in-progress"?: boolean } };
const jobs = Object.entries(wf.jobs);
const allSteps = jobs.flatMap(([name, j]) => j.steps.map((s) => ({ job: name, ...s })));

test("it runs on a v-tag push or by hand, never on pull_request_target or a branch push", () => {
  expect(Object.keys(wf.on).sort()).toEqual(["push", "workflow_dispatch"]);
  expect(wf.on.push).toEqual({ tags: ["v[0-9]*"] });
  expect(text).not.toContain("pull_request_target");
});

test("the manual run is a dry run unless told otherwise", () => {
  expect(wf.on.workflow_dispatch.inputs.dry_run).toMatchObject({ type: "boolean", default: true });
});

test("nothing has permissions by default and every job asks for its own", () => {
  expect(wf.permissions).toEqual({});
  for (const [name, j] of jobs) expect(j.permissions, `job ${name}`).toBeDefined();
});

test("only the release job can write contents, attest and use the id token", () => {
  for (const [name, j] of jobs) {
    const writes = Object.entries(j.permissions ?? {}).filter(([, v]) => v === "write").map(([k]) => k).sort();
    expect(writes, `job ${name}`).toEqual(name === "release" ? ["attestations", "contents", "id-token"] : []);
  }
});

test("every action is pinned by a 40 character SHA", () => {
  const uses = allSteps.map((s) => s.uses).filter((u): u is string => !!u && !u.startsWith("./"));
  expect(uses.length).toBeGreaterThan(5);
  for (const u of uses) expect(u, u).toMatch(/@[0-9a-f]{40}$/);
});

test("it uses no secrets and persists no checkout credentials", () => {
  expect(text).not.toMatch(/secrets\./);
  for (const s of allSteps.filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"], s.job).toBe(false);
});

test("no run script interpolates text a pusher controls: tags, refs and inputs go through env", () => {
  for (const s of allSteps.filter((s) => s.run)) {
    expect(s.run, `${s.job}: ${s.name}`).not.toMatch(/\$\{\{\s*(github\.(ref|ref_name|head_ref|event)|inputs\.)/);
  }
});

test("only the release job creates a release, as a pre-release, and never on a dry run", () => {
  const creators = allSteps.filter((s) => s.run?.includes("gh release create"));
  expect(creators.map((s) => s.job)).toEqual(["release"]);
  expect(creators[0].run).toContain("--prerelease");
  expect(creators[0].run).toContain("--verify-tag");
  expect(creators[0].if).toBe("github.event_name == 'push'");
});

test("the attestation is made only on a real release, and covers the assets", () => {
  const attest = allSteps.filter((s) => s.uses?.startsWith("actions/attest-build-provenance@"));
  expect(attest).toHaveLength(1);
  expect(attest[0].job).toBe("release");
  expect(attest[0].if).toBe("github.event_name == 'push'");
  expect(String(attest[0].with?.["subject-path"])).toContain("dist/");
});

test("the tag is checked against PklProject before anything is built", () => {
  expect(wf.jobs.prepare.steps.some((s) => s.run?.includes("release-assets.ts check-tag"))).toBe(true);
  for (const name of ["build", "smoke", "release"]) expect([wf.jobs[name].needs].flat()).toContain(name === "build" ? "prepare" : name === "smoke" ? "build" : "smoke");
});

test("the release is verified from the outside after it is published, on all three platforms", () => {
  const v = wf.jobs["verify-release"];
  expect([v.needs].flat()).toContain("release");
  expect(v.if).toBe("github.event_name == 'push'");
  expect(JSON.stringify(v.strategy?.matrix)).toContain("ubuntu-24.04-arm");
  const run = v.steps.map((s) => s.run ?? "").join("\n");
  expect(run).toContain("gh attestation verify");
  expect(run).toContain("mise install --locked");
  expect(run).toContain("upgrade");
});

test("two releases never run at once, and a running release is not cancelled", () => {
  expect(wf.concurrency?.["cancel-in-progress"]).toBe(false);
});

test("the consumer the release is verified with limits the lock to the supported platforms and checks the assets mise chose", () => {
  const run = wf.jobs["verify-release"].steps.map((s) => s.run ?? "").join("\n");
  expect(run).toContain('lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]');
  expect(run).toContain("pantheon-scaffold-(linux-x64|linux-arm64|darwin-arm64)");
});
