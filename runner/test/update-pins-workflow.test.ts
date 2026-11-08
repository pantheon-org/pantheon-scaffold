import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// Properties of .github/workflows/update-pins.yml, which keeps pkl/Pins.pkl current and publishes the freshness record (decision 83).
// The App steps are unverified until the workflow has run once with the App installed.
const text = readFileSync(join(import.meta.dir, "../../.github/workflows/update-pins.yml"), "utf8");
type Step = { name?: string; id?: string; uses?: string; run?: string; env?: Record<string, string>; with?: Record<string, unknown>; if?: string };
const wf = Bun.YAML.parse(text) as { on: Record<string, any>; permissions: unknown; jobs: Record<string, { permissions: Record<string, string>; steps: Step[] }> };
const steps = Object.values(wf.jobs).flatMap((j) => j.steps);
const runs = steps.filter((s) => s.run);
const idx = (needle: string) => runs.findIndex((s) => s.run!.includes(needle));

test("it runs on a schedule, a push to main that changes the pin tooling, or by hand", () => {
  expect(Object.keys(wf.on).sort()).toEqual(["push", "schedule", "workflow_dispatch"]);
  expect(wf.on.push.branches).toEqual(["main"]);
  expect(wf.on.push.paths).toBeDefined();
  expect(text).not.toContain("pull_request");
});

test("nothing has permissions by default and the job reads contents only: writes use the App token", () => {
  expect(wf.permissions).toEqual({});
  for (const j of Object.values(wf.jobs)) expect(j.permissions).toEqual({ contents: "read" });
  const mint = steps.find((s) => s.id === "app-token")!;
  expect(mint.with).toMatchObject({ "permission-contents": "write", "permission-pull-requests": "write" });
  expect(mint.with).not.toHaveProperty("permission-workflows");
});

test("the only secret is the App key, and the default token is never used to write", () => {
  expect([...text.matchAll(/secrets\.(\w+)/g)].map((m) => m[1])).toEqual(["GH_APP_PRIVATE_KEY"]);
  expect(text).not.toContain("secrets.GITHUB_TOKEN");
});

test("every action is pinned by a 40 character SHA and checkout keeps no credentials", () => {
  for (const s of steps.filter((s) => s.uses)) expect(s.uses, s.uses).toMatch(/@[0-9a-f]{40}$/);
  for (const s of steps.filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"]).toBe(false);
});

test("no run script interpolates an expression, and each is 15 lines or fewer", () => {
  for (const s of runs) {
    expect(s.run, s.name).not.toContain("${{");
    expect(s.run!.trimEnd().split("\n").length, s.name).toBeLessThanOrEqual(15);
  }
});

test("the freshness record is written only after the pins were checked and the pull request step ran", () => {
  const update = idx("update-pins.ts");
  const pr = idx("gh pr create");
  const publish = idx("refs/heads/freshness");
  expect(update).toBeGreaterThanOrEqual(0);
  expect(update).toBeLessThan(pr);
  expect(pr).toBeLessThan(publish);
});

test("the update step asks for the record and the summary, and a failing check fails the job", () => {
  const update = runs[idx("update-pins.ts")];
  expect(update.run).toContain("--freshness-out");
  expect(update.run).toContain("--summary");
  expect(update.run).not.toMatch(/\|\|\s*true/);
  expect(update["continue-on-error" as keyof Step]).toBeUndefined();
});

test("the record goes to the freshness branch, never main", () => {
  const publish = runs[idx("refs/heads/freshness")].run!;
  expect(publish).not.toContain("refs/heads/main");
  expect(publish).toContain("freshness.json");
});

test("the pull request step does nothing when no pin changed", () => {
  expect(runs[idx("gh pr create")].run).toMatch(/git diff --quiet/);
});

test("the pull request is a conventional commit and is opened with the App token", () => {
  const pr = runs[idx("gh pr create")];
  expect(pr.run).toMatch(/--title "chore\(deps\): /);
  expect(pr.env?.GH_TOKEN).toBe("${{ steps.app-token.outputs.token }}");
});
