import type { KnownBad } from "./preflight";
import { compareSemver, parseSemver } from "./semver";

export const RELEASES_API = "https://api.github.com/repos/pantheon-org/pantheon-scaffold/releases?per_page=100";

export class LatestError extends Error {}

export type Release = { tag_name: string; draft: boolean; prerelease: boolean };

/**
 * The newest release the consumer upgrade workflow should propose, as a version without the leading v: never a draft, a pre-release only when
 * asked for (promotion from pre-release is a separate, reviewed step), never a version on the known-bad list, and never a tag that is not a version.
 */
export function latestVersion(releases: Release[], opts: { includePrereleases?: boolean; knownBad?: KnownBad }): string | null {
  const bad = new Set((opts.knownBad?.versions ?? []).map((v) => v.version));
  const versions = releases
    .filter((x) => !x.draft && (opts.includePrereleases || !x.prerelease))
    .map((x) => x.tag_name.replace(/^v/, ""))
    .filter((v) => parseSemver(v) !== null && !bad.has(v));
  if (versions.length === 0) return null;
  return versions.reduce((a, b) => (compareSemver(b, a) > 0 ? b : a));
}

export type FetchOptions = { fetcher?: (url: string, headers: Record<string, string>) => Promise<string>; url?: string; token?: string; attempts?: number; delayMs?: number };

const download = async (url: string, headers: Record<string, string>): Promise<string> => {
  const res = await fetch(url, { headers, signal: AbortSignal.timeout(15_000) });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.text();
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** The first 100 releases of the package repository. A fetch that fails is retried and then fails closed. */
export async function fetchReleases(opts: FetchOptions = {}): Promise<Release[]> {
  const url = opts.url ?? RELEASES_API;
  const fetcher = opts.fetcher ?? download;
  const headers: Record<string, string> = { Accept: "application/vnd.github+json", ...(opts.token ? { Authorization: `Bearer ${opts.token}` } : {}) };
  const attempts = Math.max(1, opts.attempts ?? 3);
  let text = "";
  for (let i = 1; ; i++) {
    try {
      text = await fetcher(url, headers);
      break;
    } catch (e) {
      if (i >= attempts) throw new LatestError(`could not fetch ${url} after ${attempts} attempts, failing closed: ${(e as Error).message}`);
      await sleep(opts.delayMs ?? 2000);
    }
  }
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    throw new LatestError(`${url} did not return JSON`);
  }
  if (!Array.isArray(json) || json.some((x) => typeof x?.tag_name !== "string")) throw new LatestError(`${url} did not return a list of releases`);
  return json.map((x) => ({ tag_name: x.tag_name, draft: x.draft === true, prerelease: x.prerelease === true }));
}
