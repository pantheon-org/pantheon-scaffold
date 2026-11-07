import { afterEach, expect, test } from "bun:test";
import { mkdirSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import { findCaseCollisions, PathError, resolveInside } from "../src/paths";
import { cleanupRepos, tempRepo } from "./helpers";

afterEach(cleanupRepos);

test("a nested relative path resolves inside the root", () => {
  const root = tempRepo();
  expect(resolveInside(root, ".github/workflows/ci.yml")).toEndWith("/.github/workflows/ci.yml");
});
test("a leading ./ is normalised", () => expect(resolveInside(tempRepo(), "./a.txt")).toEndWith("/a.txt"));
test("rejects an empty path", () => expect(() => resolveInside(tempRepo(), "")).toThrow(PathError));
test("rejects an absolute path", () => expect(() => resolveInside(tempRepo(), "/etc/passwd")).toThrow(/absolute/));
test("rejects a home path", () => expect(() => resolveInside(tempRepo(), "~/x")).toThrow(/absolute/));
test("rejects a parent segment", () => expect(() => resolveInside(tempRepo(), "../x")).toThrow(/escapes/));
test("rejects a parent segment hidden in the middle", () => expect(() => resolveInside(tempRepo(), "a/../../x")).toThrow(/escapes/));
test("rejects backslashes", () => expect(() => resolveInside(tempRepo(), "a\\b")).toThrow(/backslashes/));
test("rejects a null byte", () => expect(() => resolveInside(tempRepo(), "a\0b")).toThrow(PathError));
test("rejects a path that resolves outside through a symlinked directory", () => {
  const root = tempRepo();
  const outside = tempRepo();
  symlinkSync(outside, join(root, "link"));
  expect(() => resolveInside(root, "link/file.txt")).toThrow(/symlink/);
});
test("rejects a symlink file that points outside", () => {
  const root = tempRepo();
  const outside = tempRepo();
  symlinkSync(join(outside, "x"), join(root, "f"));
  expect(() => resolveInside(root, "f")).toThrow(/symlink/);
});
test("allows a symlinked directory that stays inside the root", () => {
  const root = tempRepo();
  mkdirSync(join(root, "real"));
  symlinkSync(join(root, "real"), join(root, "alias"));
  expect(() => resolveInside(root, "alias/f.txt")).not.toThrow();
});
test("finds paths that differ only by case", () => expect(findCaseCollisions(["a/B.txt", "a/b.txt", "c"])).toEqual(["a/B.txt and a/b.txt differ only by case"]));
test("identical paths are not a case collision", () => expect(findCaseCollisions(["a", "a"])).toEqual([]));
