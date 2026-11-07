import { afterEach, expect, test } from "bun:test";
import { chmodSync, readdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { apply, shortDiff } from "../src/apply";
import { readManifest, sha256 } from "../src/manifest";
import { cleanupRepos, contractOf, external, get, has, modeOf, owned, put, seed, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const kinds = (r: Awaited<ReturnType<typeof apply>>) => r.actions.map((a) => `${a.kind}:${a.path}`);

test("a fresh run creates owned files with the contract mode and writes the manifest last", async () => {
  const root = tempRepo();
  const r = await apply(contractOf(owned("a.txt", "one\n"), owned("s/run.sh", "#!/bin/sh\n", "0755")), root);
  expect(kinds(r)).toEqual(["create:a.txt", "create:s/run.sh"]);
  expect(get(root, "a.txt")).toBe("one\n");
  expect(modeOf(root, "s/run.sh")).toBe("0755");
  expect(modeOf(root, "a.txt")).toBe("0644");
  expect((await readManifest(root)).files["a.txt"]).toEqual({ class: "owned", hash: sha256("one\n") });
});

test("a second run changes nothing", async () => {
  const root = tempRepo();
  const c = contractOf(owned("a.txt", "one\n"));
  await apply(c, root);
  const manifestBefore = get(root, ".pkl-gen/manifest.json");
  const r = await apply(c, root);
  expect(kinds(r)).toEqual(["unchanged:a.txt"]);
  expect(r.written).toBe(0);
  expect(get(root, ".pkl-gen/manifest.json")).toBe(manifestBefore);
});

test("no stage files are left behind", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "x\n")), root);
  expect(has(root, ".pkl-gen/stage")).toBe(false);
  expect(readdirSync(join(root, ".pkl-gen"))).toEqual(["manifest.json"]);
});

test("an unmodified owned file is updated when the render changes", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "one\n")), root);
  const r = await apply(contractOf(owned("a.txt", "two\n")), root);
  expect(kinds(r)).toEqual(["update:a.txt"]);
  expect(get(root, "a.txt")).toBe("two\n");
});

test("a hand-edited owned file is refused and nothing else is written", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "one\n"), owned("b.txt", "b1\n")), root);
  put(root, "a.txt", "edited\n");
  const r = await apply(contractOf(owned("a.txt", "one\n"), owned("b.txt", "b2\n")), root);
  expect(r.refusals).toHaveLength(1);
  expect(r.refusals[0]).toContain("a.txt: was edited by hand");
  expect(r.refusals[0]).toContain("line 1 on disk:  edited");
  expect(r.written).toBe(0);
  expect(get(root, "a.txt")).toBe("edited\n");
  expect(get(root, "b.txt")).toBe("b1\n");
});

test("--force overwrites a hand-edited owned file", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "one\n")), root);
  put(root, "a.txt", "edited\n");
  const r = await apply(contractOf(owned("a.txt", "one\n")), root, { force: true });
  expect(r.refusals).toEqual([]);
  expect(get(root, "a.txt")).toBe("one\n");
});

test("first adoption: an identical existing file is adopted, not rewritten", async () => {
  const root = tempRepo();
  put(root, "a.txt", "same\n");
  const r = await apply(contractOf(owned("a.txt", "same\n")), root);
  expect(kinds(r)).toEqual(["adopt:a.txt"]);
  expect(r.written).toBe(0);
  expect((await readManifest(root)).files["a.txt"].hash).toBe(sha256("same\n"));
});

test("first adoption: a differing existing file is refused with a diff", async () => {
  const root = tempRepo();
  put(root, "a.txt", "mine\n");
  const r = await apply(contractOf(owned("a.txt", "theirs\n")), root);
  expect(r.refusals[0]).toContain("will not adopt");
  expect(r.refusals[0]).toContain("line 1 on disk:  mine");
  expect(r.refusals[0]).toContain("line 1 rendered: theirs");
  expect(get(root, "a.txt")).toBe("mine\n");
  expect(has(root, ".pkl-gen/manifest.json")).toBe(false);
});

test("first adoption: --force overwrites a differing existing file", async () => {
  const root = tempRepo();
  put(root, "a.txt", "mine\n");
  await apply(contractOf(owned("a.txt", "theirs\n")), root, { force: true });
  expect(get(root, "a.txt")).toBe("theirs\n");
});

test("seed: written once, then left alone even when the render changes", async () => {
  const root = tempRepo();
  await apply(contractOf(seed("README.md", "# first\n")), root);
  put(root, "README.md", "# mine\n");
  const r = await apply(contractOf(seed("README.md", "# second\n")), root);
  expect(kinds(r)).toEqual(["seed-kept:README.md"]);
  expect(get(root, "README.md")).toBe("# mine\n");
});

test("seed to owned is refused for an edited seed", async () => {
  const root = tempRepo();
  await apply(contractOf(seed("README.md", "# first\n")), root);
  put(root, "README.md", "# mine\n");
  const r = await apply(contractOf(owned("README.md", "# first\n")), root);
  expect(r.refusals[0]).toContain("was seeded and has been edited");
  expect(get(root, "README.md")).toBe("# mine\n");
});

test("seed to owned is allowed for an untouched seed", async () => {
  const root = tempRepo();
  await apply(contractOf(seed("README.md", "# first\n")), root);
  const r = await apply(contractOf(owned("README.md", "# second\n")), root);
  expect(r.refusals).toEqual([]);
  expect(get(root, "README.md")).toBe("# second\n");
});

