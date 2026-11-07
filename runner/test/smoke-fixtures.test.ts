import { expect, test } from "bun:test";
import { join } from "node:path";
import { evalContractJson } from "../src/pkl";

// The fixtures the compiled-binary smoke suite (runner/scripts/smoke.ts) renders. Kept honest here, where bun test can see them.
const root = join(import.meta.dir, "../..");
const paths = async (name: string) =>
  ((await evalContractJson(join(import.meta.dir, "fixtures", name, "pantheon.pkl"), root)) as { files: { path: string }[] }).files.map((f) => f.path);

test("smoke-orphan-a renders two files and smoke-orphan-b drops the second", async () => {
  expect(await paths("smoke-orphan-a")).toEqual(["a.txt", "b.txt"]);
  expect(await paths("smoke-orphan-b")).toEqual(["a.txt"]);
});

test("smoke-case-collision renders two paths that differ only by case", async () => {
  expect(await paths("smoke-case-collision")).toEqual(["README.md", "readme.md"]);
});

test("smoke-case-a and smoke-case-b render the same file under names that differ only by case", async () => {
  expect(await paths("smoke-case-a")).toEqual(["readme.md"]);
  expect(await paths("smoke-case-b")).toEqual(["README.md"]);
});
