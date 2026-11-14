import { afterEach, expect, test } from "bun:test";
import { readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { evalContractJson, scaffoldBin } from "./support/contract";
import { cleanupRepos, commitAll, get, gitRepo, has, put, tempRepo } from "./helpers";

// options.without: builtins left out by name. The aggregators in Builtins.pkl skip a left-out builtin; a builtin that cannot work without another
// fails with an error naming both. The default render (nothing left out) is compared with the baseline of pull request 116 by
// runner/scripts/builtins-baseline, not here.
afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");

const COMPOSITION = `workflows { ...Builtins.standardWorkflows(options) }
files {
  ...Builtins.standardFiles(options)
  ...Builtins.baseFiles(options)
  ...Builtins.toolingFiles(options)
  ...Builtins.miseFiles(options)
  ...Builtins.hkFiles(options)
}`;

let n = 0;
/** Writes a pantheon.pkl with the full composition, the given options (skills and binary on unless set) and an optional composition, and returns its path. */
function config(without: string[], extra = "", composition = COMPOSITION, imports = ""): string {
  const list = without.map((w) => `"${w}"`).join("; ");
  const f = join(tempRepo(), `pantheon-${n++}.pkl`);
  writeFileSync(
    f,
    `amends "file://${root}/pkl/Config.pkl"\nimport "file://${root}/pkl/Builtins.pkl"\n${imports}\noptions {\n  toolchain = "1.96.1"\n  runnerVersion = "0.1.9"\n  skillName = "demo-skill"\n  crateName = "demo"\n  ${extra}\n  without { ${list} }\n}\n${composition}\n`,
  );
  return f;
}
type Contract = { files: { path: string; class: string; mode: string; content?: string }[] };
const evalConfig = (f: string) => evalContractJson(f, root, { allowFilesUnder: [join(root, "pkl")] }) as Promise<Contract>;
async function render(without: string[], extra = ""): Promise<Map<string, string>> {
  const c = await evalConfig(config(without, extra));
  return new Map(c.files.map((x) => [x.path, x.content ?? ""]));
}

const folders = readdirSync(join(root, "pkl/Builtins"), { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name)
  .sort();

test("the registry of builtin names in Shared.pkl is the list of folders under pkl/Builtins", () => {
  const f = join(tempRepo(), "names.pkl");
  writeFileSync(f, `import "file://${root}/pkl/Builtins/Shared.pkl"\nnames = Shared.builtinNames\n`);
  const r = Bun.spawnSync(["pkl", "eval", "-f", "json", "--allowed-resources", "file:,prop:", f], { stdout: "pipe", stderr: "pipe" });
  expect(r.stderr.toString()).toBe("");
  expect([...(JSON.parse(r.stdout.toString()).names as string[])].sort()).toEqual(folders);
  expect(folders).toHaveLength(28);
});

// What leaving each builtin out removes from the full render (skills and binary on), read from the render and checked against what the builtin
// owns. A builtin that others need is left out together with them. dependabot.yml changes whenever a workflow goes, because its exclude list
// follows the workflows that are rendered; nothing else changes and nothing is added.
const DEPENDABOT = ".github/dependabot.yml";
const WF = ".github/workflows/";
const SC = ".github/scripts/";
const EXPECTED: Record<string, { also?: string[]; removed: string[]; dependabot: boolean }> = {
  actionlint: { removed: [".github/actionlint.yaml"], dependabot: false },
  agnix: { removed: [".agnix.toml", `${SC}agnix-assemble-comment.sh`, `${SC}agnix-file-issues.sh`, `${SC}agnix-introduced-errors.sh`, `${WF}agnix.yml`], dependabot: true },
  aislop: { removed: [`${WF}aislop.yml`], dependabot: true },
  "audit-skill": { removed: [`${WF}audit-skill.yml`], dependabot: true },
  "cargo-deny": { also: ["rust-ci"], removed: [`${SC}cargo-deny.sh`, `${SC}check-toolchain-version.sh`, `${WF}cargo-deny.yml`, `${WF}rust-ci.yml`, "deny.toml"], dependabot: true },
  "claude-settings": { removed: [".claude/settings.json"], dependabot: false },
  coderabbit: { removed: [".coderabbit.yaml"], dependabot: false },
  "commit-types": { removed: [".github/commit-types.txt", `${SC}check-commit-types.sh`, `${WF}commit-types.yml`], dependabot: true },
  "conventional-commits": { also: ["commit-types"], removed: [".github/commit-types.txt", `${SC}check-commit-types.sh`, `${WF}commit-types.yml`, `${WF}conventional-commits.yml`], dependabot: true },
  dependabot: { removed: [DEPENDABOT], dependabot: false },
  gitignore: { removed: [".gitignore"], dependabot: false },
  hk: { removed: ["hk.pkl", "hk.shared.pkl"], dependabot: false },
  "inline-scripts": { removed: [`${SC}check-inline-scripts.sh`, `${WF}inline-scripts.yml`], dependabot: true },
  mise: { removed: [".mise/conf.d/repo.toml", "mise.lock", "mise.toml"], dependabot: false },
  plumber: { removed: [".plumber.yaml", `${WF}plumber.yml`], dependabot: true },
  "pr-auto-rebase": { removed: [`${SC}auto-rebase.sh`, `${WF}pr-auto-rebase.yml`], dependabot: true },
  "pull-request-template": { removed: [".github/pull_request_template.md"], dependabot: false },
  release: { removed: [`${WF}dist-check.yml`, `${WF}release.yml`, "dist-workspace.toml"], dependabot: true },
  "release-automerge": { removed: [`${SC}release-automerge.sh`, `${WF}release-automerge.yml`], dependabot: true },
  "release-please": { removed: [`${SC}tag-release.sh`, `${WF}release-please.yml`], dependabot: true },
  rumdl: { removed: [".rumdl.toml"], dependabot: false },
  "rust-ci": { removed: [`${SC}check-toolchain-version.sh`, `${WF}rust-ci.yml`], dependabot: true },
  "rust-toolchain": { also: ["rust-ci"], removed: [`${SC}check-toolchain-version.sh`, `${WF}rust-ci.yml`, "rust-toolchain.toml"], dependabot: true },
  "scaffold-check": { removed: [`${WF}scaffold-check.yml`], dependabot: true },
  security: { removed: ["SECURITY.md"], dependabot: false },
  "sync-labels": { removed: [".github/labels.txt", `${SC}sync-labels.sh`, `${WF}sync-labels.yml`], dependabot: true },
  // Not rendered by the aggregators (it is a consumer workflow a repository adds itself), so leaving it out changes nothing.
  upgrade: { removed: [], dependabot: false },
  zizmor: { removed: [".github/zizmor.yml", `${WF}zizmor.yml`], dependabot: true },
};

test("every builtin has an entry in the table of what leaving it out removes", () => {
  expect(Object.keys(EXPECTED).sort()).toEqual(folders);
});

test("leaving a builtin out removes what it owns, changes only dependabot.yml, and adds nothing", async () => {
  const full = await render([]);
  expect(full.size).toBe(49);
  for (const name of folders) {
    const { also = [], removed, dependabot } = EXPECTED[name]!;
    const r = await render([name, ...also]);
    const gone = [...full.keys()].filter((p) => !r.has(p)).sort();
    const changed = [...full.keys()].filter((p) => r.has(p) && r.get(p) !== full.get(p)).sort();
    const added = [...r.keys()].filter((p) => !full.has(p));
    expect({ name, gone }).toEqual({ name, gone: [...removed].sort() });
    expect({ name, changed }).toEqual({ name, changed: dependabot ? [DEPENDABOT] : [] });
    expect({ name, added }).toEqual({ name, added: [] });
  }
}, 180_000);

test("dependabot's exclude list follows the workflows that are rendered", async () => {
  const full = await render([]);
  const r = await render(["plumber"]);
  expect(full.get(DEPENDABOT)).toContain(`${WF}plumber.yml`);
  expect(r.get(DEPENDABOT)).not.toContain(`${WF}plumber.yml`);
  expect(r.get(DEPENDABOT)).toContain(`${WF}rust-ci.yml`);
});

test("a name that is not a builtin is an error that lists the builtins, and so is an empty or a repeated name", async () => {
  await expect(evalConfig(config(["plumbr"]))).rejects.toThrow(/"plumbr", which is not a builtin of this package version\. Builtins: actionlint, agnix, aislop.*plumber.*zizmor/s);
  await expect(evalConfig(config(["rumdl ", ""]))).rejects.toThrow(/"rumdl ", "", which are not a builtin of this package version/);
  await expect(evalConfig(config(["rumdl", "mise", "rumdl"]))).rejects.toThrow(/names a builtin more than once: rumdl/);
});

test("leaving out a builtin that another needs fails and names both, with the way out", async () => {
  for (const [needs, dependent, reason] of [
    ["conventional-commits", "commit-types", /check-commit-types\.sh reads the copy of the commit types/],
    ["rust-toolchain", "rust-ci", /reads the channel from rust-toolchain\.toml/],
    ["cargo-deny", "rust-ci", /checks the toolchain lines of \.github\/workflows\/cargo-deny\.yml/],
  ] as const) {
    const message = await evalConfig(config([needs])).then(
      () => "",
      (e: Error) => e.message,
    );
    expect(message).toContain(`options.without leaves out "${needs}", which "${dependent}" needs`);
    expect(message).toMatch(reason);
    expect(message).toContain(`Leave "${dependent}" out too (options.without), or keep "${needs}".`);
    // leaving the dependent out as well is the way out, and it renders
    expect((await evalConfig(config([needs, dependent]))).files.length).toBeGreaterThan(0);
  }
});

test("mise and hk are not hard requirements: a repository can supply mise.toml and hk.pkl itself", async () => {
  const r = await render(["mise", "hk"]);
  expect(r.has("mise.toml")).toBe(false);
  expect(r.has("hk.shared.pkl")).toBe(false);
  expect(r.has(`${WF}scaffold-check.yml`)).toBe(true);
  expect(r.has(`${WF}rust-ci.yml`)).toBe(true);
});

test("leaving every builtin out renders nothing and is not an error", async () => {
  const c = await evalConfig(config(folders));
  expect(c.files).toEqual([]);
});

test("a name that a toggle already removes is fine, and the render equals the toggle alone", async () => {
  const toggleSkills = await render([], "skills = false");
  expect(await render(["audit-skill"], "skills = false")).toEqual(toggleSkills);
  const toggleBinary = await render([], "binary = false");
  expect(await render(["release"], "binary = false")).toEqual(toggleBinary);
  expect(toggleSkills.has(`${WF}audit-skill.yml`)).toBe(false);
});

test("features = all with the Rust builtins left out still renders, with no placeholder left in any file", async () => {
  const r = await render(["rust-ci", "rust-toolchain", "cargo-deny"], "features = \"all\"");
  expect(r.has(`${WF}rust-ci.yml`)).toBe(false);
  for (const [path, content] of r) expect({ path, placeholder: content.includes("@@") }).toEqual({ path, placeholder: false });
});

test("a Preset that amends a left-out builtin fails and says options.without may have left it out", async () => {
  const composition = `${COMPOSITION.replace("...Builtins.standardWorkflows(options)", "...Presets.amendWorkflow(Builtins.standardWorkflows(options), \"rust-ci\", (w) -> w)")}`;
  const f = config(["rust-ci"], "", composition, `import "file://${root}/pkl/Presets.pkl"`);
  await expect(evalConfig(f)).rejects.toThrow(/preset: workflow "rust-ci" is not a builtin of this package version\. Builtins: .*A builtin that options\.without leaves out is not in the mapping either\./s);
});

test("a single builtin function still renders when its builtin is left out: without only affects the aggregators", async () => {
  const composition = `workflows { ["rust-ci"] = Builtins.rustCi(options) }\nfiles {}`;
  const c = await evalConfig(config(["rust-ci"], "", composition));
  expect(c.files.map((x) => x.path)).toEqual([`${WF}rust-ci.yml`]);
});

/** Renders a configuration into a repository with the CLI, the way a consumer would; returns the exit code and what it printed. */
async function cliRender(repo: string, without: string[]): Promise<{ code: number; err: string }> {
  const p = Bun.spawn([scaffoldBin(), "render", config(without), "--root", repo, "--allow-local-package", join(root, "pkl")], { stdout: "pipe", stderr: "pipe" });
  const [err] = await Promise.all([new Response(p.stderr).text(), p.exited]);
  return { code: p.exitCode ?? 1, err };
}

test("in an adopted repository, leaving a builtin out deletes the files it rendered, and a seeded file stays", async () => {
  const repo = gitRepo();
  expect((await cliRender(repo, [])).code).toBe(0);
  commitAll(repo, "render");
  expect(has(repo, ".plumber.yaml")).toBe(true);
  expect(has(repo, "hk.pkl")).toBe(true);
  expect((await cliRender(repo, ["plumber", "hk"])).code).toBe(0);
  expect(has(repo, ".plumber.yaml")).toBe(false);
  expect(has(repo, `${WF}plumber.yml`)).toBe(false);
  expect(has(repo, "hk.shared.pkl")).toBe(false);
  // hk.pkl was seeded: it is never deleted, and it now imports a file that is gone, which the docs say.
  expect(has(repo, "hk.pkl")).toBe(true);
  expect(has(repo, `${WF}rust-ci.yml`)).toBe(true);
});

test("in an adopted repository, a file of a left-out builtin that was edited by hand blocks the render", async () => {
  const repo = gitRepo();
  expect((await cliRender(repo, [])).code).toBe(0);
  put(repo, ".plumber.yaml", "mine\n");
  commitAll(repo, "edited by hand");
  const r = await cliRender(repo, ["plumber"]);
  expect(r.code).toBe(3);
  expect(r.err).toContain("it is not deleted");
  expect(get(repo, ".plumber.yaml")).toBe("mine\n");
});

test("turning a builtin back on writes its files again", async () => {
  const repo = gitRepo();
  await cliRender(repo, ["plumber"]);
  expect(has(repo, ".plumber.yaml")).toBe(false);
  commitAll(repo, "plumber out");
  expect((await cliRender(repo, [])).code).toBe(0);
  expect(has(repo, ".plumber.yaml")).toBe(true);
});
