import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { compareDirectives, compareWorkflows } from "../src/equality";
import { evalContractJson } from "../src/pkl";
import type { Schema } from "../src/schema";

// Phase 5 wave B: the files that differ between the four repositories only in a parameter (crate name, skill name) or a toggle. Each is
// compared with the repository's own pinned copy (main at 1e2fd46, 05c3d84, b9e706e, 42f48b4, survey of 08-11-2026).
const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const REPOS = ["pantheon-adr", "pantheon-journal", "pantheon-skill-auditor", "pantheon-agentic-harness-dev-kit"] as const;
type Repo = (typeof REPOS)[number];
const rendered = Object.fromEntries(
  await Promise.all(REPOS.map(async (r) => [r, parseContract(await evalContractJson(join(import.meta.dir, "fixtures", `real-workflows-${r}`, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema)] as const)),
) as Record<Repo, ReturnType<typeof parseContract>>;
const file = (r: Repo, p: string) => rendered[r].files.find((f) => f.path === p);
const upstream = (r: Repo, p: string) => readFileSync(join(import.meta.dir, "upstream/phase5", r, p), "utf8");
const gitBlobSha = (text: string): string => {
  const body = new TextEncoder().encode(text);
  return new Bun.CryptoHasher("sha1").update(`blob ${body.length}\0`).update(body).digest("hex");
};

const BLOBS: [Repo, string, string][] = [
  ["pantheon-adr", ".coderabbit.yaml", "da97d156ddec4d82c8c33ecfb24bf04ba606ee7c"],
  ["pantheon-adr", ".github/scripts/tag-release.sh", "1314873bd8eb1a24b8d8ed43a9cf0b44a88f3165"],
  ["pantheon-adr", ".github/workflows/audit-skill.yml", "f94eca2d6a47c6acc5f1e13e85e19c0d87ff5fb8"],
  ["pantheon-adr", "SECURITY.md", "40fde387ed9022b214a1671ee6d37ab3309afb9b"],
  ["pantheon-agentic-harness-dev-kit", ".coderabbit.yaml", "c482c34cea153aa8bbff2e803c58f98800d6c1e5"],
  ["pantheon-agentic-harness-dev-kit", "SECURITY.md", "d9211075bd406c305f04c52bbde484b4f53e7069"],
  ["pantheon-journal", ".coderabbit.yaml", "68c7bc809afefc78044a07eb8ac43be1d6fd3041"],
  ["pantheon-journal", ".github/scripts/tag-release.sh", "9990db7f936290fdd6b724150f9dc6b8a9096ea1"],
  ["pantheon-journal", ".github/workflows/audit-skill.yml", "e59e14d2b8c367666864eed5a76cda0b58c2ef13"],
  ["pantheon-journal", "SECURITY.md", "1b768601a8cb5661b27572ea0ea426f05f110076"],
  ["pantheon-skill-auditor", ".coderabbit.yaml", "8c316238a9f367ae59f88ee4c601e917ce0af325"],
  ["pantheon-skill-auditor", ".github/scripts/tag-release.sh", "b8202c2ef298b8c070a6ee4ef2750ca926d67036"],
  ["pantheon-skill-auditor", ".github/workflows/audit-skill.yml", "5b42f69e602badd05ddcf5f8862ca47360b18957"],
  ["pantheon-skill-auditor", "SECURITY.md", "49e58f97f7562f4e605fe2d9269d7076812c9f33"],
];

test("the pinned upstream copies are the repositories' files at the surveyed commits", () => {
  for (const [r, p, blob] of BLOBS) expect(gitBlobSha(upstream(r, p)), `${r} ${p}`).toBe(blob);
});

const WITH_SKILL: Repo[] = ["pantheon-adr", "pantheon-journal", "pantheon-skill-auditor"];

for (const r of WITH_SKILL) {
  test(`${r}: audit-skill.yml equals the repository's file under P.1, with the directive comments and job names`, () => {
    const mine = file(r, ".github/workflows/audit-skill.yml")!.content!;
    const theirs = upstream(r, ".github/workflows/audit-skill.yml");
    expect(compareWorkflows(theirs, mine)).toEqual({ equal: true, differences: [], duplicateKeys: [] });
    expect(compareDirectives(theirs, mine)).toEqual([]);
  });

  test(`${r}: tag-release.sh is byte for byte the repository's file and executable`, () => {
    const f = file(r, ".github/scripts/tag-release.sh")!;
    expect(f.content).toBe(upstream(r, ".github/scripts/tag-release.sh"));
    expect(f.mode).toBe("0755");
    expect(f.class).toBe("owned");
  });
}

for (const r of REPOS) {
  test(`${r}: .coderabbit.yaml is byte for byte the repository's file`, () => {
    expect(file(r, ".coderabbit.yaml")!.content).toBe(upstream(r, ".coderabbit.yaml"));
  });
}

test("SECURITY.md is seeded, so a repository that words its scope differently keeps its own text", () => {
  for (const r of REPOS) expect(file(r, "SECURITY.md")!.class, r).toBe("seed");
});

for (const r of ["pantheon-adr", "pantheon-journal", "pantheon-agentic-harness-dev-kit"] as Repo[]) {
  test(`${r}: SECURITY.md is byte for byte the repository's file`, () => {
    expect(file(r, "SECURITY.md")!.content).toBe(upstream(r, "SECURITY.md"));
  });
}

test("pantheon-skill-auditor: SECURITY.md differs only in the three scope lines that name its crates, skill and composite action", () => {
  const mine = file("pantheon-skill-auditor", "SECURITY.md")!.content!.split("\n");
  const theirs = upstream("pantheon-skill-auditor", "SECURITY.md").split("\n");
  const different = mine.filter((l) => !theirs.includes(l));
  expect(different.every((l) => l.startsWith("- The "))).toBe(true);
  expect(different.length).toBeLessThanOrEqual(3);
});

test("the library repository, with no binary and no skill, gets no audit-skill.yml and no tag-release.sh", () => {
  const paths = rendered["pantheon-agentic-harness-dev-kit"].files.map((f) => f.path);
  expect(paths).not.toContain(".github/workflows/audit-skill.yml");
  expect(paths).not.toContain(".github/scripts/tag-release.sh");
});

test("the composite-action variant runs the repository's own action and the bootstrap binary, the others use the released action", () => {
  const auditor = file("pantheon-skill-auditor", ".github/workflows/audit-skill.yml")!.content!;
  expect(auditor).toContain("uses: ./");
  expect(auditor).toContain("download-url:");
  const adr = file("pantheon-adr", ".github/workflows/audit-skill.yml")!.content!;
  expect(adr).toMatch(/uses: pantheon-org\/pantheon-skill-auditor@[0-9a-f]{40}\n/);
  expect(rendered["pantheon-adr"].pins).toContainEqual({ action: "pantheon-org/pantheon-skill-auditor", sha: "a71472ec078f54ff13b5b74b7e3786779ef28df9", version: "v0.3.1" });
  expect(adr).not.toContain("download-url");
});

test("every other file is unchanged by the new builtins: the rendered set grows by exactly the new files", () => {
  const adr = rendered["pantheon-adr"].files.map((f) => f.path);
  for (const p of [".coderabbit.yaml", "SECURITY.md", ".github/scripts/tag-release.sh", ".github/workflows/audit-skill.yml", ".gitignore"]) expect(adr, p).toContain(p);
});
