import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { evalContractJson, scaffoldBin } from "./support/contract";
import type { Schema } from "../src/schema";
import { cleanupRepos, has, tempRepo } from "./helpers";
import { afterEach } from "bun:test";

// Phase 5 wave A: the verbatim files of the `binary` toggle and the one `.gitignore`. Git blobs are those of pantheon-adr at 1e2fd46; the
// same three binary files are identical in pantheon-journal and pantheon-skill-auditor, and .gitignore in all three (survey, 08-11-2026).
afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const render = async (fixture: string) =>
  parseContract(await evalContractJson(join(import.meta.dir, "fixtures", fixture, "pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const binary = await render("phase5-binary");
const library = await render("phase5-library");
const file = (c: typeof binary, path: string) => c.files.find((f) => f.path === path)!;
const gitBlobSha = (text: string): string => {
  const body = new TextEncoder().encode(text);
  return new Bun.CryptoHasher("sha1").update(`blob ${body.length}\0`).update(body).digest("hex");
};

const BLOBS: Record<string, string> = {
  ".github/actionlint.yaml": "010277fad034ccbc7ec5b08827de18b22c23b703",
};

test("a repository with a binary gets .gitignore, actionlint.yaml, dist-workspace.toml and release.yml", () => {
  expect(binary.files.map((f) => f.path).sort()).toEqual([".github/actionlint.yaml", ".github/workflows/release.yml", ".gitignore", "dist-workspace.toml"]);
});

test("a repository without a binary gets .gitignore only", () => {
  expect(library.files.map((f) => f.path)).toEqual([".gitignore"]);
});

for (const [path, blob] of Object.entries(BLOBS)) {
  test(`${path} is byte for byte the pinned pantheon-adr file, owned and not executable`, () => {
    const f = file(binary, path);
    expect(gitBlobSha(f.content!)).toBe(blob);
    expect(f.class).toBe("owned");
    expect(f.mode).toBe("0644");
  });
}

// Decision 85: Dependabot bumped upload-artifact and download-artifact inside the generated release.yml on 04-11-2026 and dist-workspace.toml,
// which tells dist which commits to write, was not changed, so `dist generate --check` fails on the repositories' main. The file is rendered
// with the two commits release.yml uses, v7.0.2 and v8.0.2, and is otherwise the pantheon-adr file (blob e08f6a7 at 1e2fd46).
test("dist-workspace.toml is the pantheon-adr file with exactly the two action commits that release.yml uses", () => {
  const upstream = readFileSync(join(import.meta.dir, "upstream/phase5/pantheon-adr/dist-workspace.toml"), "utf8");
  expect(gitBlobSha(upstream)).toBe("e08f6a76e797aebe650685131f709dc53e0dcb20");
  const expected = upstream
    .replace("043fb46d1a93c77aae656e7c1c64a875d1fc6a0a", "cf430e030ddbb5b0abf93d22962f4752f3646cd9")
    .replace("3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c", "9000827ccba6bdab643e8b6fd33ac0654aef8333");
  const f = file(binary, "dist-workspace.toml");
  expect(f.content).toBe(expected);
  expect(f.class).toBe("owned");
  expect(f.mode).toBe("0644");
});

// Decision 87: the runner keeps its package cache in .pkl-gen/cache, which must not be committed, so the shipped .gitignore is the pantheon-adr
// file (blob 66e8098 at 1e2fd46) plus the line `.pkl-gen/cache/`.
test(".gitignore is the pantheon-adr file plus exactly the line .pkl-gen/cache/, owned and not executable", () => {
  const upstream = readFileSync(join(import.meta.dir, "upstream/phase5/pantheon-adr/.gitignore"), "utf8");
  expect(gitBlobSha(upstream)).toBe("66e8098804d3b4b7ddc5cf4f00cfa8dd94d2c49f");
  const f = file(binary, ".gitignore");
  expect(f.content).toBe(upstream + ".pkl-gen/cache/\n");
  expect(f.class).toBe("owned");
  expect(f.mode).toBe("0644");
});

test(".gitignore is the same in both kinds of repository: target/ is the decided pick (decision 74)", () => {
  expect(file(library, ".gitignore").content).toBe(file(binary, ".gitignore").content);
  expect(file(library, ".gitignore").content!.split("\n")[0]).toBe("target/");
});

test("release.yml is external: listed with no content, because dist generate writes it", () => {
  const f = file(binary, ".github/workflows/release.yml");
  expect(f.class).toBe("external");
  expect(f.content).toBeUndefined();
});

test("an external release.yml is never written, and an existing one is left alone", async () => {
  const dir = tempRepo();

  const config = join(import.meta.dir, "fixtures/phase5-binary/pantheon.pkl");
  const run = () => Bun.spawn([scaffoldBin(), "render", config, "--root", dir, "--allow-local-package", join(root, "pkl")], { stdout: "pipe", stderr: "pipe" });
  const p = run();
  expect(await p.exited).toBe(0);
  expect(has(dir, ".github/workflows/release.yml")).toBe(false);
  expect(has(dir, "dist-workspace.toml")).toBe(true);
});
