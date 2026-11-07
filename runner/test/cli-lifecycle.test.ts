import { afterEach, expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { cleanupRepos, get, has, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const cli = join(import.meta.dir, "../src/cli.ts");
const uri = (v: string) => `package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${v}/pantheon-scaffold@${v}#/Config.pkl`;

type Extra = Record<string, unknown>;
/** A fake pkl that records its arguments and prints a contract with a package URI, so the lifecycle rules run without a package. */
function setup(version: string, extra: Extra = {}) {
  const dir = tempRepo();
  mkdirSync(join(dir, "bin"));
  const contract = { schemaVersion: 1, files: [{ path: "a.txt", class: "owned", mode: "0644", content: "x\n" }], pins: [], escapeHatch: { uses: 0, constructs: 0 }, packageUri: uri(version), ...extra };
  writeFileSync(join(dir, "contract.json"), JSON.stringify(contract));
  writeFileSync(join(dir, "bin/pkl"), `#!/bin/sh\nprintf '%s\\n' "$@" > "${join(dir, "args")}"\ncat "${join(dir, "contract.json")}"\n`);
  chmodSync(join(dir, "bin/pkl"), 0o755);
  const repo = tempRepo();
  writeFileSync(join(repo, "pantheon.pkl"), "# fake\n");
  const run = async (args: string[], env: Record<string, string> = {}) => {
    const p = Bun.spawn(["bun", cli, ...args], { cwd: repo, stdout: "pipe", stderr: "pipe", env: { ...process.env, PATH: `${join(dir, "bin")}:${process.env.PATH}`, ...env } });
    const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
    return { out: out.trim(), err: err.trim(), code };
  };
  return { repo, run, args: () => readFileSync(join(dir, "args"), "utf8").trim().split("\n") };
}

const badList = (version: string) => JSON.stringify({ versions: [{ version, reason: "renders a broken deny.toml" }] });
const serve = (body: string) => Bun.serve({ port: 0, fetch: () => new Response(body) });
const noNet = { PANTHEON_SCAFFOLD_KNOWN_BAD_URL: "http://127.0.0.1:9/known-bad.json" };

test("render records the package version in the manifest", async () => {
  const s = setup("0.2.0");
  const r = await s.run(["render", "pantheon.pkl", "--root", s.repo], noNet);
  expect(r.code).toBe(0);
  expect(JSON.parse(get(s.repo, ".pkl-gen/manifest.json")).packageVersion).toBe("0.2.0");
});

test("an older package than the last render is refused with exit 3, and --force renders it", async () => {
  const s = setup("0.2.0");
  put(s.repo, ".pkl-gen/manifest.json", JSON.stringify({ manifestVersion: 1, packageVersion: "0.3.0", files: {} }));
  const r = await s.run(["render", "pantheon.pkl", "--root", s.repo], noNet);
  expect(r.code).toBe(3);
  expect(r.err).toContain("package 0.2.0 is older than 0.3.0");
  expect(has(s.repo, "a.txt")).toBe(false);
  expect((await s.run(["render", "pantheon.pkl", "--root", s.repo, "--force"], noNet)).code).toBe(0);
  expect(has(s.repo, "a.txt")).toBe(true);
});

test("a runner older than the package requires fails with exit 1 and a clean message", async () => {
  const s = setup("0.2.0", { requires: { runner: "9.0.0" } });
  const r = await s.run(["render", "pantheon.pkl", "--root", s.repo], noNet);
  expect(r.code).toBe(1);
  expect(r.err).toContain("older than the 9.0.0 this package requires");
  expect(r.err).not.toContain("    at ");
});

test("a known-bad package version warns and still renders, and --strict refuses before writing", async () => {
  const server = serve(badList("0.2.0"));
  try {
    const env = { PANTHEON_SCAFFOLD_KNOWN_BAD_URL: `http://localhost:${server.port}/known-bad.json` };
    const s = setup("0.2.0");
    const strict = await s.run(["render", "pantheon.pkl", "--root", s.repo, "--strict"], env);
    expect(strict.code).toBe(3);
    expect(strict.err).toContain("known-bad list: renders a broken deny.toml");
    expect(has(s.repo, "a.txt")).toBe(false);
    const warn = await s.run(["render", "pantheon.pkl", "--root", s.repo], env);
    expect(warn.code).toBe(0);
    expect(warn.err).toContain("warning: package 0.2.0 is on the known-bad list");
  } finally {
    server.stop(true);
  }
});

test("check also refuses a known-bad version with --strict", async () => {
  const server = serve(badList("0.2.0"));
  try {
    const s = setup("0.2.0");
    const r = await s.run(["check", "pantheon.pkl", "--root", s.repo, "--strict"], { PANTHEON_SCAFFOLD_KNOWN_BAD_URL: `http://localhost:${server.port}/known-bad.json` });
    expect(r.code).toBe(3);
  } finally {
    server.stop(true);
  }
});

test("an unreachable known-bad list falls back to the bundled snapshot with a note and never stops the run", async () => {
  const s = setup("0.2.0");
  const r = await s.run(["render", "pantheon.pkl", "--root", s.repo], noNet);
  expect(r.code).toBe(0);
  expect(r.err).toContain("using the bundled snapshot");
});

test("a package with no version (local) does not fetch the known-bad list at all", async () => {
  const s = setup("0.2.0", { packageUri: "file:///work/pkl/Config.pkl" });
  const r = await s.run(["render", "pantheon.pkl", "--root", s.repo], noNet);
  expect(r.code).toBe(0);
  expect(r.err).toBe("");
});

test("a disabled Security builtin is a warning on stderr and does not change the exit code", async () => {
  const s = setup("0.2.0", { securityBuiltins: [{ name: "cargo-deny", enabled: false }] });
  const r = await s.run(["render", "pantheon.pkl", "--root", s.repo, "--strict"], noNet);
  expect(r.code).toBe(0);
  expect(r.err).toContain("warning: required Security builtin cargo-deny is disabled");
});

test("pkl gets the package cache under the repository by default, and --cache-dir overrides it", async () => {
  const s = setup("0.2.0");
  await s.run(["check", "pantheon.pkl", "--root", s.repo], noNet);
  let a = s.args();
  expect(a[a.indexOf("--cache-dir") + 1]).toBe(join(s.repo, ".pkl-gen/cache"));
  await s.run(["check", "pantheon.pkl", "--root", s.repo, "--cache-dir", "/elsewhere/cache"], noNet);
  a = s.args();
  expect(a[a.indexOf("--cache-dir") + 1]).toBe("/elsewhere/cache");
});

test("detach removes the manifest and leaves the files, with exit 0", async () => {
  const s = setup("0.2.0");
  await s.run(["render", "pantheon.pkl", "--root", s.repo], noNet);
  const r = await s.run(["detach", "--root", s.repo]);
  expect(r.code).toBe(0);
  expect(r.out).toContain("detached");
  expect(has(s.repo, ".pkl-gen")).toBe(false);
  expect(get(s.repo, "a.txt")).toBe("x\n");
});

test("detach --dry-run changes nothing, and detach without a manifest is exit 1", async () => {
  const s = setup("0.2.0");
  expect((await s.run(["detach", "--root", s.repo])).code).toBe(1);
  await s.run(["render", "pantheon.pkl", "--root", s.repo], noNet);
  const r = await s.run(["detach", "--root", s.repo, "--dry-run"]);
  expect(r.code).toBe(0);
  expect(r.out).toContain("dry run");
  expect(has(s.repo, ".pkl-gen/manifest.json")).toBe(true);
});

test("detach takes no config file and an unknown option is a usage error", async () => {
  const s = setup("0.2.0");
  expect((await s.run(["detach", "--nope"])).code).toBe(2);
});

test("--repair rebuilds an unreadable manifest, so the version rules must not read it first", async () => {
  const s = setup("0.2.0");
  put(s.repo, "a.txt", "x\n");
  put(s.repo, ".pkl-gen/manifest.json", "<<<<<<< merge conflict");
  const broken = await s.run(["render", "pantheon.pkl", "--root", s.repo], noNet);
  expect(broken.code).toBe(1);
  expect(broken.err).toContain("--repair");
  const fixed = await s.run(["render", "pantheon.pkl", "--root", s.repo, "--repair"], noNet);
  expect(fixed.code).toBe(0);
  expect(JSON.parse(get(s.repo, ".pkl-gen/manifest.json")).packageVersion).toBe("0.2.0");
});
