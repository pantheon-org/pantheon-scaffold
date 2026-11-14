import { afterEach, expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { evalContractJson } from "./support/contract";
import { cleanupRepos, tempRepo } from "./helpers";

// Presence inside templates: what mise.toml, hk.shared.pkl and the headers say when a builtin is left out (options.without).
afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");
const builtins = join(root, "pkl/Builtins");
const folders = readdirSync(builtins, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name).sort();

/** Evaluates a pkl expression over Shared.pkl and returns the JSON it prints. */
function shared(body: string): Record<string, unknown> {
  const f = join(tempRepo(), "shared.pkl");
  writeFileSync(f, `import "file://${builtins}/Shared.pkl"\n${body}\n`);
  const r = Bun.spawnSync(["pkl", "eval", "-f", "json", "--allowed-resources", "file:,prop:", f], { stdout: "pipe", stderr: "pipe" });
  expect(r.stderr.toString()).toBe("");
  return JSON.parse(r.stdout.toString());
}
const flags = (without: string[]) =>
  shared(`local p = new Shared.Present { without = List(${without.map((w) => `"${w}"`).join(", ")}) }\ntools = Shared.toolFlags(p)\npresent = Shared.presentFlags(p)`) as {
    tools: Record<string, boolean>;
    present: Record<string, boolean>;
  };
const tables = () => shared("owners = Shared.toolOwners\nalways = Shared.alwaysTools") as { owners: Record<string, string[]>; always: string[] };

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
  expect(flags(["agnix", "inline-scripts", "release-automerge"]).tools.jq).toBe(false);
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
