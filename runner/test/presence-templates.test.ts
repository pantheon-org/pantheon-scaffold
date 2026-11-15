import { afterEach, expect, test } from "bun:test";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { evalContractJson } from "./support/contract";
import { cleanupRepos, git, tempRepo } from "./helpers";

// Presence inside templates: what mise.toml, hk.shared.pkl and the headers say when a builtin is left out (options.without).
afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");
const builtins = join(root, "pkl/Builtins");
const folders = readdirSync(builtins, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name).sort();

/** Evaluates a pkl expression over Shared.pkl and Presence.pkl and returns the JSON it prints. */
function shared(body: string): Record<string, unknown> {
  const f = join(tempRepo(), "shared.pkl");
  writeFileSync(f, `import "file://${builtins}/Shared.pkl"\nimport "file://${builtins}/../Presence.pkl"\n${body}\n`);
  const r = Bun.spawnSync(["pkl", "eval", "-f", "json", "--allowed-resources", "file:,prop:", f], { stdout: "pipe", stderr: "pipe" });
  expect(r.stderr.toString()).toBe("");
  return JSON.parse(r.stdout.toString());
}
const flags = (without: string[]) =>
  shared(`local p = new Presence.Present { without = List(${without.map((w) => `"${w}"`).join(", ")}) }\ntools = p.toolFlags\npresent = p.presentFlags`) as {
    tools: Record<string, boolean>;
    present: Record<string, boolean>;
  };
const tables = () => shared("owners = Presence.toolOwners\nalways = Presence.alwaysTools") as { owners: Record<string, string[]>; always: string[] };

test("the present mapping has one entry for every builtin folder, and the tools mapping one for every tool of the table", () => {
  const { tools, present } = flags([]);
  const { owners, always } = tables();
  expect(Object.keys(present).sort()).toEqual(folders);
  expect(Object.keys(tools).sort()).toEqual([...always, ...Object.keys(owners)].sort());
  expect(Object.values(present).every((v) => v === true)).toBe(true);
  expect(Object.values(tools).every((v) => v === true)).toBe(true);
});

test("every owner in the table is a builtin", () => {
  for (const [tool, list] of Object.entries(tables().owners)) for (const o of list) expect(folders, `${tool}: ${o}`).toContain(o);
});

test("a tool is present while any of its owners is, and gone when all are left out", () => {
  const { owners } = tables();
  for (const [tool, list] of Object.entries(owners)) {
    expect(flags(list.slice(0, -1))["tools"][tool], `${tool} with one owner left`).toBe(true);
  }
  expect(flags(["hk"]).tools).toMatchObject({ hk: false, yq: true, shellcheck: true });
  expect(flags(["hk", "inline-scripts"]).tools).toMatchObject({ yq: false, jq: true });
  expect(flags(["hk", "actionlint"]).tools.shellcheck).toBe(false);
  expect(flags(["agnix", "inline-scripts", "release-automerge", "release-bump", "release-please"]).tools.jq).toBe(false);
});

test("pkl and the runner are always pinned", () => {
  const { tools } = flags(["hk", "rumdl", "agnix", "actionlint", "zizmor", "inline-scripts", "release-automerge"]);
  expect(tools).toMatchObject({ pkl: true, runner: true });
});

