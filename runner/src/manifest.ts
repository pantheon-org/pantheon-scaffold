import { existsSync } from "node:fs";
import { join } from "node:path";
import type { FileClass } from "./contract";

export const MANIFEST_VERSION = 1;
export const TOOL_DIR = ".pkl-gen";
export const MANIFEST_PATH = `${TOOL_DIR}/manifest.json`;

export type ManifestEntry = { class: FileClass; hash?: string };
export type Manifest = { manifestVersion: 1; files: Record<string, ManifestEntry> };

export const emptyManifest = (): Manifest => ({ manifestVersion: MANIFEST_VERSION, files: {} });

export class ManifestError extends Error {}

export const sha256 = (text: string): string => new Bun.CryptoHasher("sha256").update(text).digest("hex");

/** Deterministic text: sorted keys, two-space indent, one trailing newline, so a merge conflict stays small. */
export function serialiseManifest(m: Manifest): string {
  const files: Record<string, ManifestEntry> = {};
  for (const path of Object.keys(m.files).sort()) {
    const e = m.files[path];
    files[path] = e.hash === undefined ? { class: e.class } : { class: e.class, hash: e.hash };
  }
  return JSON.stringify({ manifestVersion: m.manifestVersion, files }, null, 2) + "\n";
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
  return m;
}
