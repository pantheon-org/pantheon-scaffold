import { expect, test } from "bun:test";
import { join } from "node:path";

const text = await Bun.file(join(import.meta.dir, "fixtures/consumer-check.yml")).text();
const wf = Bun.YAML.parse(text) as any;

test("the consumer check runs on pull_request only, never pull_request_target", () => {
  expect(Object.keys(wf.on)).toEqual(["pull_request"]);
  expect(text).not.toContain("pull_request_target");
});
test("the consumer check uses no secrets", () => expect(text).not.toMatch(/secrets\./));
test("the token is the built-in one with read access only", () => {
  expect(wf.permissions).toEqual({});
  expect(wf.jobs.check.permissions).toEqual({ contents: "read" });
  expect(wf.jobs.check.env.GITHUB_TOKEN).toBe("${{ github.token }}");
});
test("every action is pinned by a 40 character SHA", () => {
  const uses = text.split("\n").filter((l) => /^\s*-?\s*uses:/.test(l));
  expect(uses.length).toBeGreaterThan(0);
  for (const l of uses) expect(l).toMatch(/@[0-9a-f]{40} # v/);
});
test("checkout does not persist credentials", () => expect(text).toContain("persist-credentials: false"));
test("the tools come from the lock file", () => expect(text).toContain("mise install --locked"));

test("the package cache is kept between runs, so an outage does not block the check", () => {
  expect(text).toContain("path: .pkl-gen/cache");
  expect(text).toMatch(/uses: actions\/cache@[0-9a-f]{40} # v\d+\.\d+\.\d+/);
});
