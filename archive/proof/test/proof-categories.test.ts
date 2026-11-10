import { expect, test } from "bun:test";
import { categorise } from "../scripts/proof-categories";

// Phase 7: every difference between a rendered file and the repository's own copy gets one category, from rules written down in advance.
// Anything the rules do not name is UNCATEGORISED, which fails the proof.
const base = { repo: "pantheon-adr", mode: "0644", theirsMode: "0644" as string | undefined };
const cat = (o: Partial<Parameters<typeof categorise>[0]> & { path: string; mine: string }) => categorise({ ...base, theirs: o.mine, ...o }).category;

test("identical bytes and mode are identical", () => expect(cat({ path: "deny.toml", mine: "a\n" })).toBe("identical"));

test("a file the repository lacks is new only when the package is known to add it", () => {
  for (const p of ["hk.shared.pkl", ".mise/conf.d/repo.toml", ".github/workflows/dist-check.yml", ".github/workflows/scaffold-check.yml"]) {
    expect(cat({ path: p, mine: "x\n", theirs: undefined, theirsMode: undefined }), p).toBe("new file");
  }
  expect(cat({ path: "surprise.txt", mine: "x\n", theirs: undefined, theirsMode: undefined })).toBe("UNCATEGORISED");
});

test("sync-labels.sh differing only in its mode is the decided mode change (decision 79), any other file is not", () => {
  expect(cat({ path: ".github/scripts/sync-labels.sh", mine: "x\n", mode: "0755" })).toBe("mode change only (decision 79)");
  expect(cat({ path: ".github/scripts/auto-rebase.sh", mine: "x\n", mode: "0755" })).toBe("UNCATEGORISED");
});

const wf = (pin: string, extra = "") => `name: x\n# a comment\non: [push]\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@${pin}${extra}\n`;
const A = "a".repeat(40);
const B = "b".repeat(40);
test("a workflow that differs only in comments and formatting is equal under P.1", () => {
  expect(cat({ path: ".github/workflows/x.yml", theirs: wf(A), mine: wf(A).replace("# a comment\n", "") })).toBe("comments and formatting only (P.1)");
});
test("a workflow whose action commit moved since the repository last updated it is a moved pin", () => {
  expect(cat({ path: ".github/workflows/x.yml", theirs: wf(A), mine: wf(B) })).toBe("pin moved");
});
test("a workflow that differs in content is uncategorised", () => {
  expect(cat({ path: ".github/workflows/x.yml", theirs: wf(A), mine: wf(A).replace("ubuntu-latest", "ubuntu-24.04") })).toBe("UNCATEGORISED");
});

test(".gitignore: only the decided target/ pick and the .pkl-gen/cache/ line may differ (decisions 74 and 87)", () => {
  expect(cat({ path: ".gitignore", theirs: "target/\n.context/\n", mine: "target/\n.context/\n.pkl-gen/cache/\n" })).toBe("recorded difference: decisions 74 and 87");
  expect(cat({ path: ".gitignore", repo: "pantheon-agentic-harness-dev-kit", theirs: "/target\n", mine: "target/\n.pkl-gen/cache/\n" })).toBe("recorded difference: decisions 74 and 87");
  expect(cat({ path: ".gitignore", theirs: "target/\n", mine: "target/\nsecret.txt\n" })).toBe("UNCATEGORISED");
});

const dist = (up: string, down: string) => `[dist.github-action-commits]\n"actions/checkout" = "c"\n"actions/upload-artifact" = "${up}" # v7\n"actions/download-artifact" = "${down}" # v8\n`;
test("dist-workspace.toml: only the two artifact action commits may differ (decision 85)", () => {
  expect(cat({ path: "dist-workspace.toml", theirs: dist("1", "2"), mine: dist("3", "4") })).toBe("recorded difference: decision 85");
  expect(cat({ path: "dist-workspace.toml", theirs: dist("1", "2"), mine: dist("3", "4").replace('"c"', '"d"') })).toBe("UNCATEGORISED");
});

