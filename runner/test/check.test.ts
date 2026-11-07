import { afterEach, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { apply } from "../src/apply";
import { check } from "../src/check";
import { cleanupRepos, contractOf, external, get, owned, put, seed, tempRepo } from "./helpers";

afterEach(cleanupRepos);

test("no drift after a render", async () => {
  const root = tempRepo();
  const c = contractOf(owned("a.txt", "one\n"), seed("s.md", "# s\n"), external("m.lock"));
  await apply(c, root);
  expect(check(c, root)).toEqual({ drift: [], checked: 1 });
});

test("a hand edit is drift with a diff", async () => {
  const root = tempRepo();
  const c = contractOf(owned("a.txt", "one\n"));
  await apply(c, root);
  put(root, "a.txt", "edited\n");
  const r = check(c, root);
  expect(r.drift).toEqual([{ path: "a.txt", reason: "different", diff: "    line 1 on disk:  edited\n    line 1 rendered: one" }]);
});

test("a stale file (the render changed) is drift", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "one\n")), root);
  expect(check(contractOf(owned("a.txt", "two\n")), root).drift).toHaveLength(1);
});

test("a missing file is drift", () => {
  expect(check(contractOf(owned("a.txt", "one\n")), tempRepo()).drift).toEqual([{ path: "a.txt", reason: "missing", diff: "" }]);
});

test("seed and external files are skipped", () => {
  expect(check(contractOf(seed("s.md", "x\n"), external("m.lock")), tempRepo())).toEqual({ drift: [], checked: 0 });
});

test("check writes nothing, not even the tool folder", () => {
  const root = tempRepo();
  check(contractOf(owned("a.txt", "one\n")), root);
  expect(existsSync(join(root, ".pkl-gen"))).toBe(false);
  expect(existsSync(join(root, "a.txt"))).toBe(false);
});

test("check uses the text with version comments, so a rendered repo is clean", async () => {
  const root = tempRepo();
  const sha = "3d3c42e5aac5ba805825da76410c181273ba90b1";
  const c = { ...contractOf(owned(".github/workflows/x.yml", `steps:\n  - uses: actions/checkout@${sha}\n`)), pins: [{ action: "actions/checkout", sha, version: "v7.0.1" }] };
  await apply(c, root);
  expect(get(root, ".github/workflows/x.yml")).toContain("# v7.0.1");
  expect(check(c, root).drift).toEqual([]);
});
