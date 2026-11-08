import { expect, test } from "bun:test";
import { join } from "node:path";
import { evalContractJson } from "../src/pkl";

// The fixtures the compiled-binary smoke suite (runner/scripts/smoke.ts) renders. Kept honest here, where bun test can see them.
const root = join(import.meta.dir, "../..");
const paths = async (name: string) =>
  ((await evalContractJson(join(import.meta.dir, "fixtures", name, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] })) as { files: { path: string }[] }).files.map((f) => f.path);

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

test("the phase 3 fixtures render the same 19 files, with the default and with all features", async () => {
  const expected = [
    ".claude/settings.json", ".github/scripts/agnix-assemble-comment.sh", ".github/scripts/agnix-file-issues.sh", ".github/scripts/agnix-introduced-errors.sh",
    ".github/scripts/auto-rebase.sh", ".github/scripts/cargo-deny.sh", ".github/scripts/check-commit-types.sh", ".github/scripts/check-inline-scripts.sh",
    ".github/scripts/check-toolchain-version.sh", ".github/scripts/release-automerge.sh", ".github/scripts/sync-labels.sh", ".mise/conf.d/repo.toml", ".rumdl.toml",
    "deny.toml", "hk.pkl", "hk.shared.pkl", "mise.lock", "mise.toml", "rust-toolchain.toml",
  ];
  expect((await paths("phase3")).sort()).toEqual(expected);
  expect((await paths("phase3-all")).sort()).toEqual(expected);
});
