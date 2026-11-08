import { afterEach, expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { ContractError, parseContract } from "../src/contract";
import { SUPPORTED_PLATFORMS } from "../src/lockcheck";
import { evalContractJson } from "../src/pkl";
import type { Schema } from "../src/schema";
import { cleanupRepos, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const render = async (fixture: string) =>
  parseContract(await evalContractJson(join(import.meta.dir, "fixtures", fixture, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const contract = await render("mise");
const file = (path: string) => contract.files.find((f) => f.path === path)!;
const upstream = await Bun.file(join(import.meta.dir, "upstream/mise.toml")).text();
const gitBlobSha = (text: string): string => {
  const body = new TextEncoder().encode(text);
  return new Bun.CryptoHasher("sha1").update(`blob ${body.length}\0`).update(body).digest("hex");
};

test("the mise files are the owned mise.toml, the external mise.lock and the seeded per-repo config", () => {
  expect(contract.files.map((f) => `${f.class}:${f.path}`).sort()).toEqual(["external:mise.lock", "owned:mise.toml", "seed:.mise/conf.d/repo.toml"]);
  expect(file("mise.lock").content).toBeUndefined();
  for (const f of contract.files) expect(f.mode, f.path).toBe("0644");
});

test("the pinned upstream copy is the pantheon-adr mise.toml at 1e2fd46 (git blob 16c264c)", () => {
  expect(gitBlobSha(upstream)).toBe("16c264c54f1408330819ecd81131de90eb9b49e4");
});

/** The lines that differ between two texts, from diff -U0, as { removed, added }. */
function lineDiff(a: string, b: string): { removed: string[]; added: string[] } {
  const dir = tempRepo();
  writeFileSync(join(dir, "a"), a);
  writeFileSync(join(dir, "b"), b);
  const r = Bun.spawnSync(["diff", "-U0", join(dir, "a"), join(dir, "b")], { stdout: "pipe" });
  const lines = r.stdout.toString().split("\n");
  return { removed: lines.filter((l) => l.startsWith("-") && !l.startsWith("---")).map((l) => l.slice(1)), added: lines.filter((l) => l.startsWith("+") && !l.startsWith("+++")).map((l) => l.slice(1)) };
}

test("mise.toml differs from the repositories' current file only by the named additions and the two comments", () => {
  const d = lineDiff(upstream, file("mise.toml").content!);
  expect(d.removed).toEqual([
    "# Workflow security linter. Keep in sync with `version` in",
    "# .github/workflows/zizmor.yml.",
    "# Lints skill frontmatter and agent files. Keep in sync with AGNIX_VERSION in",
    "# .github/workflows/agnix.yml.",
  ]);
  expect(d.added).toEqual([
    "# Workflow security linter. Pinned in the scaffold package (Tools.pkl).",
    "# Lints skill frontmatter and agent files. Pinned in the scaffold package (Tools.pkl).",
    "# The scaffold runner, released together with the package that rendered this file.",
    '"github:pantheon-org/pantheon-scaffold" = "0.1.2"',
    "# Record every tool in mise.lock with its checksum, and refuse a tool whose provenance",
    "# attestation does not verify. Limited to the platforms the runner is built for: with no",
    "# list mise locks every platform and names the package zip for the ones with no runner.",
    "lockfile = true",
    "locked_verify_provenance = true",
    'lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]',
  ]);
});

test("the tool specifiers are the ones the repositories use today: exact stays exact and latest stays latest (decision q14, option A)", () => {
  const text = file("mise.toml").content!;
  for (const pin of ['actionlint = "1.7.12"', 'zizmor = "1.32.0"', 'rumdl = "0.2.77"', '"github:agent-sh/agnix" = "0.56.6"', 'hk = "latest"', 'pkl = "latest"', 'yq = "latest"', 'jq = "latest"', 'shellcheck = "latest"', 'minimum_release_age = "7d"'])
    expect(text, pin).toContain(pin);
});

test("lockfile_platforms is exactly the platforms the runner is built for", () => {
  const m = /lockfile_platforms = \[(.*)\]/.exec(file("mise.toml").content!)!;
  expect(m[1].split(",").map((s) => s.trim().replaceAll('"', ""))).toEqual(Object.keys(SUPPORTED_PLATFORMS));
});

test("the toolchain option reaches the rust entry", async () => {
  expect(file("mise.toml").content).toContain('rust = { version = "1.96.1", components = "rustfmt,clippy" }');
});

test("the seeded per-repo config says what it is for and holds no tools", () => {
  const seed = file(".mise/conf.d/repo.toml").content!;
  expect(seed).toMatch(/seeded once/i);
  expect(seed).toContain("[tools]");
  expect(seed).not.toMatch(/^[a-z"].*=/m);
});

test("using the mise builtin without a runner version and not as a released package fails with a clear message", async () => {
  await expect(render("mise-no-runner")).rejects.toThrow(ContractError);
  await expect(render("mise-no-runner")).rejects.toThrow("runnerVersion");
});

// Decision 29 with the real mise: the per-repo file adds tools, and the owned file wins a conflict. The data directory is empty and only
// the tools the config asks for are listed (--current), so the result does not depend on what is installed on the machine.
test("real mise reads both files, adds the repo's own tool and lets the owned mise.toml win a conflict", () => {
  const dir = tempRepo();
  mkdirSync(join(dir, ".mise/conf.d"), { recursive: true });
  mkdirSync(join(dir, "data"));
  const empty = join(dir, "empty.toml");
  writeFileSync(empty, "");
  writeFileSync(join(dir, "mise.toml"), file("mise.toml").content!);
  // The seed ends with an empty [tools] section, so a repository adds its tools right after it.
  writeFileSync(join(dir, ".mise/conf.d/repo.toml"), `${file(".mise/conf.d/repo.toml").content!}jq = "1.6"\ntokei = "13.0.0"\n`);
  const env = { ...process.env, MISE_DATA_DIR: join(dir, "data"), MISE_CEILING_PATHS: dirname(dir), MISE_TRUSTED_CONFIG_PATHS: dir, MISE_GLOBAL_CONFIG_FILE: empty, MISE_SYSTEM_CONFIG_FILE: empty };
  const r = Bun.spawnSync(["mise", "ls", "--current", "--json"], { cwd: dir, env, stdout: "pipe", stderr: "pipe" });
  expect(r.exitCode, r.stderr.toString()).toBe(0);
  const tools = JSON.parse(r.stdout.toString()) as Record<string, { requested_version: string; source: { path: string } }[]>;
  const entry = (name: string) => ({ requested: tools[name][0].requested_version, source: tools[name][0].source.path.split("/").slice(-3).join("/").replace(/^.*\/(mise\.toml)$/, "$1").replace(/^.*(\.mise\/conf\.d\/repo\.toml)$/, "$1") });
  expect(entry("jq")).toEqual({ requested: "latest", source: "mise.toml" });
  expect(entry("tokei")).toEqual({ requested: "13.0.0", source: ".mise/conf.d/repo.toml" });
  expect(entry("rust").requested).toBe("1.96.1");
  expect(entry("github:pantheon-org/pantheon-scaffold").requested).toBe("0.1.2");
  expect(Object.keys(tools)).toHaveLength(12);
});
