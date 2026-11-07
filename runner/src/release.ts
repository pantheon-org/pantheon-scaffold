import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { formatChecksums } from "./checksums";
import { parseSemver } from "./semver";

export const TARGETS = ["linux-x64", "linux-arm64", "darwin-arm64"] as const;
export const binaryName = (target: string): string => `pantheon-scaffold-${target}`;
const packageFile = (version: string): string => `pantheon-scaffold@${version}`;

/** Every file a release carries. The package files come from `pkl project package`, the binaries from the build script. */
export function assetNames(version: string): string[] {
  const pkg = packageFile(version);
  return ["checksums.txt", ...TARGETS.map(binaryName), pkg, `${pkg}.sha256`, `${pkg}.zip`, `${pkg}.zip.sha256`].sort();
}

/** The package version, from the one `version = "..."` line of pkl/PklProject. */
export function pklProjectVersion(text: string): string {
  const hits = [...text.matchAll(/^[ \t]*version[ \t]*=[ \t]*"([^"]+)"/gm)];
  if (hits.length === 0) throw new Error("PklProject has no version line");
  if (hits.length > 1) throw new Error("PklProject has more than one version line");
  return hits[0][1];
}

/** A release is cut from a tag `v<version>` and the version must be the one in PklProject, because the package URI is built from it. */
export function checkTag(tag: string, pklProject: string): string {
  const m = /^v([0-9A-Za-z.-]+)$/.exec(tag);
  if (!m || !parseSemver(m[1])) throw new Error(`${JSON.stringify(tag)} is not a release tag (expected v<major>.<minor>.<patch>[-pre-release])`);
  const declared = pklProjectVersion(pklProject);
  if (declared !== m[1]) throw new Error(`tag ${tag} does not match the PklProject version ${declared}`);
  return m[1];
}

const sha256 = (bytes: Uint8Array): string => new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
const bare = (path: string): string => readFileSync(path, "utf8").trim();

/**
 * The contents of checksums.txt: the three binaries, the package metadata file (its checksum is the one pkl checks with ::sha256) and
 * the zip. Before it is written, the package files are checked against each other, so a release that disagrees with itself is never
 * attested.
 */
export function assembleChecksums(dir: string, version: string): string {
  const missing = assetNames(version).filter((n) => n !== "checksums.txt" && !existsSync(join(dir, n)));
  if (missing.length) throw new Error(`missing release files: ${missing.join(", ")}`);

  const pkg = packageFile(version);
  const metaBytes = readFileSync(join(dir, pkg));
  const zipBytes = readFileSync(join(dir, `${pkg}.zip`));
  const meta = JSON.parse(metaBytes.toString("utf8")) as { version?: string; packageZipChecksums?: { sha256?: string } };
  if (meta.version !== version) throw new Error(`the package metadata says version ${meta.version}, expected ${version}`);
  if (meta.packageZipChecksums?.sha256 !== sha256(zipBytes)) throw new Error(`${pkg}.zip does not match the checksum in the package metadata`);
  if (bare(join(dir, `${pkg}.sha256`)) !== sha256(metaBytes)) throw new Error(`${pkg}.sha256 does not match the metadata file`);
  if (bare(join(dir, `${pkg}.zip.sha256`)) !== sha256(zipBytes)) throw new Error(`${pkg}.zip.sha256 does not match the zip`);

  const entries = [
    ...TARGETS.map((t) => ({ name: binaryName(t), sha256: sha256(readFileSync(join(dir, binaryName(t)))) })),
    { name: pkg, sha256: sha256(metaBytes) },
    { name: `${pkg}.zip`, sha256: sha256(zipBytes) },
  ];
  return formatChecksums(entries);
}
