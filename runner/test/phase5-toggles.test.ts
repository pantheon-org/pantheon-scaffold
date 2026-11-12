import { afterEach, expect, test } from "bun:test";
import { writeFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { evalContractJson } from "../src/pkl";
import { cleanupRepos, commitAll, get, gitRepo, has, put, tempRepo } from "./helpers";

// Phase 5 wave C: the four toggles, each flipped from a base configuration, with the exact set of files it removes and changes, and what
// the runner does with the files a toggle drops. The expected sets were read from the render and checked against what each toggle owns.
afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");
type T = { skills: boolean; binary: boolean; features: "default" | "all"; compositeAction: boolean; skillName?: string | null; crateName?: string | null };
const BASE: T = { skills: true, binary: true, features: "default", compositeAction: false, skillName: "demo-skill", crateName: "demo" };

let n = 0;
/** Writes a pantheon.pkl for these options that uses the standard workflows and files, and returns its path. */
function config(t: T): string {
  const f = join(tempRepo(), `pantheon-${n++}.pkl`);
  const names = [t.skillName ? `skillName = "${t.skillName}"` : "", t.crateName ? `crateName = "${t.crateName}"` : ""].join("\n  ");
  writeFileSync(f, `amends "file://${root}/pkl/Config.pkl"\nimport "file://${root}/pkl/Builtins.pkl"\noptions {\n  toolchain = "1.96.1"\n  ${names}\n  skills = ${t.skills}\n  binary = ${t.binary}\n  features = "${t.features}"\n  compositeAction = ${t.compositeAction}\n}\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles { ...Builtins.standardFiles(options) }\n`);
  return f;
}
type Row = { class: string; mode: string; content?: string };
async function render(t: T): Promise<Map<string, Row>> {
  const c = (await evalContractJson(config(t), root, { allowFilesUnder: [join(root, "pkl")] })) as { files: (Row & { path: string })[] };
  return new Map(c.files.map((f) => [f.path, f]));
}
const same = (a?: Row, b?: Row) => JSON.stringify(a) === JSON.stringify(b);

const EXPECTED: [string, Partial<T>, { removed: string[]; changed: string[] }][] = [
  ["skills off", { skills: false }, {
    removed: [".github/workflows/audit-skill.yml"],
    changed: [".agnix.toml", ".coderabbit.yaml", ".github/dependabot.yml", ".github/workflows/agnix.yml", "SECURITY.md"],
  }],
  ["binary off", { binary: false }, {
    removed: [".github/actionlint.yaml", ".github/scripts/tag-release.sh", ".github/workflows/dist-check.yml", ".github/workflows/release.yml", ".github/zizmor.yml", "dist-workspace.toml"],
    changed: [".github/dependabot.yml", ".github/workflows/release-please.yml", ".github/workflows/zizmor.yml", "SECURITY.md"],
  }],
  ["features all", { features: "all" }, { removed: [], changed: [".github/workflows/rust-ci.yml"] }],
  ["compositeAction on", { compositeAction: true }, { removed: [], changed: [".github/workflows/audit-skill.yml", ".github/workflows/zizmor.yml", ".github/zizmor.yml"] }],
];

const base = await render(BASE);
for (const [label, flip, expected] of EXPECTED) {
  test(`${label}: removes and changes exactly the files that toggle owns, and adds none`, async () => {
    const other = await render({ ...BASE, ...flip });
    const removed = [...base.keys()].filter((p) => !other.has(p)).sort();
    const added = [...other.keys()].filter((p) => !base.has(p));
    const changed = [...base.keys()].filter((p) => other.has(p) && !same(base.get(p), other.get(p))).sort();
    expect({ removed, changed, added }).toEqual({ removed: expected.removed, changed: expected.changed, added: [] });
  });
}

test("a base configuration renders the same twice: the toggles have no hidden input", async () => {
  const again = await render(BASE);
  expect([...again.keys()].sort()).toEqual([...base.keys()].sort());
  for (const p of base.keys()) expect(same(base.get(p), again.get(p)), p).toBe(true);
});

test("skills on without skillName is a clear error that names the option and the way out", async () => {
  await expect(render({ ...BASE, skillName: null })).rejects.toThrow(/options\.skillName is required when options\.skills is true.*skills = false/s);
});

test("skills off needs no skillName, and binary off needs no crateName", async () => {
  const r = await render({ ...BASE, skills: false, binary: false, skillName: null, crateName: null });
  expect(r.has("SECURITY.md")).toBe(true);
  expect(r.has(".github/workflows/audit-skill.yml")).toBe(false);
});

test("binary on without crateName is a clear error that names the option and the way out", async () => {
  await expect(render({ ...BASE, crateName: null })).rejects.toThrow(/options\.crateName is required when options\.binary is true.*binary = false/s);
});

/** Renders a configuration into a repository with the CLI, the way a consumer would. */
async function cliRender(repo: string, t: T): Promise<number> {
  const p = Bun.spawn(["bun", join(root, "runner/src/cli.ts"), "render", config(t), "--root", repo, "--allow-local-package", join(root, "pkl")], { stdout: "pipe", stderr: "pipe" });
  await p.exited;
  return p.exitCode ?? 1;
}
const listing = (repo: string, sub = ""): string[] =>
  readdirSync(join(repo, sub), { withFileTypes: true }).flatMap((e) => (e.name === ".git" || e.name === ".pkl-gen" ? [] : e.isDirectory() ? listing(repo, join(sub, e.name)) : [join(sub, e.name)]));

test("turning skills and binary off deletes only the files they owned, and never touches the external release.yml", async () => {
  const repo = gitRepo();
  expect(await cliRender(repo, BASE)).toBe(0);
  put(repo, ".github/workflows/release.yml", "# written by dist generate\n");
  commitAll(repo, "render, and the generated release.yml");
  const before = new Set(listing(repo));
  expect(await cliRender(repo, { ...BASE, skills: false, binary: false })).toBe(0);
  const after = new Set(listing(repo));
  const deleted = [...before].filter((p) => !after.has(p)).sort();
  expect(deleted).toEqual([".github/actionlint.yaml", ".github/scripts/tag-release.sh", ".github/workflows/audit-skill.yml", ".github/workflows/dist-check.yml", ".github/zizmor.yml", "dist-workspace.toml"]);
  expect(get(repo, ".github/workflows/release.yml")).toBe("# written by dist generate\n");
  expect(has(repo, ".github/workflows/plumber.yml")).toBe(true);
});

test("turning a toggle back on writes its files again", async () => {
  const repo = gitRepo();
  await cliRender(repo, { ...BASE, skills: false });
  expect(has(repo, ".github/workflows/audit-skill.yml")).toBe(false);
  commitAll(repo, "skills off");
  expect(await cliRender(repo, BASE)).toBe(0);
  expect(has(repo, ".github/workflows/audit-skill.yml")).toBe(true);
});
