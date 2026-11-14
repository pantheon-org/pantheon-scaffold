import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { compareDirectives, compareWorkflows } from "./support/equality";
import { evalContractJson } from "./support/contract";
import type { Schema } from "../src/schema";
import { expectPins, unpin } from "./helpers";

// Phase 5 wave D: dist-check.yml (the drift check of the generated release.yml, `binary`) and scaffold-check.yml (`check` in consumer CI,
// every repository). Neither exists in the four repositories yet, so dist-check is asserted by structure and scaffold-check is compared with
// the consumer-check sample that docs/check-in-ci.md describes.
const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const render = async (fixture: string) =>
  parseContract(await evalContractJson(join(import.meta.dir, "fixtures", fixture, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const adr = await render("real-workflows-pantheon-adr");
const kit = await render("real-workflows-pantheon-agentic-harness-dev-kit");
const file = (c: typeof adr, p: string) => c.files.find((f) => f.path === p);
const DIST = ".github/workflows/dist-check.yml";
const CHECK = ".github/workflows/scaffold-check.yml";

type Step = { name?: string; uses?: string; run?: string; env?: Record<string, string>; with?: Record<string, unknown> };
const parsed = (text: string) => Bun.YAML.parse(text) as { on: Record<string, any>; permissions: unknown; jobs: Record<string, { permissions: Record<string, string>; steps: Step[] }> };
const stepsOf = (text: string) => Object.values(parsed(text).jobs).flatMap((j) => j.steps);

test("dist-check.yml is rendered for a repository with a binary and not for one without", () => {
  expect(file(adr, DIST)).toBeDefined();
  expect(file(kit, DIST)).toBeUndefined();
});

test("scaffold-check.yml is rendered for every repository", () => {
  expect(file(adr, CHECK)).toBeDefined();
  expect(file(kit, CHECK)).toBeDefined();
});

test("the dist version the check installs is the one dist-workspace.toml names", () => {
  const toml = file(adr, "dist-workspace.toml")!.content!;
  const version = /cargo-dist-version = "([^"]+)"/.exec(toml)![1];
  const text = file(adr, DIST)!.content!;
  expect(text).toContain(`DIST_VERSION: ${version}`);
});

test("dist-check.yml runs dist generate --check, on a pull request, a push to main or by hand, and never on pull_request_target", () => {
  const text = file(adr, DIST)!.content!;
  expect(Object.keys(parsed(text).on).sort()).toEqual(["pull_request", "push", "workflow_dispatch"]);
  expect(text).not.toContain("pull_request_target");
  expect(stepsOf(text).some((s) => s.run?.includes("dist generate --check"))).toBe(true);
  expect(parsed(text).on.pull_request.paths).toContain("dist-workspace.toml");
  expect(parsed(text).on.pull_request.paths).toContain(".github/workflows/release.yml");
});

test("dist-check.yml uses no secrets, reads contents only, pins every action and keeps no checkout credentials", () => {
  const text = file(adr, DIST)!.content!;
  expect(text).not.toMatch(/secrets\./);
  expect(parsed(text).permissions).toEqual({ contents: "read" });
  for (const s of stepsOf(text).filter((s) => s.uses)) expect(s.uses, s.uses).toMatch(/@[0-9a-f]{40}$/);
  for (const s of stepsOf(text).filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"]).toBe(false);
  for (const s of stepsOf(text).filter((s) => s.run)) {
    expect(s.run, s.name).not.toContain("${{");
    expect(s.run!.trimEnd().split("\n").length, s.name).toBeLessThanOrEqual(15);
  }
});

test("scaffold-check.yml equals the consumer-check sample under P.1, with the pins masked, and the directive comments", () => {
  const sample = readFileSync(join(import.meta.dir, "fixtures/consumer-check.yml"), "utf8");
  const mine = file(adr, CHECK)!.content!;
  expect(compareWorkflows(unpin(sample), unpin(mine))).toEqual({ equal: true, differences: [], duplicateKeys: [] });
  expect(compareDirectives(sample, mine)).toEqual([]);
});

test("scaffold-check.yml is the same for every repository", () => {
  expect(file(kit, CHECK)!.content).toBe(file(adr, CHECK)!.content);
});

test("the pins the two workflows use are the cache, checkout, mise and nothing unlisted", () => {
  const used = adr.pins.map((p) => p.action).filter((a) => ["actions/cache", "actions/checkout", "jdx/mise-action"].includes(a));
  expectPins(adr.pins.filter((p) => used.includes(p.action)), ["actions/cache", "actions/checkout", "jdx/mise-action"]);
});
