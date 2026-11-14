import { expectPins, unpin } from "./helpers";
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { compareDirectives, compareWorkflows } from "./support/equality";
import { evalContractJson } from "./support/contract";
import type { Schema } from "../src/schema";

const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const render = async (fixture: string) =>
  parseContract(await evalContractJson(join(import.meta.dir, "fixtures", fixture, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const contract = await render("workflows-a");
const file = (c: typeof contract, path: string) => c.files.find((f) => f.path === path)!;
const upstream = (name: string) => readFileSync(join(import.meta.dir, "upstream/workflows", `${name}.yml`), "utf8");
const gitBlobSha = (text: string): string => {
  const body = new TextEncoder().encode(text);
  return new Bun.CryptoHasher("sha1").update(`blob ${body.length}\0`).update(body).digest("hex");
};

const WORKFLOWS: Record<string, string> = {
  plumber: "a3468d27b20b77bb39909a2dbf22b173bf059d13",
  "sync-labels": "b81cb37d260a91399939b25198803c339992418a",
  "pr-auto-rebase": "a9931a88d908eba56fe03f54d31a711f5c593e27",
  "release-automerge": "fd5d20a13f93b26d153222769188299204b93fc9",
  "commit-types": "f507416d9636d72b9091220542d906188093c670",
  "conventional-commits": "e3e06519eaa74194c0052eeec358e6fcbf4d265c",
};

test("the pinned upstream copies are the pantheon-adr workflows at 1e2fd46, identical in all four repositories", () => {
  for (const [name, blob] of Object.entries(WORKFLOWS)) expect(gitBlobSha(upstream(name)), name).toBe(blob);
});

test("wave A renders the six workflows and the four config files they use", () => {
  expect(contract.files.map((f) => f.path).sort()).toEqual([
    ".github/commit-types.txt", ".github/labels.txt", ".github/pull_request_template.md",
    ".github/workflows/commit-types.yml", ".github/workflows/conventional-commits.yml", ".github/workflows/plumber.yml",
    ".github/workflows/pr-auto-rebase.yml", ".github/workflows/release-automerge.yml", ".github/workflows/sync-labels.yml", ".plumber.yaml",
  ]);
});

test("the one per-file ignore in the rendered .rumdl.toml is for the pull request template this contract renders", async () => {
  const tooling = await render("tooling");
  const toml = Bun.TOML.parse(file(tooling, ".rumdl.toml").content!) as { "per-file-ignores"?: Record<string, string[]> };
  const ignored = Object.keys(toml["per-file-ignores"] ?? {});
  expect(ignored).toHaveLength(1);
  expect(file(contract, ignored[0])).toBeDefined();
  expect(ignored[0]).toBe(".github/pull_request_template.md");
});

for (const name of Object.keys(WORKFLOWS)) {
  test(`${name}.yml equals the pinned pantheon-adr copy under the P.1 rule`, () => {
    expect(compareWorkflows(unpin(upstream(name)), unpin(file(contract, `.github/workflows/${name}.yml`).content!))).toEqual({ equal: true, differences: [], duplicateKeys: [] });
  });

  test(`${name}.yml keeps every comment a tool acts on`, () => {
    expect(compareDirectives(upstream(name), file(contract, `.github/workflows/${name}.yml`).content!)).toEqual([]);
  });

  test(`${name}.yml keeps its job ids and job names, which the required status checks depend on`, () => {
    const jobs = (text: string) => Object.entries((Bun.YAML.parse(text) as { jobs: Record<string, { name?: string }> }).jobs).map(([id, j]) => [id, j.name]);
    expect(jobs(file(contract, `.github/workflows/${name}.yml`).content!)).toEqual(jobs(upstream(name)));
  });
}

test("the zizmor ignore comment on the pull_request_target trigger is rendered on the on: line", () => {
  const text = file(contract, ".github/workflows/conventional-commits.yml").content!;
  expect(text).toContain("\n'on': # zizmor: ignore[dangerous-triggers] reads only the PR title; checks out and runs no PR code\n");
});

test("the four config files are byte for byte the pinned pantheon-adr files and not executable", async () => {
  const blobs = (await Bun.file(join(import.meta.dir, "upstream/workflow-files-blobs.json")).json()) as Record<string, string>;
  expect(Object.keys(blobs)).toHaveLength(4);
  for (const [path, blob] of Object.entries(blobs)) {
    expect(gitBlobSha(file(contract, path).content!), path).toBe(blob);
    expect(file(contract, path).mode, path).toBe("0644");
  }
});

test("the pins are the three actions the workflows use, each a commit with a version tag", () => {
  expectPins(contract.pins, ["actions/checkout", "actions/create-github-app-token", "getplumber/plumber"]);
});

test("wave A uses no escape hatch", () => expect(contract.escapeHatch.uses).toBe(0));

test("the App client id is an option, with the pantheon-ai-bot id as the default", async () => {
  for (const name of ["pr-auto-rebase", "release-automerge"]) expect(file(contract, `.github/workflows/${name}.yml`).content, name).toContain('client-id: Iv23ligzn16vkDCWfHgc');
  const custom = await render("workflows-a-client-id");
  for (const name of ["pr-auto-rebase", "release-automerge"]) {
    const text = file(custom, `.github/workflows/${name}.yml`).content!;
    expect(text, name).toContain("client-id: Iv-test-client-id");
    expect(text, name).not.toContain("Iv23ligzn16vkDCWfHgc");
  }
});

// actionlint over the render (plan task 4.4). It reads the workflow files and, when shellcheck is on the PATH, also checks the run scripts.
test("actionlint is clean on the six rendered workflows", async () => {
  const { mkdirSync, mkdtempSync, rmSync, writeFileSync } = await import("node:fs");
  const { tmpdir } = await import("node:os");
  const dir = mkdtempSync(join(tmpdir(), "scaffold-actionlint-"));
  try {
    mkdirSync(join(dir, ".github/workflows"), { recursive: true });
    for (const f of contract.files.filter((f) => f.path.startsWith(".github/workflows/"))) writeFileSync(join(dir, f.path), f.content!);
    const files = contract.files.filter((f) => f.path.startsWith(".github/workflows/")).map((f) => f.path);
    const r = Bun.spawnSync(["actionlint", ...files], { cwd: dir, stdout: "pipe", stderr: "pipe" });
    expect(r.stdout.toString() + r.stderr.toString(), "actionlint output").toBe("");
    expect(r.exitCode).toBe(0);
  } finally {
    if (dir.startsWith(tmpdir()) && dir.includes("scaffold-actionlint-")) rmSync(dir, { recursive: true, force: true });
  }
});

// Phase 7 benefit test (task 7.6): the one canonical change released as 0.1.4. plumber.yml says it is generated, in its header comment, so the
// change reaches every repository as a comment-only difference, which the proof already accepts.
const NOTICE = "# Generated by pantheon-scaffold from pantheon.pkl: change it there, not in this file.\n";
test("plumber.yml ends its header with the notice that it is generated, and no other workflow carries it", () => {
  const text = file(contract, ".github/workflows/plumber.yml").content!;
  expect(text.startsWith("# CI/CD security scan")).toBe(true);
  expect(text).toContain(`#\n${NOTICE}name: Plumber\n`);
  for (const name of Object.keys(WORKFLOWS).filter((n) => n !== "plumber")) expect(file(contract, `.github/workflows/${name}.yml`).content, name).not.toContain(NOTICE);
});
