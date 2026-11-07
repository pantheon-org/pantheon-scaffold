import { expect, test } from "bun:test";
import type { Pin } from "../src/contract";
import { appendVersions, findMissingComments, findUnpinned } from "../src/versions";

const SHA = "3d3c42e5aac5ba805825da76410c181273ba90b1";
const pins: Pin[] = [{ action: "actions/checkout", sha: SHA, version: "v7.0.1" }, { action: "owner/repo/.github/workflows/x.yml", sha: SHA, version: "v1.0.0" }];
const wf = (body: string) => `jobs:\n  a:\n    steps:\n${body}\n`;

test("appends the comment to a pinned line", () => {
  const out = appendVersions(wf(`      - uses: actions/checkout@${SHA}`), pins);
  expect(out).toContain(`- uses: actions/checkout@${SHA} # v7.0.1`);
});
test("is idempotent", () => {
  const once = appendVersions(wf(`      - uses: actions/checkout@${SHA}`), pins);
  expect(appendVersions(once, pins)).toBe(once);
});
test("keeps an existing matching comment byte for byte", () => {
  const t = wf(`      - uses: actions/checkout@${SHA} # v7.0.1`);
  expect(appendVersions(t, pins)).toBe(t);
});
test("refuses to overwrite a differing comment", () => {
  expect(() => appendVersions(wf(`      - uses: actions/checkout@${SHA} # v6`), pins)).toThrow(/differs/);
});
test("fails on a pin with no known version", () => {
  expect(() => appendVersions(wf(`      - uses: other/thing@${SHA}`), pins)).toThrow(/no version known/);
});
test("handles quoted values and keeps the quotes", () => {
  const out = appendVersions(wf(`      - uses: "actions/checkout@${SHA}"`), pins);
  expect(out).toContain(`"actions/checkout@${SHA}" # v7.0.1`);
});
test("handles a reusable workflow path", () => {
  const out = appendVersions(wf(`    uses: owner/repo/.github/workflows/x.yml@${SHA}`), pins);
  expect(out).toContain("# v1.0.0");
});
test("ignores local actions and docker references", () => {
  const t = wf(`      - uses: ./.github/actions/local\n      - uses: docker://alpine@sha256:${"a".repeat(64)}`);
  expect(appendVersions(t, pins)).toBe(t);
});
test("ignores a uses line inside a run block scalar", () => {
  const t = wf(`      - run: |\n          echo hello\n          uses: actions/checkout@${SHA}\n      - uses: actions/checkout@${SHA}`);
  const out = appendVersions(t, pins);
  expect(out).toContain(`          uses: actions/checkout@${SHA}\n`);
  expect(out.split("# v7.0.1").length - 1).toBe(1);
});
test("ignores uses text inside a quoted string and a comment", () => {
  const t = wf(`      - name: "uses: actions/checkout@${SHA}"\n      # uses: actions/checkout@${SHA}`);
  expect(appendVersions(t, pins)).toBe(t);
});
test("the checker flags a pinned line with no comment and passes one with a comment", () => {
  expect(findMissingComments(wf(`      - uses: actions/checkout@${SHA}`))).toHaveLength(1);
  expect(findMissingComments(wf(`      - uses: actions/checkout@${SHA} # v7.0.1`))).toHaveLength(0);
});
test("the checker reports an unpinned tag", () => {
  expect(findUnpinned(wf("      - uses: actions/checkout@v4"))).toHaveLength(1);
  expect(findUnpinned(wf(`      - uses: actions/checkout@${SHA}`))).toHaveLength(0);
});
