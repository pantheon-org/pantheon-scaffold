import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// The configuration file is scaffold.pkl. These are the only tracked files that may still say pantheon.pkl, and why.
const ALLOWED: Record<string, string> = {
  "crates/scaffold-core/src/config_name.rs": "the one error and its unit tests",
  "features/renamed-config.feature": "the scenarios for that error",
  "runner/test/config-name.test.ts": "this test",
  "docs/adopting.md": "the upgrade note and the note about 0.1.10, which describe the old name",
  "docs/runner-upgrade.md": "what was observed on the real upgrade of 0.1.7 to 0.1.8",
  "docs/workflow-layer.md": "the 0.1.4 header note, a dated fact",
  "docs/ts-runner-test-mapping.md": "the title of a deleted TypeScript test",
  "docs/refocus-2026-11-10.md": "a dated record",
  "docs/presets-design.md": "a dated design record",
  "crates/scaffold-core/tests/data/yaml-corpus.json": "a frozen snapshot of old workflows, text and parsed",
  "runner/scripts/builtins-baseline/README.md": "the account of the one baseline refresh for the rename",
};

const root = join(import.meta.dir, "../..");
const tracked = (): string[] => {
  const r = Bun.spawnSync(["git", "ls-files", "-z"], { cwd: root, stdout: "pipe", stderr: "pipe" });
  if (r.exitCode !== 0) throw new Error(`git ls-files failed: ${r.stderr.toString()}`);
  return r.stdout.toString().split("\0").filter((f) => f !== "" && !f.startsWith("archive/"));
};

test("no tracked file is named pantheon.pkl", () => {
  expect(tracked().filter((f) => f === "pantheon.pkl" || f.endsWith("/pantheon.pkl"))).toEqual([]);
});

test("the old config name appears only in the files that are allowed to say it", () => {
  const found = tracked().filter((f) => readFileSync(join(root, f), "utf8").includes("pantheon.pkl"));
  expect(found.filter((f) => !(f in ALLOWED))).toEqual([]);
});

test("every allowed file still says it, so the list does not outlive its reasons", () => {
  const stale = Object.keys(ALLOWED).filter((f) => !readFileSync(join(root, f), "utf8").includes("pantheon.pkl"));
  expect(stale).toEqual([]);
});
