import { afterEach, expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { parseContract } from "../src/contract";
import { evalContractJson } from "../src/pkl";
import type { Schema } from "../src/schema";
import { cleanupRepos, git, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const render = async (fixture: string) =>
  parseContract(await evalContractJson(join(import.meta.dir, "fixtures", fixture, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const def = await render("hk");
const all = await render("hk-all");
const content = (c: typeof def, path: string) => c.files.find((f) => f.path === path)!.content!;
const upstream = readFileSync(join(import.meta.dir, "upstream/hk.pkl"), "utf8");
const kit = readFileSync(join(import.meta.dir, "upstream/hk-kit.pkl"), "utf8");
const gitBlobSha = (text: string): string => {
  const body = new TextEncoder().encode(text);
  return new Bun.CryptoHasher("sha1").update(`blob ${body.length}\0`).update(body).digest("hex");
};

/** The text between the line that starts a Mapping and the first closing brace at column 0. */
function mappingBody(text: string, startsWith: string): string {
  const lines = text.split("\n");
  const start = lines.findIndex((l) => l.startsWith(startsWith));
  expect(start, `no line starts with ${startsWith}`).toBeGreaterThanOrEqual(0);
  const end = lines.findIndex((l, i) => i > start && l === "}");
  return lines.slice(start + 1, end).join("\n");
}
const upPre = (t: string) => mappingBody(t, "local preCommit");
const upPush = (t: string) => mappingBody(t, "local prePush");
const shPre = (t: string) => mappingBody(t, "function preCommit()");
const shPush = (t: string) => mappingBody(t, "function prePush()");

test("the pinned upstream copies are the hk.pkl files of pantheon-adr at 1e2fd46 and of the library repo at 42f48b4", () => {
  expect(gitBlobSha(upstream)).toBe("e4bbde76e0d09f3bbc329a01e38a62deb7473513");
  expect(gitBlobSha(kit)).toBe("37ed8613e5f7f095333fc558bbddecf4d5748f2d");
});

test("the hook files are the owned shared steps module and the seeded hk.pkl", () => {
  expect(def.files.map((f) => `${f.class}:${f.path}`).filter((p) => p.includes("hk")).sort()).toEqual(["owned:hk.shared.pkl", "seed:hk.pkl"]);
  expect(all.files.map((f) => f.path).sort()).toEqual(["hk.pkl", "hk.shared.pkl"]);
});

test("the pre-commit steps are byte for byte the ones in pantheon-adr's hk.pkl", () => {
  expect(shPre(content(def, "hk.shared.pkl"))).toBe(upPre(upstream));
});

test("the pre-push steps with the default features are byte for byte the ones in pantheon-adr's hk.pkl", () => {
  expect(shPush(content(def, "hk.shared.pkl"))).toBe(upPush(upstream));
});

test("the pre-push steps with all features are byte for byte the ones in the library repo's hk.pkl", () => {
  expect(shPush(content(all, "hk.shared.pkl"))).toBe(upPush(kit));
});

/** The lines that differ between two texts, from diff -U0. */
function lineDiff(a: string, b: string): { removed: string[]; added: string[] } {
  const dir = tempRepo();
  writeFileSync(join(dir, "a"), a);
  writeFileSync(join(dir, "b"), b);
  const lines = Bun.spawnSync(["diff", "-U0", join(dir, "a"), join(dir, "b")], { stdout: "pipe" }).stdout.toString().split("\n");
  return { removed: lines.filter((l) => l.startsWith("-") && !l.startsWith("---")).map((l) => l.slice(1)), added: lines.filter((l) => l.startsWith("+") && !l.startsWith("+++")).map((l) => l.slice(1)) };
}

test("the pre-commit steps differ from the library repo's only by one comment, which is wording tied to .github/actionlint.yaml", () => {
  const d = lineDiff(upPre(kit), shPre(content(all, "hk.shared.pkl")));
  expect(d.removed).toEqual([
    "    // the workflow files so it only runs when one is staged. It is a local gate:",
    "    // there is no actionlint workflow, so it is not a required status check.",
  ]);
  expect(d.added).toEqual([
    "    // the workflow files so it only runs when one is staged. Reads",
    "    // .github/actionlint.yaml. It is a local gate: there is no actionlint",
    "    // workflow, so it is not a required status check.",
  ]);
});

test("the seeded hk.pkl is pantheon-adr's hooks block with the steps taken from the shared module", () => {
  const hooks = (t: string) => t.slice(t.indexOf("\nhooks {\n"));
  const expected = hooks(upstream).replace("steps = prePush", "steps = shared.prePush()").replaceAll("steps = preCommit", "steps = shared.preCommit()");
  expect(hooks(content(def, "hk.pkl"))).toBe(expected);
});

test("the seeded hk.pkl pins the hk config package, imports the shared module and holds no step of its own", () => {
  const seed = content(def, "hk.pkl");
  expect(seed.split("\n")[0]).toBe('amends "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Config.pkl"');
  expect(seed).toContain('import "hk.shared.pkl" as shared');
  expect(seed).not.toContain('["rustfmt"]');
  expect(seed).toContain("seeded once");
});

test("the shared module says it is rendered and imports the same hk config package", () => {
  const shared = content(def, "hk.shared.pkl");
  expect(shared).toMatch(/^\/\/ Rendered by pantheon-scaffold/);
  expect(shared).toContain('import "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Config.pkl" as hk');
});

// Real hk on the render. The plan only lists steps, so no tool is needed to run these.
function repoWith(c: typeof def, extra: Record<string, string> = {}): string {
  const dir = tempRepo();
  git(dir, "init", "-q", "-b", "main");
  for (const f of c.files) {
    mkdirSync(dirname(join(dir, f.path)), { recursive: true });
    writeFileSync(join(dir, f.path), f.content!, { mode: f.mode === "0755" ? 0o755 : 0o644 });
  }
  for (const [p, text] of Object.entries(extra)) writeFileSync(join(dir, p), text);
  git(dir, "add", "-A");
  return dir;
}
const hkEnv = (): Record<string, string> => ({ ...(Object.fromEntries(Object.entries(process.env).filter(([k, v]) => v !== undefined && !k.startsWith("HK_"))) as Record<string, string>), HK_MISE: "0" });
const hk = (dir: string, ...args: string[]) => {
  const r = Bun.spawnSync(["hk", ...args], { cwd: dir, env: hkEnv(), stdout: "pipe", stderr: "pipe" });
  return { code: r.exitCode, out: (r.stdout.toString() + r.stderr.toString()).replace(/\x1b\[[0-9;]*m/g, "") };
};

test("real hk validates the render with the default features and with all features", () => {
  for (const c of [def, all]) {
    const r = hk(repoWith(c), "validate");
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("is valid");
  }
});

test("real hk plans the five shared steps, and a step a repository adds to the seeded file next to them", () => {
  // A repository adds a step by editing a hook in the seeded file: a module can define hooks only once.
  const seed = content(def, "hk.pkl");
  const at = seed.lastIndexOf("steps = shared.preCommit()");
  const edited = seed.slice(0, at) + 'steps { ...shared.preCommit(); ["my-local-step"] { glob = List("*.txt"); check = "true" } }' + seed.slice(at + "steps = shared.preCommit()".length);
  const dir = repoWith(def, { "hk.pkl": edited, "notes.txt": "x\n" });
  const r = hk(dir, "check", "--all", "--plan");
  expect(r.code, r.out).toBe(0);
  for (const step of ["rustfmt", "rumdl", "yamllint", "shellcheck", "actionlint", "my-local-step"]) expect(r.out, step).toContain(step);
});

// The hook fixers over a fresh render (plan task 3.1). Only shellcheck has files to look at: the render holds shell scripts, TOML and JSON, no
// Rust, Markdown or workflow files. It needs shellcheck on the PATH, which the GitHub Ubuntu runners have.
test("the hook fixers and checks over a fresh render pass and change nothing", () => {
  const dir = repoWith(def);
  const before = git(dir, "status", "--porcelain");
  const fix = hk(dir, "fix", "--all");
  expect(fix.code, fix.out).toBe(0);
  const check = hk(dir, "check", "--all");
  expect(check.code, check.out).toBe(0);
  git(dir, "add", "-A");
  expect(git(dir, "status", "--porcelain")).toBe(before);
  expect(git(dir, "diff", "--stat")).toBe("");
});
