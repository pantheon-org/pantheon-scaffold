import { afterAll, beforeAll, expect, test } from "bun:test";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseChecksums } from "../src/checksums";
import { assembleChecksums, assetNames, checkTag, pklProjectVersion } from "../src/release";

const root = join(import.meta.dir, "../..");
const project = readFileSync(join(root, "pkl/PklProject"), "utf8");
const VERSION = pklProjectVersion(project);
const sha = (b: Uint8Array | string): string => new Bun.CryptoHasher("sha256").update(b).digest("hex");
const TARGETS = ["linux-x64", "linux-arm64", "darwin-arm64"];

let work = "";
let pkgOut = "";
beforeAll(() => {
  work = mkdtempSync(join(tmpdir(), "scaffold-release-"));
  pkgOut = join(work, "pkg");
  const build = Bun.spawnSync(["pkl", "project", "package", join(root, "pkl"), "--output-path", pkgOut], { stdout: "pipe", stderr: "pipe" });
  if (build.exitCode !== 0) throw new Error(build.stderr.toString());
});
afterAll(() => {
  if (work.startsWith(tmpdir()) && work.includes("scaffold-release-")) rmSync(work, { recursive: true, force: true });
});

/** A release folder with the real package build and fake binaries. */
function releaseDir(name: string): string {
  const dir = join(work, name);
  mkdirSync(dir);
  for (const f of [`pantheon-scaffold@${VERSION}`, `pantheon-scaffold@${VERSION}.zip`, `pantheon-scaffold@${VERSION}.sha256`, `pantheon-scaffold@${VERSION}.zip.sha256`]) copyFileSync(join(pkgOut, f), join(dir, f));
  for (const t of TARGETS) writeFileSync(join(dir, `pantheon-scaffold-${t}`), `binary for ${t}\n`);
  return dir;
}

test("pklProjectVersion reads the one version line of the package block", () => {
  expect(pklProjectVersion(`package {\n  name = "x"\n  version = "1.2.3"\n}\n`)).toBe("1.2.3");
  expect(() => pklProjectVersion(`package {}\n`)).toThrow("no version");
  expect(() => pklProjectVersion(`version = "1.0.0"\nversion = "2.0.0"\n`)).toThrow("more than one");
});

test("a tag that matches the PklProject version passes, pre-release tags included", () => {
  expect(checkTag(`v${VERSION}`, project)).toBe(VERSION);
  expect(checkTag("v0.2.0-rc.1", `version = "0.2.0-rc.1"`)).toBe("0.2.0-rc.1");
});

test("a tag that is not v followed by a version is refused", () => {
  for (const tag of ["0.1.0", "v0.1", "latest", "runner-v0.1.0", "v0.1.0+build", "refs/tags/v0.1.0"]) expect(() => checkTag(tag, `version = "0.1.0"`)).toThrow("not a release tag");
});

test("a tag that differs from the PklProject version is refused, so the package URI cannot point at the wrong release", () => {
  expect(() => checkTag("v0.1.0", `version = "0.0.0"`)).toThrow("does not match the PklProject version 0.0.0");
});

test("the release carries the package files, the three binaries and the checksums", () => {
  expect(assetNames("0.1.0")).toEqual([
    "checksums.txt",
    "pantheon-scaffold-darwin-arm64",
    "pantheon-scaffold-linux-arm64",
    "pantheon-scaffold-linux-x64",
    "pantheon-scaffold@0.1.0",
    "pantheon-scaffold@0.1.0.sha256",
    "pantheon-scaffold@0.1.0.zip",
    "pantheon-scaffold@0.1.0.zip.sha256",
  ]);
});

test("checksums.txt lists the binaries, the package metadata and the zip, and parseChecksums reads it back", () => {
  const dir = releaseDir("ok");
  const parsed = parseChecksums(assembleChecksums(dir, VERSION));
  expect(parsed.map((c) => c.name)).toEqual([
    "pantheon-scaffold-darwin-arm64",
    "pantheon-scaffold-linux-arm64",
    "pantheon-scaffold-linux-x64",
    `pantheon-scaffold@${VERSION}`,
    `pantheon-scaffold@${VERSION}.zip`,
  ]);
  const meta = parsed.find((c) => c.name === `pantheon-scaffold@${VERSION}`)!;
  expect(meta.sha256).toBe(sha(readFileSync(join(dir, `pantheon-scaffold@${VERSION}`))));
  expect(parsed.find((c) => c.name === "pantheon-scaffold-linux-x64")!.sha256).toBe(sha("binary for linux-x64\n"));
});