const dep = (ignore: string) => `version: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n  - package-ecosystem: github-actions\n    directory: /\n${ignore}`;
test("dependabot.yml: only the ignore list of the github-actions entry may differ (decision 86)", () => {
  const ign = "    ignore:\n      - dependency-name: actions/upload-artifact\n      - dependency-name: actions/download-artifact\n";
  expect(cat({ path: ".github/dependabot.yml", theirs: dep(""), mine: dep(ign) })).toBe("recorded difference: decision 86");
  expect(cat({ path: ".github/dependabot.yml", theirs: dep(""), mine: dep(ign).replace("directory: /\n    ignore", "directory: /x\n    ignore") })).toBe("UNCATEGORISED");
});

test("SECURITY.md of the skill auditor may differ in its scope bullets only; no other repository may differ", () => {
  const s = (bullet: string) => `# Security\n\nScope:\n\n${bullet}\n- CI/CD workflows.\n`;
  expect(cat({ path: "SECURITY.md", repo: "pantheon-skill-auditor", theirs: s("- The CLI."), mine: s("- The CLI and the crates under `crates/`.") })).toBe("recorded difference: seeded file, the repository's own scope wording");
  expect(cat({ path: "SECURITY.md", repo: "pantheon-adr", theirs: s("- The CLI."), mine: s("- Another.") })).toBe("UNCATEGORISED");
  expect(cat({ path: "SECURITY.md", repo: "pantheon-skill-auditor", theirs: s("- The CLI."), mine: s("- The CLI.").replace("# Security", "# Policy") })).toBe("UNCATEGORISED");
});

test("mise.toml: the decided change of 10 lines added and 4 removed (Phase 3), no other", () => {
  expect(cat({ path: "mise.toml", theirs: "a\n", mine: "b\n", numstat: { added: 10, removed: 4 } })).toBe("migration: mise.toml, 10 lines added and 4 removed");
  expect(cat({ path: "mise.toml", theirs: "a\n", mine: "b\n", numstat: { added: 11, removed: 4 } })).toBe("UNCATEGORISED");
});

test("hk.pkl replaced by the seeded one is the migration of decision 31", () => {
  expect(cat({ path: "hk.pkl", theirs: "old\n", mine: "seeded\n" })).toBe("migration: hk.pkl replaced by the seeded one (decision 31)");
});

test("any other file that differs in bytes is uncategorised", () => {
  expect(cat({ path: "deny.toml", theirs: "a\n", mine: "b\n" })).toBe("UNCATEGORISED");
});

test("the detail says what was found", () => {
  expect(categorise({ ...base, path: "deny.toml", theirs: "a\n", mine: "b\n" }).detail).toContain("bytes differ");
});

test(".gitignore: losing any line other than /target is uncategorised", () => {
  expect(cat({ path: ".gitignore", theirs: "target/\ndist/\n", mine: "target/\n.pkl-gen/cache/\n" })).toBe("UNCATEGORISED");
});

test("SECURITY.md: a repository other than the skill auditor may not differ, even in a scope bullet", () => {
  const s = (bullet: string) => `# Security\n\n${bullet}\n`;
  expect(cat({ path: "SECURITY.md", repo: "pantheon-adr", theirs: s("- The CLI."), mine: s("- The other CLI.") })).toBe("UNCATEGORISED");
});

test("a workflow that loses a comment a tool acts on is uncategorised, not a comment difference", () => {
  const withDirective = "name: x\n'on': # zizmor: ignore[dangerous-triggers] reads only the title\n  pull_request_target:\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo hi\n";
  const without = withDirective.replace(" # zizmor: ignore[dangerous-triggers] reads only the title", "");
  expect(cat({ path: ".github/workflows/x.yml", theirs: withDirective, mine: without })).toBe("UNCATEGORISED");
});
