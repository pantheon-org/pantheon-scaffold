import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { SUPPORTED_PLATFORMS, checkRunnerLock } from "../src/lockcheck";

// Both locks are real `mise lock` output for the published v0.1.1 release.
const fixture = (name: string) => readFileSync(join(import.meta.dir, "fixtures", name), "utf8");
const all = fixture("mise-lock-all-platforms.lock");
const three = fixture("mise-lock-three-platforms.lock");

test("the supported platforms are the three the runner is built for", () => {
  expect(SUPPORTED_PLATFORMS).toEqual({ "linux-x64": "pantheon-scaffold-linux-x64", "linux-arm64": "pantheon-scaffold-linux-arm64", "macos-arm64": "pantheon-scaffold-darwin-arm64" });
});

test("a lock limited to the three supported platforms has no problems", () => {
  expect(checkRunnerLock(three)).toEqual([]);
});

test("a lock for every platform names the package zip for platforms with no runner, and the glibc build for musl", () => {
  const problems = checkRunnerLock(all);
  expect(problems).toHaveLength(4);
  expect(problems.filter((p) => p.includes("pantheon-scaffold@0.1.1.zip")).map((p) => p.split(":")[0]).sort()).toEqual(["macos-x64", "windows-x64"]);
  expect(problems.filter((p) => p.startsWith("linux-")).map((p) => p.split(":")[0]).sort()).toEqual(["linux-arm64-musl", "linux-x64-musl"]);
  for (const p of problems) expect(p).toContain("not a supported platform");
});

test("a supported platform whose entry names the wrong asset is a problem", () => {
  const bad = three.replace("pantheon-scaffold-linux-x64", "pantheon-scaffold%400.1.1.zip");
  const problems = checkRunnerLock(bad);
  expect(problems).toHaveLength(1);
  expect(problems[0]).toContain("linux-x64: mise chose pantheon-scaffold@0.1.1.zip, expected pantheon-scaffold-linux-x64");
});

test("a lock with no entry for the runner has nothing to check", () => {
  expect(checkRunnerLock("# lock for 0.1.0\n")).toEqual([]);
  expect(checkRunnerLock(`[tools."github:other/tool"."platforms.windows-x64"]\nurl = "https://example.test/tool.zip"\n`)).toEqual([]);
});

test("entries of other tools are never mistaken for the runner's", () => {
  const mixed = `${three}\n[tools."github:other/tool"."platforms.windows-x64"]\nurl = "https://example.test/tool.zip"\n`;
  expect(checkRunnerLock(mixed)).toEqual([]);
});

test("an entry with no url is reported, not skipped", () => {
  const noUrl = `[tools."github:pantheon-org/pantheon-scaffold"."platforms.linux-x64"]\nchecksum = "sha256:${"a".repeat(64)}"\n`;
  expect(checkRunnerLock(noUrl).join()).toContain("linux-x64: no url");
});