test("the metadata checksum is the one pkl checks with ::sha256, the same as the .sha256 file", () => {
  const dir = releaseDir("pklsum");
  const meta = parseChecksums(assembleChecksums(dir, VERSION)).find((c) => c.name === `pantheon-scaffold@${VERSION}`)!;
  expect(meta.sha256).toBe(readFileSync(join(dir, `pantheon-scaffold@${VERSION}.sha256`), "utf8").trim());
});

test("a missing binary or package file is an error that lists what is missing", () => {
  const dir = releaseDir("missing");
  rmSync(join(dir, "pantheon-scaffold-linux-arm64"));
  rmSync(join(dir, `pantheon-scaffold@${VERSION}.zip`));
  expect(() => assembleChecksums(dir, VERSION)).toThrow(/missing.*pantheon-scaffold-linux-arm64.*pantheon-scaffold@.*\.zip/s);
});

test("a zip that does not match the checksum in the metadata is refused", () => {
  const dir = releaseDir("badzip");
  writeFileSync(join(dir, `pantheon-scaffold@${VERSION}.zip`), "tampered");
  expect(() => assembleChecksums(dir, VERSION)).toThrow("does not match the checksum in the package metadata");
});

test("a .sha256 file that does not match its file is refused", () => {
  const dir = releaseDir("badsum");
  writeFileSync(join(dir, `pantheon-scaffold@${VERSION}.sha256`), "0".repeat(64));
  expect(() => assembleChecksums(dir, VERSION)).toThrow(".sha256 does not match");
});

test("metadata for another version is refused", () => {
  const dir = releaseDir("badver");
  const path = join(dir, `pantheon-scaffold@${VERSION}`);
  const meta = JSON.parse(readFileSync(path, "utf8"));
  meta.version = "9.9.9";
  writeFileSync(path, JSON.stringify(meta));
  writeFileSync(join(dir, `pantheon-scaffold@${VERSION}.sha256`), sha(readFileSync(path)));
  expect(() => assembleChecksums(dir, VERSION)).toThrow("says version 9.9.9");
});

// The wrapper the release workflow calls.
const script = join(import.meta.dir, "../scripts/release-assets.ts");
const wrap = async (...args: string[]) => {
  const p = Bun.spawn(["bun", script, ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};

test("the wrapper prints the version for a matching tag and fails with exit 1 for another", async () => {
  expect(await wrap("check-tag", `v${VERSION}`)).toMatchObject({ out: VERSION, code: 0 });
  const bad = await wrap("check-tag", "v9.9.9");
  expect(bad.code).toBe(1);
  expect(bad.err).toContain("does not match the PklProject version");
});

test("the wrapper writes checksums.txt into the release folder", async () => {
  const dir = releaseDir("wrapper");
  const r = await wrap("checksums", dir, VERSION);
  expect(r.code).toBe(0);
  expect(parseChecksums(readFileSync(join(dir, "checksums.txt"), "utf8"))).toHaveLength(5);
});

test("the wrapper reports a usage error with exit 2", async () => {
  expect((await wrap()).code).toBe(2);
  expect((await wrap("checksums", "only-one-arg")).code).toBe(2);
});

test("the wrapper prints the PklProject version for a dry run that has no tag", async () => {
  expect(await wrap("version")).toMatchObject({ out: VERSION, code: 0 });
});

test("a .zip.sha256 file that does not match the zip is refused", () => {
  const dir = releaseDir("badzipsum");
  writeFileSync(join(dir, `pantheon-scaffold@${VERSION}.zip.sha256`), "0".repeat(64));
  expect(() => assembleChecksums(dir, VERSION)).toThrow(".zip.sha256 does not match the zip");
});
