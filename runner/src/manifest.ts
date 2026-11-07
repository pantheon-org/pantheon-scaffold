import { existsSync } from "node:fs";
import { join } from "node:path";
import type { FileClass } from "./contract";

export const MANIFEST_VERSION = 1;
export const TOOL_DIR = ".pkl-gen";
export const MANIFEST_PATH = `${TOOL_DIR}/manifest.json`;

export type ManifestEntry = { class: FileClass; hash?: string; removeOnDetach?: true };
export type Manifest = { manifestVersion: 1; packageVersion?: string; files: Record<string, ManifestEntry> };

export const emptyManifest = (): Manifest => ({ manifestVersion: MANIFEST_VERSION, files: {} });

export class ManifestError extends Error {}

export const sha256 = (text: string): string => new Bun.CryptoHasher("sha256").update(text).digest("hex");

/** Deterministic text: sorted keys, two-space indent, one trailing newline, so a merge conflict stays small. */
export function serialiseManifest(m: Manifest): string {
  const files: Record<string, ManifestEntry> = {};
  for (const path of Object.keys(m.files).sort()) {
    const e = m.files[path];
    files[path] = { class: e.class, ...(e.hash === undefined ? {} : { hash: e.hash }), ...(e.removeOnDetach ? { removeOnDetach: true as const } : {}) };
  }
  return JSON.stringify({ manifestVersion: m.manifestVersion, ...(m.packageVersion === undefined ? {} : { packageVersion: m.packageVersion }), files }, null, 2) + "\n";
}

export async function readManifest(root: string): Promise<Manifest> {
  const path = join(root, MANIFEST_PATH);
  if (!existsSync(path)) return emptyManifest();
  let parsed: unknown;
  try {
    parsed = JSON.parse(await Bun.file(path).text());
  } catch {
    throw new ManifestError(`${MANIFEST_PATH} is not valid JSON (a merge conflict? run with --repair)`);
  }
  const m = parsed as Manifest;
  if (m?.manifestVersion !== MANIFEST_VERSION)
    throw new ManifestError(`${MANIFEST_PATH} has manifestVersion ${JSON.stringify(m?.manifestVersion)}, this runner supports ${MANIFEST_VERSION}`);
  if (!m.files || typeof m.files !== "object") throw new ManifestError(`${MANIFEST_PATH} has no files map`);
  if (m.packageVersion !== undefined && typeof m.packageVersion !== "string") throw new ManifestError(`${MANIFEST_PATH} has a packageVersion that is not text`);
  return m;
}
