import { expect, test } from "bun:test";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { evalContractJson } from "../src/pkl";
import type { Schema } from "../src/schema";

const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const contract = parseContract(await evalContractJson(join(import.meta.dir, "fixtures/tooling/pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const file = (path: string) => contract.files.find((f) => f.path === path)!;
const gitBlobSha = (text: string): string => {
  const body = new TextEncoder().encode(text);
  return new Bun.CryptoHasher("sha1").update(`blob ${body.length}\0`).update(body).digest("hex");
};
const pinned = (await Bun.file(join(import.meta.dir, "upstream/tooling-blobs.json")).json()) as Record<string, { blob: string; upstreamMode: string; note?: string }>;

// The one deliberate difference from pantheon-adr's files: .rumdl.toml ends with this table. The pull request template starts with an
// HTML comment and a level 2 heading, so MD041 can never pass for it, and the rendered hook would fail the first commit of every project.
// The same text is appended to pkl/assets/rumdl.toml. Keep it last in the file, because the test below strips it from the end.
const RUMDL_ADDITION =
  "\n# The pull request template starts with an HTML comment and a level 2 heading, so MD041 can never pass for it.\n" +
  "# Every other file is still checked by MD041.\n" +
  "[per-file-ignores]\n" +
  '".github/pull_request_template.md" = ["MD041"]\n';

// The nine scripts the four repos share (baseline SP-0.1): two come from baseFiles, seven from toolingFiles.
const NINE = [
  "agnix-assemble-comment", "agnix-file-issues", "agnix-introduced-errors", "auto-rebase", "cargo-deny",
  "check-commit-types", "check-inline-scripts", "release-automerge", "sync-labels",
].map((n) => `.github/scripts/${n}.sh`);

test("the tooling files are the nine shared scripts, check-toolchain-version.sh, the two configs and the base files", () => {
  expect(contract.files.map((f) => f.path).sort()).toEqual([
    ".claude/settings.json", ".github/scripts/agnix-assemble-comment.sh", ".github/scripts/agnix-file-issues.sh", ".github/scripts/agnix-introduced-errors.sh",
    ".github/scripts/auto-rebase.sh", ".github/scripts/cargo-deny.sh", ".github/scripts/check-commit-types.sh", ".github/scripts/check-inline-scripts.sh",
    ".github/scripts/check-toolchain-version.sh", ".github/scripts/release-automerge.sh", ".github/scripts/sync-labels.sh", ".rumdl.toml", "deny.toml", "rust-toolchain.toml",
  ]);
});

test("all nine shared scripts render executable and every config renders not executable", () => {
  for (const p of NINE) expect(file(p).mode, p).toBe("0755");
  for (const p of [".rumdl.toml", ".claude/settings.json", "deny.toml", "rust-toolchain.toml"]) expect(file(p).mode, p).toBe("0644");
});

test("every file is an owned file with content, and the Phase 3 files use no escape hatch", () => {
  for (const f of contract.files) {
    expect(f.class, f.path).toBe("owned");
    expect(f.content, f.path).toBeDefined();
  }
  expect(contract.escapeHatch.uses).toBe(0);
});

for (const [path, { blob }] of Object.entries(pinned)) {
  const title =
    path === ".rumdl.toml"
      ? `${path} renders as the pinned pantheon-adr file plus the one per-file-ignores table (git blob ${blob.slice(0, 7)} without it)`
      : `${path} renders byte for byte as the pinned pantheon-adr file (git blob ${blob.slice(0, 7)})`;
  test(title, () => {
    const content = file(path).content!;
    if (path !== ".rumdl.toml") {
      expect(gitBlobSha(content)).toBe(blob);
      return;
    }
    expect(content.endsWith(RUMDL_ADDITION)).toBe(true);
    expect(content.split(RUMDL_ADDITION).length - 1).toBe(1);
    expect(gitBlobSha(content.slice(0, -RUMDL_ADDITION.length))).toBe(blob);
  });
}

test("the rendered .rumdl.toml ignores MD041 for the pull request template and nowhere else", () => {
  const toml = Bun.TOML.parse(file(".rumdl.toml").content!) as { global: { disable: string[] }; "per-file-ignores": Record<string, string[]> };
  expect(Object.keys(toml["per-file-ignores"])).toEqual([".github/pull_request_template.md"]);
  expect(toml["per-file-ignores"][".github/pull_request_template.md"]).toEqual(["MD041"]);
  expect(toml.global.disable).not.toContain("MD041");
});

test("the pinned list covers the seven new scripts and both configs, and the nine scripts are all accounted for", () => {
  expect(Object.keys(pinned)).toHaveLength(9);
  const shared = NINE.filter((p) => p in pinned || ["cargo-deny", "check-inline-scripts"].some((n) => p.endsWith(`${n}.sh`)));
  expect(shared).toHaveLength(9);
});

test("the one script that is not executable upstream is rendered executable on purpose, and it is the only one", () => {
  const differs = Object.entries(pinned).filter(([p, { upstreamMode }]) => p.endsWith(".sh") && upstreamMode !== "100755" && file(p).mode === "0755").map(([p]) => p);
  expect(differs).toEqual([".github/scripts/sync-labels.sh"]);
});