test("owned to seed leaves the file and records it as a seed", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "one\n")), root);
  await apply(contractOf(seed("a.txt", "one\n")), root);
  expect(get(root, "a.txt")).toBe("one\n");
  expect((await readManifest(root)).files["a.txt"].class).toBe("seed");
});

test("an external file is listed, never written, hashed or touched", async () => {
  const root = tempRepo();
  put(root, "mise.lock", "lock\n");
  const r = await apply(contractOf(external("mise.lock")), root);
  expect(kinds(r)).toEqual(["external:mise.lock"]);
  expect(get(root, "mise.lock")).toBe("lock\n");
  expect((await readManifest(root)).files["mise.lock"]).toEqual({ class: "external" });
});

test("a file dropped from the contract is reported as an orphan and not deleted in 2a", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "one\n"), owned("b.txt", "two\n")), root);
  const r = await apply(contractOf(owned("a.txt", "one\n")), root);
  expect(kinds(r)).toContain("orphan:b.txt");
  expect(get(root, "b.txt")).toBe("two\n");
});

test("--dry-run plans and writes nothing", async () => {
  const root = tempRepo();
  const r = await apply(contractOf(owned("a.txt", "one\n")), root, { dryRun: true });
  expect(kinds(r)).toEqual(["create:a.txt"]);
  expect(r.dryRun).toBe(true);
  expect(has(root, "a.txt")).toBe(false);
  expect(has(root, ".pkl-gen")).toBe(false);
});

test("a path outside the repository stops the run before anything is written", async () => {
  const root = tempRepo();
  await expect(apply(contractOf(owned("a.txt", "x\n"), owned("../evil.txt", "x\n")), root)).rejects.toThrow(/escapes/);
  expect(has(root, "a.txt")).toBe(false);
});

test("paths that differ only by case stop the run", async () => {
  await expect(apply(contractOf(owned("A.txt", "x\n"), owned("a.txt", "y\n")), tempRepo())).rejects.toThrow(/differ only by case/);
});

test("a write through a symlink that leaves the repository is refused", async () => {
  const root = tempRepo();
  const outside = tempRepo();
  symlinkSync(outside, join(root, "link"));
  await expect(apply(contractOf(owned("link/x.txt", "x\n")), root)).rejects.toThrow(/symlink/);
  expect(has(outside, "x.txt")).toBe(false);
});

test("--repair rebuilds the manifest from files equal to the render and never adopts a differing file", async () => {
  const root = tempRepo();
  put(root, "same.txt", "s\n");
  put(root, "diff.txt", "mine\n");
  const r = await apply(contractOf(owned("same.txt", "s\n"), owned("diff.txt", "theirs\n"), owned("gone.txt", "g\n"), external("m.lock")), root, { repair: true });
  const m = await readManifest(root);
  expect(Object.keys(m.files)).toEqual(["m.lock", "same.txt"]);
  expect(r.refusals.map((x) => x.split(":")[0]).sort()).toEqual(["diff.txt", "gone.txt"]);
  expect(get(root, "diff.txt")).toBe("mine\n");
});

test("--repair after a broken manifest lets a normal run continue", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "one\n")), root);
  writeFileSync(join(root, ".pkl-gen/manifest.json"), "<<<<<<< x\n");
  await expect(apply(contractOf(owned("a.txt", "one\n")), root)).rejects.toThrow(/--repair/);
  await apply(contractOf(owned("a.txt", "one\n")), root, { repair: true });
  expect(kinds(await apply(contractOf(owned("a.txt", "one\n")), root))).toEqual(["unchanged:a.txt"]);
});

test("an unreadable existing file does not crash planning of the others", async () => {
  const root = tempRepo();
  put(root, "ok.txt", "x\n");
  chmodSync(join(root, "ok.txt"), 0o644);
  expect(kinds(await apply(contractOf(owned("ok.txt", "x\n")), root))).toEqual(["adopt:ok.txt"]);
});

test("shortDiff shows the first differing lines only", () => {
  expect(shortDiff("a\nb\nc\n", "a\nB\nc\n")).toBe("    line 2 on disk:  b\n    line 2 rendered: B");
});

test("pinned uses lines get their version comment before hashing, and a second run is a no-op", async () => {
  const root = tempRepo();
  const sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
  const c = { ...contractOf(owned(".github/workflows/x.yml", `jobs:\n  a:\n    steps:\n      - uses: actions/checkout@${sha}\n`)), pins: [{ action: "actions/checkout", sha, version: "v7.0.1" }] };
  await apply(c, root);
  expect(get(root, ".github/workflows/x.yml")).toContain(`actions/checkout@${sha} # v7.0.1`);
  expect((await readManifest(root)).files[".github/workflows/x.yml"].hash).toBe(sha256(get(root, ".github/workflows/x.yml")));
  expect(kinds(await apply(c, root))).toEqual(["unchanged:.github/workflows/x.yml"]);
});

test("a pinned uses line with no known version stops the run", async () => {
  const sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
  await expect(apply(contractOf(owned(".github/workflows/x.yml", `steps:\n  - uses: other/thing@${sha}\n`)), tempRepo())).rejects.toThrow(/x.yml: line 2: no version known/);
});

test("version comments are not added to files outside workflows", async () => {
  const root = tempRepo();
  const sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
  await apply(contractOf(owned("notes.md", `  - uses: actions/checkout@${sha}\n`)), root);
  expect(get(root, "notes.md")).not.toContain("#");
});
