import { afterEach, expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { allowedResources, evalContractJson, looksLikeFetchFailure } from "../src/pkl";
import { cleanupRepos, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const contractJson = JSON.stringify({ schemaVersion: 1, files: [], pins: [], escapeHatch: { uses: 0, constructs: 0 } });

/** A fake pkl on the PATH: fails with `stderr` for the first `failTimes` calls, then prints the contract. Counts its calls. */
function fakePkl(failTimes: number, stderr: string) {
  const dir = tempRepo();
  mkdirSync(join(dir, "bin"));
  const counter = join(dir, "count");
  writeFileSync(join(dir, "contract.json"), contractJson);
  writeFileSync(
    join(dir, "bin/pkl"),
    `#!/bin/sh\nn=$(cat "${counter}" 2>/dev/null || echo 0); n=$((n+1)); echo $n > "${counter}"\nif [ "$n" -le ${failTimes} ]; then echo "${stderr.replaceAll("`", "\\`")}" >&2; exit 1; fi\ncat "${join(dir, "contract.json")}"\n`,
  );
  chmodSync(join(dir, "bin/pkl"), 0o755);
  return { env: { PATH: `${join(dir, "bin")}:${process.env.PATH}` }, calls: () => Number(readFileSync(counter, "utf8")) };
}

test("a fetch failure is retried and the third attempt succeeds", async () => {
  const f = fakePkl(2, "Exception when making request `GET https://github.com/x`");
  expect(await evalContractJson("x.pkl", ".", { env: f.env, attempts: 3, delayMs: 1 })).toMatchObject({ schemaVersion: 1 });
  expect(f.calls()).toBe(3);
});

test("a fetch failure that never clears fails closed after all attempts", async () => {
  const f = fakePkl(99, "Cannot download package `package://x`");
  await expect(evalContractJson("x.pkl", ".", { env: f.env, attempts: 3, delayMs: 1 })).rejects.toThrow(/after 3 attempts, failing closed/);
  expect(f.calls()).toBe(3);
});

test("a 404 from a just-created release is retried", async () => {
  const f = fakePkl(1, "Received unexpected status code `404` when making request");
  await evalContractJson("x.pkl", ".", { env: f.env, attempts: 2, delayMs: 1 });
  expect(f.calls()).toBe(2);
});

test("a pkl error that is not a fetch problem is not retried", async () => {
  const f = fakePkl(99, "Type constraint violated");
  await expect(evalContractJson("x.pkl", ".", { env: f.env, attempts: 5, delayMs: 1 })).rejects.toThrow(/pkl eval failed \(exit 1\):/);
  expect(f.calls()).toBe(1);
});

test("one attempt means no retry", async () => {
  const f = fakePkl(99, "Exception when making request");
  await expect(evalContractJson("x.pkl", ".", { env: f.env, attempts: 1, delayMs: 1 })).rejects.toThrow(/failing closed/);
  expect(f.calls()).toBe(1);
});

test("fetch failure detection", () => {
  expect(looksLikeFetchFailure("Exception when making request")).toBe(true);
  expect(looksLikeFetchFailure("Received unexpected status code `503`")).toBe(true);
  expect(looksLikeFetchFailure("Cannot find property `x`")).toBe(false);
});

test("local package folders are allowed as anchored file: patterns only", () => {
  expect(allowedResources({ allowFilesUnder: ["/a/b.c"] })).toBe("package:,projectpackage:,https:,^file:///a/b\\.c/");
  expect(allowedResources()).toBe("package:,projectpackage:,https:");
});
