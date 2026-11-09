import { expectPins, unpin } from "./helpers";
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { compareDirectives, compareWorkflows } from "../src/equality";
import { evalContractJson } from "../src/pkl";
import type { Schema } from "../src/schema";

const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const render = async (fixture: string) =>
  parseContract(await evalContractJson(join(import.meta.dir, "fixtures", fixture, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const adr = await render("workflows-b");
const kit = await render("workflows-b-kit");
const auditor = await render("workflows-b-auditor");
type C = typeof adr;
const file = (c: C, path: string) => c.files.find((f) => f.path === path)?.content;
const text = (rel: string) => readFileSync(join(import.meta.dir, "upstream", rel), "utf8");
const gitBlobSha = (t: string): string => {
  const body = new TextEncoder().encode(t);
  return new Bun.CryptoHasher("sha1").update(`blob ${body.length}\0`).update(body).digest("hex");
};

// The pinned copies: pantheon-adr at 1e2fd46 (the other two repositories with a binary and a skill have the same blobs, except for a comment in
// release-please.yml), the library repository at 42f48b4, and pantheon-skill-auditor at b9e706e.
const ADR: Record<string, string> = { "release-please": "8b04f6aa", zizmor: "c7c1412f", agnix: "8c3d0ba0", aislop: "1d1b8344", dependabot: "349959db" };
const KIT: Record<string, string> = { "release-please": "26950f15", zizmor: "e26564d1", agnix: "b34c6731", aislop: "a6b31de4", "rust-ci": "6e1725bb", dependabot: "e8a1d9f4" };

test("the pinned copies are the repositories' files at the pinned commits", () => {
  for (const [n, b] of Object.entries(ADR)) expect(gitBlobSha(text(`workflows/${n}.yml`)).slice(0, 8), `adr ${n}`).toBe(b);
  for (const [n, b] of Object.entries(KIT)) expect(gitBlobSha(text(`workflows-kit/${n}.yml`)).slice(0, 8), `kit ${n}`).toBe(b);
  expect(gitBlobSha(text("workflows-auditor/zizmor.yml")).slice(0, 8)).toBe("a63f097a");
});

test("the files each kind of repository gets", () => {
  expect(adr.files.map((f) => f.path).sort()).toEqual([
    ".agnix.toml", ".github/dependabot.yml", ".github/workflows/agnix.yml", ".github/workflows/aislop.yml", ".github/workflows/release-please.yml",
    ".github/workflows/rust-ci.yml", ".github/workflows/zizmor.yml", ".github/zizmor.yml",
  ]);
  // No binary: no cargo-dist release.yml to configure zizmor for, so no .github/zizmor.yml.
  expect(kit.files.map((f) => f.path).sort()).toEqual([
    ".agnix.toml", ".github/dependabot.yml", ".github/workflows/agnix.yml", ".github/workflows/aislop.yml", ".github/workflows/release-please.yml",
    ".github/workflows/rust-ci.yml", ".github/workflows/zizmor.yml",
  ]);
  expect(auditor.files.map((f) => f.path).sort()).toEqual([".agnix.toml", ".github/dependabot.yml", ".github/workflows/zizmor.yml", ".github/zizmor.yml"]);
});

// Each rendering is compared with the pinned copy of the repository it stands for: P.1, the comments a tool acts on, job ids and names.
const jobs = (t: string) => Object.entries((Bun.YAML.parse(t) as { jobs: Record<string, { name?: string }> }).jobs).map(([id, j]) => [id, j.name]);
const same = (label: string, c: C, path: string, upstream: string) => {
  test(`${label}: ${path} equals the pinned copy under P.1`, () => {
    expect(compareWorkflows(unpin(upstream), unpin(file(c, path)!))).toEqual({ equal: true, differences: [], duplicateKeys: [] });
  });
  if (path.includes("/workflows/")) {
    test(`${label}: ${path} keeps the comments a tool acts on and its job ids and names`, () => {
      expect(compareDirectives(upstream, file(c, path)!)).toEqual([]);
      expect(jobs(file(c, path)!)).toEqual(jobs(upstream));
    });
  }
};
for (const n of ["release-please", "zizmor", "agnix", "aislop"]) same("adr-like", adr, `.github/workflows/${n}.yml`, text(`workflows/${n}.yml`));
same("adr-like", adr, ".github/workflows/rust-ci.yml", text("workflows/rust-ci.yml"));
// Decision 86: a repository with a binary also tells Dependabot to leave the two artifact actions of the generated release.yml alone.
// The only difference from the repository's own file is that ignore list, added here to the pinned copy.
const withDistIgnore = (upstream: string): string =>
  upstream.replace(`    cooldown:\n      default-days: 7\n    labels:\n      - "dependencies"\n      - "github-actions"`, `    cooldown:\n      default-days: 7\n    ignore:\n      - dependency-name: "actions/upload-artifact"\n      - dependency-name: "actions/download-artifact"\n    labels:\n      - "dependencies"\n      - "github-actions"`);
same("adr-like", adr, ".github/dependabot.yml", withDistIgnore(text("workflows/dependabot.yml")));
for (const n of ["release-please", "zizmor", "agnix", "aislop", "rust-ci"]) same("library-like", kit, `.github/workflows/${n}.yml`, text(`workflows-kit/${n}.yml`));
same("library-like", kit, ".github/dependabot.yml", text("workflows-kit/dependabot.yml"));
same("auditor-like", auditor, ".github/workflows/zizmor.yml", text("workflows-auditor/zizmor.yml"));

test("the verbatim config files are byte for byte the repositories' files", () => {
  expect(gitBlobSha(file(adr, ".github/zizmor.yml")!)).toBe("152581e2ee6553c9d6a4f83838763e478f24d399");
  expect(gitBlobSha(file(auditor, ".github/zizmor.yml")!)).toBe("9e9c78bbc3117929af57b362572d7714f7eebad6");
  expect(gitBlobSha(file(adr, ".agnix.toml")!)).toBe("760a2847b7a01acb4af786c3ca75796ee78b9ac5");
  expect(gitBlobSha(file(kit, ".agnix.toml")!)).toBe("0a08924f0c41e390032e8a493aaf0291e7d8da41");
});

test("a repository without a skill differs from one with a skill only in the first line of .agnix.toml", () => {
  const a = file(adr, ".agnix.toml")!.split("\n");
  const k = file(kit, ".agnix.toml")!.split("\n");
  expect(k.slice(1)).toEqual(a.slice(1));
  expect(k[0]).not.toBe(a[0]);
});

test("the tool versions the workflows pin are the ones in Tools.pkl, the same ones mise.toml pins", () => {
  const tools = readFileSync(join(root, "pkl/Tools.pkl"), "utf8");
  const tool = (name: string) => new RegExp(`const ${name}: String = "([^"]+)"`).exec(tools)![1];
  const agnix = file(adr, ".github/workflows/agnix.yml")!;
  expect(agnix).toContain(`AGNIX_VERSION: ${tool("agnix")}`);
  expect(agnix).toContain(tool("agnixSha256"));
  expect(file(adr, ".github/workflows/zizmor.yml")!).toContain(`version: ${tool("zizmor")}`);
  expect(file(adr, ".github/workflows/aislop.yml")!).toContain(`aislop@${tool("aislop")}`);
});

test("the new pins are each a commit with a version tag, and the workflows that use them report them", () => {
  expectPins(adr.pins.filter((p) => ["googleapis/release-please-action", "zizmorcore/zizmor-action", "actions/setup-node", "peter-evans/find-comment", "peter-evans/create-or-update-comment"].includes(p.action)), ["googleapis/release-please-action", "zizmorcore/zizmor-action", "actions/setup-node", "peter-evans/find-comment", "peter-evans/create-or-update-comment"]);
});

test("wave B uses no escape hatch in any of the three renders", () => {
  for (const c of [adr, kit, auditor]) expect(c.escapeHatch.uses).toBe(0);
});

test("actionlint is clean on the rendered workflows of a repository with a binary and of one without", async () => {
  const { mkdirSync, mkdtempSync, rmSync, writeFileSync } = await import("node:fs");
  const { tmpdir } = await import("node:os");
  for (const [label, c] of [["adr-like", adr], ["library-like", kit]] as const) {
    const dir = mkdtempSync(join(tmpdir(), "scaffold-actionlint-"));
    try {
      mkdirSync(join(dir, ".github/workflows"), { recursive: true });
      const files = c.files.filter((f) => f.path.startsWith(".github/workflows/")).map((f) => f.path);
      for (const f of c.files.filter((f) => files.includes(f.path))) writeFileSync(join(dir, f.path), f.content!);
      const r = Bun.spawnSync(["actionlint", ...files], { cwd: dir, stdout: "pipe", stderr: "pipe" });
      expect(r.stdout.toString() + r.stderr.toString(), `${label} actionlint output`).toBe("");
    } finally {
      if (dir.startsWith(tmpdir()) && dir.includes("scaffold-actionlint-")) rmSync(dir, { recursive: true, force: true });
    }
  }
});
