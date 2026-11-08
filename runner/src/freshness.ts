// The freshness record (decisions 52, 57, 61 and 83): the UTC time of the last successful pin check, kept as freshness.json on the `freshness`
// branch of the package repository and written only by the pin-update workflow. The consumer upgrade workflow fails visibly when it is stale.
export const FRESHNESS_URL = "https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/freshness/freshness.json";
export const MAX_AGE_DAYS = 30;
const DAY = 86_400_000;
/// A record dated this far ahead is clock skew. Further ahead than this it is a data error.
const SKEW_MS = DAY;

export class FreshnessError extends Error {}

export type Freshness = { state: "fresh" | "stale"; ageDays: number; checkedAt: string };

/** The time in a record, or an error. Only a UTC time that ends in Z is accepted, so the record never depends on a time zone. */
export function parseFreshness(text: string): Date {
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    throw new FreshnessError("the freshness record is not valid JSON");
  }
  const rec = json as { schemaVersion?: unknown; lastSuccessfulPinCheck?: unknown } | null;
  if (rec?.schemaVersion !== 1) throw new FreshnessError(`the freshness record has schemaVersion ${JSON.stringify(rec?.schemaVersion)}, this runner reads 1`);
  const when = rec.lastSuccessfulPinCheck;
  if (typeof when !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(when)) throw new FreshnessError("lastSuccessfulPinCheck must be a UTC time such as 2026-11-08T12:00:00Z");
  const date = new Date(when);
  if (Number.isNaN(date.getTime()) || date.toISOString().replace(/\.\d{3}Z$/, "Z") !== when) throw new FreshnessError(`lastSuccessfulPinCheck ${when} is not a real time`);
  return date;
}

export type CheckOptions = { fetcher?: (url: string) => Promise<string>; url?: string; now?: number; maxAgeDays?: number; attempts?: number; delayMs?: number };

const download = async (url: string): Promise<string> => {
  const res = await fetch(url, { signal: AbortSignal.timeout(15_000) });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.text();
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Fetches the record and says whether it is fresh. A fetch that fails is retried and then fails closed: an unreadable record is never fresh. */
export async function checkFreshness(opts: CheckOptions = {}): Promise<Freshness> {
  const url = opts.url ?? FRESHNESS_URL;
  const fetcher = opts.fetcher ?? download;
  const attempts = Math.max(1, opts.attempts ?? 3);
  let text = "";
  for (let i = 1; ; i++) {
    try {
      text = await fetcher(url);
      break;
    } catch (e) {
      if (i >= attempts) throw new FreshnessError(`could not fetch ${url} after ${attempts} attempts, failing closed: ${(e as Error).message}`);
      await sleep(opts.delayMs ?? 2000);
    }
  }
  const checked = parseFreshness(text);
  const age = (opts.now ?? Date.now()) - checked.getTime();
  if (age < -SKEW_MS) throw new FreshnessError(`the freshness record is dated in the future (${checked.toISOString()}), which is a clock or data error`);
  const limit = (opts.maxAgeDays ?? MAX_AGE_DAYS) * DAY;
  return { state: age > limit ? "stale" : "fresh", ageDays: Math.max(0, Math.floor(age / DAY)), checkedAt: checked.toISOString().replace(/\.\d{3}Z$/, "Z") };
}