// A script or template that names a tool needs it through mise, so its builtin must be an owner in the table. Comments and workflow run blocks do
// not count (a runner has its own jq and yq), and mise.toml and hk.shared.pkl are the two files the table governs.
test("every tool a script names outside its comments is owned by that script's builtin", () => {
  const { owners } = tables();
  const names = Object.keys(owners).filter((t) => !["rust"].includes(t)).concat("hk");
  const found: string[] = [];
  for (const b of folders) {
    const dir = join(builtins, b, "assets");
    if (!existsSync(dir)) continue;
    for (const f of readdirSync(dir).filter((x) => x.endsWith(".sh"))) {
      const code = readFileSync(join(dir, f), "utf8").split("\n").filter((l) => !/^\s*#/.test(l)).join("\n");
      for (const t of names) if (new RegExp(`\\b${t}\\b`).test(code) && !(owners[t] ?? [t]).includes(b)) found.push(`${b}/${f} names ${t}`);
    }
  }
  expect(found).toEqual([]);
});

// ---- rendering with builtins left out ----
const COMPOSITION = `workflows { ...Builtins.standardWorkflows(options) }
files {
  ...Builtins.standardFiles(options)
  ...Builtins.baseFiles(options)
  ...Builtins.toolingFiles(options)
  ...Builtins.miseFiles(options)
  ...Builtins.hkFiles(options)
}`;
let n = 0;
type Contract = { files: { path: string; content?: string }[] };
/** Renders the full composition with these builtins left out and returns the files by path. */
async function render(without: string[], extra = ""): Promise<Map<string, string>> {
  const list = without.map((w) => `"${w}"`).join("; ");
  const f = join(tempRepo(), `presence-${n++}.pkl`);
  writeFileSync(
    f,
    `amends "file://${root}/pkl/Config.pkl"\nimport "file://${root}/pkl/Builtins.pkl"\noptions {\n  toolchain = "1.96.1"\n  runnerVersion = "0.1.9"\n  skillName = "demo-skill"\n  crateName = "demo"\n  ${extra}\n  without { ${list} }\n}\n${COMPOSITION}\n`,
  );
  const c = (await evalContractJson(f, root, { allowFilesUnder: [join(root, "pkl")] })) as Contract;
  return new Map(c.files.map((x) => [x.path, x.content ?? ""]));
}
const text = (files: Map<string, string>, path: string): string => {
  const t = files.get(path);
  if (t === undefined) throw new Error(`${path} was not rendered. Files: ${[...files.keys()].join(", ")}`);
  return t;
};

test(".rumdl.toml names mise run lint only when mise is present, and keeps the pull request template ignore either way", async () => {
  const full = text(await render([]), ".rumdl.toml");
  expect(full).toContain("Lint everything with: mise run lint   (or: rumdl check .)");
  for (const without of [["mise"], ["mise", "hk"], ["pull-request-template"]]) {
    const t = text(await render(without), ".rumdl.toml");
    expect(t.includes("mise run lint"), without.join()).toBe(!without.includes("mise"));
    expect(t, without.join()).toContain('".github/pull_request_template.md" = ["MD041"]');
  }
  expect(text(await render(["mise"]), ".rumdl.toml").split("\n")[0]).toBe("# rumdl configuration. Lint everything with: rumdl check .");
});

// ---- mise.toml ----
const mise = async (without: string[]) => text(await render(without), "mise.toml");
/** The tool keys of the [tools] table and the table headers, read from the text. */
const toolKeys = (t: string): string[] => Object.keys((Bun.TOML.parse(t) as { tools: Record<string, unknown> }).tools);
const tableNames = (t: string): string[] => [...t.matchAll(/^\[([^\]]+)\]$/gm)].map((m) => m[1]!);
/** A rendered file is tidy: no empty table, no comment that is followed by a blank line or a table, no two blank lines in a row, and it ends in one newline. */
function expectTidy(t: string, label: string): void {
  const lines = t.split("\n");
  lines.forEach((l, i) => {
    if (/^\[.+\]$/.test(l)) expect(lines[i + 1] ?? "", `${label}: empty table ${l}`).not.toMatch(/^(\[.*|)$/);
    if (/^#/.test(l)) expect(lines[i + 1] ?? "", `${label}: orphan comment at line ${i + 1}`).not.toMatch(/^(\[.*|)$/);
    if (l === "") expect(lines[i + 1], `${label}: two blank lines at ${i + 1}`).not.toBe("");
  });
  expect(t.endsWith("\n") && !t.endsWith("\n\n"), `${label}: ends in one newline`).toBe(true);
}
const ALL_TOOLS = ["rust", "actionlint", "zizmor", "hk", "pkl", "yq", "jq", "shellcheck", "rumdl", "github:agent-sh/agnix", "github:pantheon-org/pantheon-scaffold"];
const ALL_TASKS = ["setup", "lint", "lint-fix", "fmt", "check"];
const tasksOf = (t: string): string[] => Object.keys(((Bun.TOML.parse(t) as { tasks?: Record<string, unknown> }).tasks) ?? {});

test("by default mise.toml pins the eleven tools and has the five tasks, the env and both hooks", async () => {
  const t = await mise([]);
  expect(toolKeys(t)).toEqual(ALL_TOOLS);
  expect(tasksOf(t)).toEqual(ALL_TASKS);
  expect(tableNames(t)).toEqual(["tools", "settings", "env", "hooks", "tasks.setup", "tasks.lint", "tasks.lint-fix", "tasks.fmt", "tasks.check"]);
  expectTidy(t, "default");
});

const CASES: { without: string[]; tools: string[]; tasks: string[]; env: boolean; postinstall: boolean }[] = [
  { without: ["hk"], tools: ALL_TOOLS.filter((x) => x !== "hk"), tasks: ["lint", "lint-fix", "fmt"], env: false, postinstall: false },
  { without: ["hk", "inline-scripts"], tools: ALL_TOOLS.filter((x) => x !== "hk" && x !== "yq"), tasks: ["lint", "lint-fix", "fmt"], env: false, postinstall: false },
  { without: ["hk", "actionlint"], tools: ALL_TOOLS.filter((x) => !["hk", "actionlint", "shellcheck"].includes(x)), tasks: ["lint", "lint-fix", "fmt"], env: false, postinstall: false },
  { without: ["rumdl"], tools: ALL_TOOLS.filter((x) => x !== "rumdl"), tasks: ["setup", "fmt", "check"], env: true, postinstall: true },
  { without: ["rust-ci"], tools: ALL_TOOLS, tasks: ALL_TASKS, env: true, postinstall: true },
  { without: ["rust-ci", "rust-toolchain"], tools: ALL_TOOLS.filter((x) => x !== "rust"), tasks: ["setup", "lint", "lint-fix", "check"], env: true, postinstall: true },
  { without: ["zizmor"], tools: ALL_TOOLS.filter((x) => x !== "zizmor"), tasks: ALL_TASKS, env: true, postinstall: true },
  { without: ["agnix"], tools: ALL_TOOLS.filter((x) => x !== "github:agent-sh/agnix"), tasks: ALL_TASKS, env: true, postinstall: true },
];

for (const c of CASES) {
  test(`mise.toml with ${c.without.join(" and ")} left out pins ${c.tools.length} tools and keeps the tasks of what is present`, async () => {
    const t = await mise(c.without);
    expect(toolKeys(t)).toEqual(c.tools);
    expect(tasksOf(t)).toEqual(c.tasks);
    expect(t.includes("HK_MISE"), "HK_MISE").toBe(c.env);
    expect(t.includes("postinstall"), "postinstall").toBe(c.postinstall);
    expect(t.includes('enter = "mise install -q"'), "enter stays").toBe(true);
    expect(t.includes('"github:pantheon-org/pantheon-scaffold" ='), "the runner pin stays").toBe(true);
    expectTidy(t, c.without.join());
  });
}

test("jq goes only when all five of its owners are left out", async () => {
  expect(toolKeys(await mise(["agnix", "inline-scripts"]))).toContain("jq");
  expect(toolKeys(await mise(["agnix", "inline-scripts", "release-automerge"]))).toContain("jq");
  expect(toolKeys(await mise(["agnix", "inline-scripts", "release-automerge", "release-bump", "release-please"]))).not.toContain("jq");
});

test("the rust comment names check-toolchain-version.sh only with rust-ci, and the actionlint comment names zizmor only with zizmor", async () => {
  expect(await mise([])).toContain("check-toolchain-version.sh");
  const noCi = await mise(["rust-ci"]);
  expect(noCi).not.toContain("check-toolchain-version.sh");
  expect(noCi).toContain("# Same toolchain as rust-toolchain.toml.\nrust = ");
  const noZizmor = await mise(["zizmor"]);
  expect(noZizmor).not.toContain("like zizmor");
  expect(noZizmor).toContain("# so a new release cannot fail a commit that did not touch a rule.");
});

test("real mise reads a reduced mise.toml and lists the tools that are left", async () => {
  const dir = tempRepo();
  mkdirSync(join(dir, "data"));
  const empty = join(dir, "empty.toml");
  writeFileSync(empty, "");
  writeFileSync(join(dir, "mise.toml"), await mise(["hk", "inline-scripts", "rumdl", "agnix"]));
  const env = { ...process.env, MISE_DISABLE_TOOLS: "", MISE_DATA_DIR: join(dir, "data"), MISE_CEILING_PATHS: dirname(dir), MISE_TRUSTED_CONFIG_PATHS: dir, MISE_GLOBAL_CONFIG_FILE: empty, MISE_SYSTEM_CONFIG_FILE: empty };
  const r = Bun.spawnSync(["mise", "ls", "--current", "--json"], { cwd: dir, env, stdout: "pipe", stderr: "pipe" });
  expect(r.exitCode, r.stderr.toString()).toBe(0);
  const listed = Object.keys(JSON.parse(r.stdout.toString()) as Record<string, unknown>).sort();
  expect(listed).toEqual(["actionlint", "github:pantheon-org/pantheon-scaffold", "jq", "pkl", "rust", "shellcheck", "zizmor"].sort());
});

// ---- hk.shared.pkl, with real hk ----
const hkEnv = (): Record<string, string> => ({
  ...(Object.fromEntries(Object.entries(process.env).filter(([k, v]) => v !== undefined && !k.startsWith("HK_"))) as Record<string, string>),
  HK_MISE: "0",
});
const hk = (dir: string, ...args: string[]) => {
  const r = Bun.spawnSync(["hk", ...args], { cwd: dir, env: hkEnv(), stdout: "pipe", stderr: "pipe" });
  return { code: r.exitCode, out: (r.stdout.toString() + r.stderr.toString()).replace(/\x1b\[[0-9;]*m/g, "") };
};
/** Writes the rendered files into a git repository, as a repository adopting the scaffold has them. */
function repoOf(files: Map<string, string>): string {
  const dir = tempRepo();
  git(dir, "init", "-q", "-b", "main");
  for (const [p, content] of files) {
    mkdirSync(dirname(join(dir, p)), { recursive: true });
    writeFileSync(join(dir, p), content);
  }
  git(dir, "add", "-A");
  return dir;
}
const STEPS = ["rustfmt", "rumdl", "yamllint", "shellcheck", "actionlint", "clippy", "test"];
const stepsIn = (shared: string): string[] => STEPS.filter((s) => shared.includes(`["${s}"]`));

const HK_CASES: { without: string[]; steps: string[] }[] = [
  { without: [], steps: STEPS },
  { without: ["rumdl"], steps: STEPS.filter((s) => s !== "rumdl") },
  { without: ["actionlint"], steps: STEPS.filter((s) => s !== "actionlint") },
  { without: ["rust-ci"], steps: STEPS.filter((s) => s !== "clippy" && s !== "test") },
  { without: ["rust-ci", "rust-toolchain"], steps: STEPS.filter((s) => !["clippy", "test", "rustfmt"].includes(s)) },
  { without: ["rumdl", "actionlint", "rust-ci", "rust-toolchain"], steps: ["yamllint", "shellcheck"] },
];

for (const c of HK_CASES) {
  test(`hk.shared.pkl with ${c.without.join(" and ") || "nothing"} left out has the steps ${c.steps.join(", ")}, and real hk accepts it with the seeded hk.pkl`, async () => {
    const files = await render(c.without);
    const shared = text(files, "hk.shared.pkl");
    expect(stepsIn(shared)).toEqual(c.steps);
    expect(shared.split("\n").every((l, i, a) => !(l === "" && a[i + 1] === "")), "no two blank lines").toBe(true);
    const dir = repoOf(files);
    const v = hk(dir, "validate");
    expect(v.code, v.out).toBe(0);
    const plan = hk(dir, "check", "--all", "--plan");
    expect(plan.code, plan.out).toBe(0);
    for (const s of c.steps.filter((x) => x !== "clippy" && x !== "test")) expect(plan.out, s).toContain(s);
    for (const s of STEPS.filter((x) => !c.steps.includes(x) && x !== "clippy" && x !== "test")) expect(plan.out, `no ${s}`).not.toContain(s);
  }, 120_000);
}

// ---- text that names another builtin ----
const MISE_LINE = "Tools are pinned in mise.toml; HK_MISE=1 runs each hook under `mise x`.";

test("the hk files say where tools are pinned only when mise is present", async () => {
  const full = await render([]);
  for (const path of ["hk.shared.pkl", "hk.pkl"]) {
    expect(text(full, path), path).toContain(MISE_LINE);
    const noMise = text(await render(["mise"]), path);
    expect(noMise.includes("mise"), `${path} without mise`).toBe(false);
  }
});

test("the pre-push comment names the rust-ci workflow only when rust-ci is present", async () => {
  expect(text(await render([]), "hk.shared.pkl")).toContain("/// The steps of the pre-push hook, matching the rust-ci workflow.\n");
  const noCi = text(await render(["rust-ci"]), "hk.shared.pkl");
  expect(noCi).toContain("/// The steps of the pre-push hook.\n");
  expect(noCi.includes("rust-ci")).toBe(false);
});

test("the zizmor workflow header runs zizmor through mise, and says it is pinned there, only when mise is present", async () => {
  const header = (files: Map<string, string>) => text(files, ".github/workflows/zizmor.yml");
  const full = header(await render([]));
  expect(full).toContain("(zizmor is pinned in mise.toml)");
  expect(full).toContain("mise exec -- zizmor --min-severity high");
  const noMise = header(await render(["mise"]));
  expect(noMise.includes("mise")).toBe(false);
  expect(noMise).toContain("Run it locally with the same findings:");
  expect(noMise).toContain("GH_TOKEN=$(gh auth token) zizmor --min-severity high");
});
