import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import bundledSnapshot from "./known-bad.bundled.json";
import { TOOL_DIR } from "./manifest";
import type { KnownBad } from "./preflight";
import { parseSemver } from "./semver";

export const KNOWN_BAD_URL = "https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/main/known-bad.json";
export const CACHE_DIR = `${TOOL_DIR}/cache`;
export const CACHE_FILE = `${CACHE_DIR}/known-bad.json`;

export type KnownBadResult = { list: KnownBad; source: "fetched" | "cache" | "bundled"; note?: string };
export type KnownBadOptions = { url?: string; fetcher?: (url: string) => Promise<string>; bundled?: KnownBad; timeoutMs?: number };

/** Returns the list when the text is a valid known-bad document, otherwise null. */
export function parseKnownBad(text: string): KnownBad | null {
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    return null;
  }
  const versions = (json as { versions?: unknown } | null)?.versions;
  if (!Array.isArray(versions)) return null;
  const clean: KnownBad["versions"] = [];
  for (const v of versions) {
    if (!v || typeof v.version !== "string" || typeof v.reason !== "string" || !parseSemver(v.version)) return null;
    clean.push({ version: v.version, reason: v.reason });
  }
  return { versions: clean };
}

const download = (timeoutMs: number) => async (url: string): Promise<string> => {
  const res = await fetch(url, { signal: AbortSignal.timeout(timeoutMs) });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.text();
};

/**
 * The known-bad list (decisions 59 and 65): fetched and cached, with a snapshot bundled in the runner as the last fallback.
 * A fetch that fails or returns something invalid never stops a run, it falls back and says so.
 */
export async function loadKnownBad(root: string, opts: KnownBadOptions = {}): Promise<KnownBadResult> {
  const url = opts.url ?? KNOWN_BAD_URL;
  const fetcher = opts.fetcher ?? download(opts.timeoutMs ?? 5000);
  const cachePath = join(root, CACHE_FILE);
  let why = "";
  try {
    const list = parseKnownBad(await fetcher(url));
    if (list) {
      mkdirSync(dirname(cachePath), { recursive: true });
      writeFileSync(cachePath, JSON.stringify(list, null, 2) + "\n");
      return { list, source: "fetched" };
    }
    why = "the fetched list is not valid";
  } catch (e) {
    why = `the fetch failed (${(e as Error).message})`;
  }
  const cached = existsSync(cachePath) ? parseKnownBad(readFileSync(cachePath, "utf8")) : null;
  if (cached) return { list: cached, source: "cache", note: `known-bad list: ${why}, using the cached list` };
  return { list: opts.bundled ?? (bundledSnapshot as KnownBad), source: "bundled", note: `known-bad list: ${why}, using the bundled snapshot` };
}
