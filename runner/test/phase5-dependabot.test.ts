import { expect, test } from "bun:test";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { evalContractJson } from "./support/contract";
import type { Schema } from "../src/schema";

// Decision 86: cargo-dist is the only writer of the generated release.yml, so with `binary` Dependabot ignores the two actions inside it
// (they change through dist-workspace.toml and `dist generate`), and without `binary` it does not.
const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const render = async (fixture: string) =>
  parseContract(await evalContractJson(join(import.meta.dir, "fixtures", fixture, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
type Entry = { "package-ecosystem": string; ignore?: { "dependency-name": string }[]; "exclude-paths"?: string[] };
const entries = (text: string): Record<string, Entry> =>
  Object.fromEntries((Bun.YAML.parse(text) as { updates: Entry[] }).updates.map((u) => [u["package-ecosystem"], u]));
const dependabot = async (fixture: string) => entries((await render(fixture)).files.find((f) => f.path === ".github/dependabot.yml")!.content!);

for (const repo of ["pantheon-adr", "pantheon-journal", "pantheon-skill-auditor"]) {
  test(`${repo}: with a binary, Dependabot ignores upload-artifact and download-artifact in the github-actions entry only`, async () => {
    const e = await dependabot(`real-workflows-${repo}`);
    expect(e["github-actions"].ignore).toEqual([{ "dependency-name": "actions/upload-artifact" }, { "dependency-name": "actions/download-artifact" }]);
    expect(e["cargo"].ignore).toEqual([{ "dependency-name": "pahdk-core" }, { "dependency-name": "pahdk-install" }]);
  });
}

test("the library repository, with no binary, has no ignore in the github-actions entry", async () => {
  const e = await dependabot("real-workflows-pantheon-agentic-harness-dev-kit");
  expect(e["github-actions"].ignore).toBeUndefined();
});

test("the header says why the two actions are ignored", async () => {
  const text = (await render("real-workflows-pantheon-adr")).files.find((f) => f.path === ".github/dependabot.yml")!.content!;
  expect(text).toContain("release.yml");
  expect(text).toContain("dist generate");
});

// Dependabot must not edit the workflow files the package renders: the runner owns them, so a bumped pin is drift (`check` exits 4). Their pins
// move through Pins.pkl (update-pins.yml) and `upgrade`. The list must be exactly the owned workflow files of the same render.
for (const fixture of ["real-workflows-pantheon-adr", "real-workflows-pantheon-agentic-harness-dev-kit"]) {
  test(`${fixture}: the github-actions entry excludes exactly the owned workflow files, and the cargo entry excludes nothing`, async () => {
    const contract = await render(fixture);
    const owned = contract.files.filter((f) => f.class === "owned" && /^\.github\/workflows\/[^/]+\.yml$/.test(f.path)).map((f) => f.path).sort();
    const e = entries(contract.files.find((f) => f.path === ".github/dependabot.yml")!.content!);
    expect(owned.length).toBeGreaterThan(10);
    expect([...(e["github-actions"]["exclude-paths"] ?? [])].sort()).toEqual(owned);
    expect(e["cargo"]["exclude-paths"]).toBeUndefined();
  });
}

test("a repository with skills excludes audit-skill.yml and one without does not; with a binary it excludes dist-check.yml", async () => {
  const withSkills = entries((await render("real-workflows-pantheon-adr")).files.find((f) => f.path === ".github/dependabot.yml")!.content!);
  const library = entries((await render("real-workflows-pantheon-agentic-harness-dev-kit")).files.find((f) => f.path === ".github/dependabot.yml")!.content!);
  expect(withSkills["github-actions"]["exclude-paths"]).toContain(".github/workflows/dist-check.yml");
  expect(library["github-actions"]["exclude-paths"]).not.toContain(".github/workflows/dist-check.yml");
});
