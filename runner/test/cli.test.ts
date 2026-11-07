import { expect, test } from "bun:test";
import { join } from "node:path";

const cli = join(import.meta.dir, "../src/cli.ts");
const root = join(import.meta.dir, "../..");
const run = async (...args: string[]) => {
  const p = Bun.spawn(["bun", cli, ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};

test("--version prints the version", async () => expect(await run("--version")).toMatchObject({ out: "0.0.0", code: 0 }));
test("contract summarises a valid fixture", async () => {
  const r = await run("contract", "runner/test/fixtures/minimal/pantheon.pkl");
  expect(r.code).toBe(0);
  expect(r.out).toBe("contract ok: 4 files, 1 pins, escape hatch 0 of 4 constructs");
});
test("contract fails with exit 1 on a refused read", async () => {
  const r = await run("contract", "runner/test/fixtures/read-file/pantheon.pkl");
  expect(r.code).toBe(1);
  expect(r.err).toContain("Refusing to read resource");
});
test("an unknown command prints usage with exit 2", async () => expect((await run("nope")).code).toBe(2));

import { afterEach } from "bun:test";
import { statSync } from "node:fs";
import { cleanupRepos, get, has, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const slice = "runner/test/fixtures/slice/pantheon.pkl";
const pkgDir = join(root, "pkl");
const render = (repo: string, ...extra: string[]) => run("render", slice, "--root", repo, "--allow-local-package", pkgDir, ...extra);

test("render writes the slice into an empty repo with modes and a manifest", async () => {
  const repo = tempRepo();
  const r = await render(repo);
  expect(r.code).toBe(0);
  expect(r.out).toContain("written 8 file(s)");
  expect(has(repo, ".github/workflows/cargo-deny.yml")).toBe(true);
  expect((statSync(join(repo, ".github/scripts/cargo-deny.sh")).mode & 0o777).toString(8)).toBe("755");
  expect(has(repo, ".pkl-gen/manifest.json")).toBe(true);
});

test("a second render writes nothing", async () => {
  const repo = tempRepo();
  await render(repo);
  const r = await render(repo);
  expect(r.code).toBe(0);
  expect(r.out).toContain("written 0 file(s)");
  expect(r.out).not.toContain("create");
});

test("a hand edit is refused with exit 3 and the file is kept", async () => {
  const repo = tempRepo();
  await render(repo);
  put(repo, "deny.toml", "# mine\n");
  const r = await render(repo);
  expect(r.code).toBe(3);
  expect(r.err).toContain("deny.toml: was edited by hand");
  expect(get(repo, "deny.toml")).toBe("# mine\n");
  expect((await render(repo, "--force")).code).toBe(0);
  expect(get(repo, "deny.toml")).not.toBe("# mine\n");
});

test("--dry-run lists the plan and writes nothing", async () => {
  const repo = tempRepo();
  const r = await render(repo, "--dry-run");
  expect(r.code).toBe(0);
  expect(r.out).toContain("create    deny.toml");
  expect(r.out).toContain("dry run: would write 8 file(s)");
  expect(has(repo, "deny.toml")).toBe(false);
});

test("a local package cannot be read without --allow-local-package", async () => {
  const r = await run("render", slice, "--root", tempRepo());
  expect(r.code).toBe(1);
  expect(r.err).toContain("Refusing to read resource");
});

test("an unknown option is a usage error", async () => expect((await run("render", slice, "--nope")).code).toBe(2));
test("an option without its value is a usage error", async () => expect((await run("render", slice, "--root")).code).toBe(2));

import { compareWorkflows } from "../src/equality";

test("the rendered workflows carry version comments and still equal the pinned upstream copies", async () => {
  const repo = tempRepo();
  await render(repo);
  for (const name of ["cargo-deny", "inline-scripts", "rust-ci"]) {
    const text = get(repo, `.github/workflows/${name}.yml`);
    expect(text).toContain("actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1");
    const upstream = await Bun.file(join(root, `runner/test/upstream/workflows/${name}.yml`)).text();
    expect(compareWorkflows(upstream, text)).toEqual({ equal: true, differences: [], duplicateKeys: [] });
  }
});
