import { afterEach, expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
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
