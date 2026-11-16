import { afterEach, expect, test } from "bun:test";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { evalContractJson } from "./support/contract";
import { cleanupRepos, tempRepo } from "./helpers";

// options.dependabotGithubActions: the github-actions entry of dependabot.yml is rendered unless nothing is left for it to scan. Its exclude-paths
// lists every workflow the package renders, so a repository whose workflows are all rendered has nothing outside it and Dependabot fails with
// dependency_file_not_found (dev-kit, 16-11-2026). `auto` renders the entry for a binary, a composite action, the draft release shape and
// workflow Builtins left out with `without`; `on` and `off` force it.
afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");

let n = 0;
type Contract = { files: { path: string; class: string; content?: string }[] };
async function render(extra: string, without: string[] = []): Promise<Contract> {
  const list = without.map((w) => `"${w}"`).join("; ");
  const f = join(tempRepo(), `dependabot-${n++}.pkl`);
  writeFileSync(
    f,
    `amends "file://${root}/pkl/Config.pkl"\nimport "file://${root}/pkl/Builtins.pkl"\noptions {\n  toolchain = "1.96.1"\n  runnerVersion = "0.4.0"\n  skills = false\n  ${extra.includes("crateName") ? "" : 'crateName = "demo"'}\n  ${extra}\n  without { ${list} }\n}\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles { ...Builtins.standardFiles(options) }\n`,
  );
  return (await evalContractJson(f, root, { allowFilesUnder: [join(root, "pkl")] })) as Contract;
}
type Entry = { "package-ecosystem": string; "exclude-paths"?: string[] };
const dependabot = (c: Contract): { text: string; entries: Record<string, Entry>; updates: Entry[] } => {
  const text = c.files.find((f) => f.path === ".github/dependabot.yml")!.content!;
  const doc = Bun.YAML.parse(text) as { version: number; updates: Entry[] };
  expect(doc.version).toBe(2);
  expect(doc.updates.length).toBeGreaterThan(0);
  return { text, entries: Object.fromEntries(doc.updates.map((u) => [u["package-ecosystem"], u])), updates: doc.updates };
};
const owned = (c: Contract) => c.files.filter((f) => f.class === "owned" && /^\.github\/workflows\/[^/]+\.yml$/.test(f.path)).map((f) => f.path).sort();

const CASES: [string, string, string[], boolean][] = [
  ["a binary", "binary = true", [], true],
  ["a composite action", "binary = false; compositeAction = true", [], true],
  ["the draft release shape", 'binary = false; releaseShape = "draft"', [], true],
  ["builtins left out (one workflow)", "binary = false", ["rust-ci"], true],
  ["a tool Builtin left out (it renders no workflow)", "binary = false", ["rumdl"], false],
  ["a Builtin the toggles already removed (release without binary)", "binary = false", ["release"], false],
  ["everything a workflow Builtin could be left out", "binary = false", ["plumber", "sync-labels", "pr-auto-rebase", "release-automerge", "release-please", "zizmor", "agnix", "aislop", "rust-ci", "cargo-deny", "inline-scripts", "scaffold-check"], true],
  ["a plain library (the dev-kit)", 'binary = false; features = "all"', [], false],
  ["on, on a plain library", 'binary = false; dependabotGithubActions = "on"', [], true],
  ["off, on a binary", 'binary = true; dependabotGithubActions = "off"', [], false],
  ["auto, spelt out, on a plain library", 'binary = false; dependabotGithubActions = "auto"', [], false],
];
for (const [label, extra, without, entry] of CASES) {
  test(`${label}: the github-actions entry is ${entry ? "rendered" : "left out"}, the cargo entry always`, async () => {
    const c = await render(extra, without);
    const d = dependabot(c);
    expect(d.entries["cargo"], "cargo entry").toBeDefined();
    expect(d.entries["github-actions"] !== undefined).toBe(entry);
    if (entry) expect([...(d.entries["github-actions"]["exclude-paths"] ?? [])].sort(), "exclude-paths").toEqual(owned(c));
    expect(d.text.includes("There is no github-actions entry")).toBe(!entry);
    expect(d.text.includes("release.yml caveat"), "the dist note belongs to the entry with a binary").toBe(entry && extra.includes("binary = true"));
  });
}

test("the header has its four variants, and none claims what the file does not do", async () => {
  const plain = dependabot(await render("binary = false")).text;
  expect(plain).toContain("Keeps Cargo dependencies current.");
  expect(plain).not.toContain("SHA-pinned GitHub Actions");
  expect(plain).toContain('options.dependabotGithubActions = "on"');
  expect(plain).toContain("pantheon-agentic-harness-dev-kit crates are git dependencies");
  const plainIgnore = dependabot(await render('binary = false; gitDependencyIgnore = new Listing { "pahdk-core" }')).text;
  expect(plainIgnore).toContain("pahdk-core and pahdk-install are git dependencies");
  expect(plainIgnore).not.toContain("SHA-pinned GitHub Actions");
  const entry = dependabot(await render("binary = true")).text;
  expect(entry).toContain("Keeps Cargo dependencies and the SHA-pinned GitHub Actions current.");
  expect(entry).not.toContain("There is no github-actions entry");
  const entryIgnore = dependabot(await render('binary = true; gitDependencyIgnore = new Listing { "pahdk-core" }')).text;
  expect(entryIgnore).toContain("pahdk-core and pahdk-install are git dependencies");
  expect(entryIgnore).toContain("SHA-pinned GitHub Actions");
});

test("this repository's options render the committed .github/dependabot.yml, so check stays clean", async () => {
  const c = await render('binary = false; crateName = "pantheon-scaffold"; releaseShape = "draft"; versionFile = "pkl/PklProject"; versionCommitScope = "pkl"; releaseAutomerge = "always"; features = "default"');
  const committed = await Bun.file(join(root, ".github/dependabot.yml")).text();
  const rendered = dependabot(c).text;
  expect(Bun.YAML.parse(rendered)).toEqual(Bun.YAML.parse(committed));
});

test("a misspelt value is refused by the render", async () => {
  await expect(render('binary = false; dependabotGithubActions = "yes"')).rejects.toThrow(/dependabotGithubActions|yes/);
});
