import { expect, test } from "bun:test";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { evalContractJson } from "../src/pkl";
import type { Schema } from "../src/schema";

// Properties of the consumer upgrade workflow (wave C). There is no upstream copy to compare with: it is new, so its safety is asserted here.
// The steps that use the GitHub App are built but unverified until the App has `workflows: write` (decision 83, plan P.4).
const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const contract = parseContract(await evalContractJson(join(import.meta.dir, "fixtures/workflows-c/pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const file = contract.files.find((f) => f.path === ".github/workflows/upgrade-scaffold.yml")!;
const text = file.content!;
type Step = { name?: string; id?: string; uses?: string; run?: string; env?: Record<string, string>; with?: Record<string, unknown>; if?: string };
const wf = Bun.YAML.parse(text) as { on: Record<string, any>; permissions: Record<string, string>; jobs: Record<string, { permissions: Record<string, string>; steps: Step[] }> };
const steps = Object.values(wf.jobs).flatMap((j) => j.steps);
const runs = steps.filter((s) => s.run);

test("it renders one owned file that detach deletes", () => {
  expect(contract.files.map((f) => f.path)).toEqual([".github/workflows/upgrade-scaffold.yml"]);
  expect(file.class).toBe("owned");
  expect(file.removeOnDetach).toBe(true);
});

test("it runs on a schedule or by hand, never on a pull request or a push", () => {
  expect(Object.keys(wf.on).sort()).toEqual(["schedule", "workflow_dispatch"]);
  expect(text).not.toContain("pull_request");
});

test("nothing has permissions by default and the job reads contents only", () => {
  expect(wf.permissions).toEqual({});
  for (const j of Object.values(wf.jobs)) expect(j.permissions).toEqual({ contents: "read" });
});

test("the App token is the only credential, and it asks for workflows write as well as contents and pull requests", () => {
  expect([...text.matchAll(/secrets\.(\w+)/g)].map((m) => m[1])).toEqual(["GH_APP_PRIVATE_KEY"]);
  const mint = steps.find((s) => s.id === "app-token")!;
  expect(mint.with).toMatchObject({ "permission-contents": "write", "permission-pull-requests": "write", "permission-workflows": "write" });
});

test("every action is pinned by a 40 character SHA and checkout keeps no credentials", () => {
  for (const s of steps.filter((s) => s.uses)) expect(s.uses, s.uses).toMatch(/@[0-9a-f]{40}$/);
  for (const s of steps.filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"]).toBe(false);
});

test("no run script interpolates an expression: everything goes through env", () => {
  for (const s of runs) expect(s.run, s.name).not.toContain("${{");
});

test("every run script is 15 lines or fewer", () => {
  for (const s of runs) expect(s.run!.trimEnd().split("\n").length, s.name).toBeLessThanOrEqual(15);
});

test("it checks freshness, then finds the latest release, then upgrades, in that order", () => {
  const at = (needle: string) => runs.findIndex((s) => s.run!.includes(needle));
  expect(at("pantheon-scaffold freshness")).toBeGreaterThanOrEqual(0);
  expect(at("pantheon-scaffold freshness")).toBeLessThan(at("pantheon-scaffold latest"));
  expect(at("pantheon-scaffold latest")).toBeLessThan(at("pantheon-scaffold upgrade"));
});

test("a repository already on the latest release opens no pull request", () => {
  const upgrade = runs.find((s) => s.run!.includes("pantheon-scaffold upgrade"))!;
  const pr = runs.find((s) => s.run!.includes("gh pr create"))!;
  expect(upgrade.if).toBeDefined();
  expect(pr.if).toBeDefined();
});

test("the pull request is opened with the App token, never the default token", () => {
  const pr = runs.find((s) => s.run!.includes("gh pr create"))!;
  expect(pr.env?.GH_TOKEN).toBe("${{ steps.app-token.outputs.token }}");
  expect(text).not.toContain("secrets.GITHUB_TOKEN");
  expect(text).not.toContain("github.token");
});

test("the commit and the pull request title are conventional commits", () => {
  const pr = runs.find((s) => s.run!.includes("gh pr create"))!.run!;
  expect(pr).toMatch(/commit .*"chore: upgrade pantheon-scaffold to /);
  expect(pr).toMatch(/--title "chore: upgrade pantheon-scaffold to /);
});
