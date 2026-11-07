import { expect, test } from "bun:test";
import { compareSemver, parseSemver } from "../src/semver";

test("parses a plain version, a leading v and a pre-release", () => {
  expect(parseSemver("1.2.3")).toEqual({ major: 1, minor: 2, patch: 3, pre: [] });
  expect(parseSemver("v0.10.0")).toEqual({ major: 0, minor: 10, patch: 0, pre: [] });
  expect(parseSemver("0.1.0-rc.1")).toEqual({ major: 0, minor: 1, patch: 0, pre: ["rc", "1"] });
});

test("rejects text that is not a version", () => {
  for (const bad of ["", "1.2", "1.2.3.4", "a.b.c", "01.2.3", "1.2.3-", "latest"]) expect(parseSemver(bad)).toBeNull();
});

test("compares numerically, not as text", () => {
  expect(compareSemver("0.10.0", "0.9.0")).toBeGreaterThan(0);
  expect(compareSemver("1.0.0", "1.0.0")).toBe(0);
  expect(compareSemver("0.0.1", "0.1.0")).toBeLessThan(0);
});

test("a pre-release is lower than its release and pre-releases follow the semver order", () => {
  expect(compareSemver("0.1.0-rc.1", "0.1.0")).toBeLessThan(0);
  expect(compareSemver("0.1.0-rc.2", "0.1.0-rc.10")).toBeLessThan(0);
  expect(compareSemver("0.1.0-alpha", "0.1.0-alpha.1")).toBeLessThan(0);
  expect(compareSemver("0.1.0-alpha.1", "0.1.0-beta")).toBeLessThan(0);
  expect(compareSemver("0.1.0-1", "0.1.0-a")).toBeLessThan(0);
});

test("comparing an unparsable version throws, never guesses", () => {
  expect(() => compareSemver("nope", "1.0.0")).toThrow("not a version");
});
