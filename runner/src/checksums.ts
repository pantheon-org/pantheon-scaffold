export type Checksum = { sha256: string; name: string };

/** Release assets are listed in sha256sum format: 64 hex characters, a space, an optional `*` binary marker, then the file name. */
const LINE = /^([0-9a-f]{64}) [ *](\S.*)$/;

export function parseChecksums(text: string): Checksum[] {
  const out: Checksum[] = [];
  for (const line of text.split("\n")) {
    if (line.trim() === "") continue;
    const m = LINE.exec(line);
    if (!m) throw new Error(`${JSON.stringify(line)} is not a checksum line (expected sha256sum format)`);
    out.push({ sha256: m[1], name: m[2] });
  }
  return out;
}

/** The checksum of one named file. A missing or repeated name is an error: a checksum must never be guessed or picked. */
export function findChecksum(text: string, name: string): string {
  const hits = parseChecksums(text).filter((c) => c.name === name);
  if (hits.length === 0) throw new Error(`no checksum for ${name} in the release's checksums file`);
  if (hits.length > 1) throw new Error(`more than one checksum for ${name} in the release's checksums file`);
  return hits[0].sha256;
}

export const checksumAssetUrl = (base: string, version: string): string => `${base.replace(/\/+$/, "")}/v${version}/checksums.txt`;
