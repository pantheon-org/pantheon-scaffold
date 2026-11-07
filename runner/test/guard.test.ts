import { afterEach, expect, test } from "bun:test";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deleteGuard } from "../src/guard";
import { cleanupRepos, git, gitRepo, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

test("a clean tracked tree with a tracked file passes", () => {
  const root = gitRepo({ "a.txt": "a\n" });
  expect(deleteGuard(root, ["a.txt"])).toEqual([]);
});

test("a modified tracked file makes the tree not clean", () => {
  const root = gitRepo({ "a.txt": "a\n" });
  put(root, "a.txt", "changed\n");
  expect(deleteGuard(root, ["a.txt"]).join("\n")).toContain("not clean");
});

test("a staged change makes the tree not clean", () => {
  const root = gitRepo({ "a.txt": "a\n", "b.txt": "b\n" });
  put(root, "b.txt", "changed\n");
  git(root, "add", "b.txt");
  expect(deleteGuard(root, ["a.txt"]).join("\n")).toContain("not clean");
});

test("an untracked file is never deleted, and an untracked file elsewhere does not block the tracked one", () => {
  const root = gitRepo({ "a.txt": "a\n" });
  put(root, "new.txt", "u\n");
  expect(deleteGuard(root, ["new.txt"]).join("\n")).toContain("not tracked by git");
  expect(deleteGuard(root, ["a.txt"])).toEqual([]);
});

test("a directory that is not a git repository refuses", () => {
  const root = tempRepo();
  put(root, "a.txt", "a\n");
  expect(deleteGuard(root, ["a.txt"]).join("\n")).toContain("not a git repository");
});

test("a sparse checkout counts as not clean", () => {
  const root = gitRepo({ "a.txt": "a\n" });
  git(root, "config", "core.sparseCheckout", "true");
  expect(deleteGuard(root, ["a.txt"]).join("\n")).toContain("sparse or partial");
});

test("a partial clone counts as not clean", () => {
  const root = gitRepo({ "a.txt": "a\n" });
  git(root, "config", "extensions.partialClone", "origin");
  expect(deleteGuard(root, ["a.txt"]).join("\n")).toContain("sparse or partial");
});

test("a shallow checkout is not refused (decision 47)", () => {
  const source = gitRepo({ "a.txt": "a\n" });
  put(source, "a.txt", "two\n");
  git(source, "commit", "-qam", "second");
  const clone = join(mkdtempSync(join(tmpdir(), "scaffold-test-")), "clone");
  git(source, "clone", "-q", "--depth", "1", `file://${source}`, clone);
  expect(git(clone, "rev-parse", "--is-shallow-repository").trim()).toBe("true");
  expect(deleteGuard(clone, ["a.txt"])).toEqual([]);
});

test("GIT_DIR in the environment cannot redirect the guard to another repository", () => {
  const other = gitRepo({ "x.txt": "x\n" });
  const root = gitRepo({ "a.txt": "a\n" });
  const before = { dir: process.env.GIT_DIR, tree: process.env.GIT_WORK_TREE };
  process.env.GIT_DIR = join(other, ".git");
  process.env.GIT_WORK_TREE = other;
  try {
    expect(deleteGuard(root, ["a.txt"])).toEqual([]);
  } finally {
    if (before.dir === undefined) delete process.env.GIT_DIR;
    else process.env.GIT_DIR = before.dir;
    if (before.tree === undefined) delete process.env.GIT_WORK_TREE;
    else process.env.GIT_WORK_TREE = before.tree;
  }
});

test("a tracked file in a subfolder of the root is found", () => {
  const root = gitRepo({ "dir/a.txt": "a\n" });
  expect(deleteGuard(root, ["dir/a.txt"])).toEqual([]);
});
