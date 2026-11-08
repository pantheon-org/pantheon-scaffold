// Pin updates (decisions 16, 58 and 71): for each action pinned in pkl/Pins.pkl, find its latest published release, resolve the tag to a commit, check
// that the commit belongs to the upstream repository and that the tag is a release, and move the pin only to a newer release. A pin whose commit is
// not the commit of its recorded tag is a problem too. Nothing here writes to GitHub, and a GitHub error is a problem, never "unchanged".
import { compareSemver, parseSemver } from "./semver";

export type PinLine = { name: string; action: string; sha: string; version: string };
export type PinChange = { name: string; action: string; from: string; to: string; toSha: string; major: boolean };
export type PinProblem = { name: string; message: string };

const LINE = /^(?<name>[A-Za-z][A-Za-z0-9]*): Pin = new \{ action = "(?<action>[^"]+)"; sha = "(?<sha>[0-9a-f]{40})"; version = "(?<version>[^"]+)" \}$/;

export function parsePins(text: string): PinLine[] {
  const out: PinLine[] = [];
  for (const line of text.split("\n")) {
    if (!/: Pin = new \{/.test(line)) continue;
    const m = LINE.exec(line);
    if (!m?.groups) throw new Error(`cannot read the pin: ${line}`);
    out.push({ name: m.groups.name, action: m.groups.action, sha: m.groups.sha, version: m.groups.version });
  }
  return out;
}

/** owner/repo of an action, which may carry a path (owner/repo/.github/workflows/x.yml). */
export const repoOf = (action: string): string => action.split("/").slice(0, 2).join("/");

export type PinApi = {
  /** The tag of the latest published release (not a draft or a pre-release), or null when the repository has none. */
  latestRelease(repo: string): Promise<string | null>;
  /** The commit a tag points to, with an annotated tag followed to its commit. */
  tagCommit(repo: string, tag: string): Promise<string>;
  /** Whether the tag has a published release. */
  isRelease(repo: string, tag: string): Promise<boolean>;
  /** Whether the commit exists in the repository. */
  commitExists(repo: string, sha: string): Promise<boolean>;
};

export async function updatePins(pins: PinLine[], api: PinApi): Promise<{ changes: PinChange[]; problems: PinProblem[] }> {
  const changes: PinChange[] = [];
  const problems: PinProblem[] = [];
  for (const pin of pins) {
    const repo = repoOf(pin.action);
    const problem = (message: string) => problems.push({ name: pin.name, message });
    try {
      const current = parseSemver(pin.version);
      const latest = await api.latestRelease(repo);
      if (latest === null) {
        problem(`no release found on ${repo}`);
        continue;
      }
      const next = parseSemver(latest);
      if (!current || !next) {
        problem(`${pin.action}: ${!current ? pin.version : latest} is not a version, the update will not guess`);
        continue;
      }
      if (compareSemver(latest, pin.version) <= 0) {
        // Up to date (or the latest release is older): the pin must still be the commit of its recorded tag.
        const commit = await api.tagCommit(repo, pin.version);
        if (commit !== pin.sha) problem(`${pin.action} ${pin.version} resolves to ${commit}, not the pinned ${pin.sha}`);
        continue;
      }
      if (!(await api.isRelease(repo, latest))) {
        problem(`${pin.action} ${latest} is a tag but not a published release`);
        continue;
      }
      const toSha = await api.tagCommit(repo, latest);
      if (!(await api.commitExists(repo, toSha))) {
        problem(`${pin.action} ${latest} resolves to ${toSha}, which does not belong to ${repo}`);
        continue;
      }
      changes.push({ name: pin.name, action: pin.action, from: pin.version, to: latest, toSha, major: next.major !== current.major });
    } catch (e) {
      problem(`${pin.action}: ${(e as Error).message}`);
    }
  }
  return { changes, problems };
}

/** Rewrites the sha and version of the named pin lines. Everything else in the file is left as it is, and applying a change twice is harmless. */
export function applyChanges(text: string, changes: PinChange[]): string {
  const lines = text.split("\n");
  for (const c of changes) {
    const i = lines.findIndex((l) => l.startsWith(`${c.name}: Pin = new {`));
    if (i === -1) throw new Error(`no pin named ${c.name} in the file`);
    lines[i] = lines[i].replace(/sha = "[0-9a-f]{40}"; version = "[^"]+"/, `sha = "${c.toSha}"; version = "${c.to}"`);
  }
  return lines.join("\n");
}

/** The text of freshness.json for a pin check that succeeded at `nowMs`. */
export function renderFreshness(nowMs: number): string {
  return JSON.stringify({ schemaVersion: 1, lastSuccessfulPinCheck: new Date(nowMs).toISOString().replace(/\.\d{3}Z$/, "Z") }, null, 2) + "\n";
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** The real answers, from the GitHub REST API. Only the update script uses it. A 5xx is retried, a 404 is an answer. */
export function githubApi(token: string | undefined, base = "https://api.github.com"): PinApi {
  const headers: Record<string, string> = { Accept: "application/vnd.github+json", ...(token ? { Authorization: `Bearer ${token}` } : {}) };
  async function get(path: string): Promise<{ status: number; json: any }> {
    for (let attempt = 1; ; attempt++) {
      const res = await fetch(`${base}${path}`, { headers, signal: AbortSignal.timeout(20_000) });
      if (res.status >= 500 && attempt < 3) {
        await sleep(2000);
        continue;
      }
      return { status: res.status, json: res.status === 404 ? null : await res.json().catch(() => null) };
    }
  }
  const must = async (path: string) => {
    const r = await get(path);
    if (r.status !== 200) throw new Error(`GET ${path} answered HTTP ${r.status}`);
    return r.json;
  };
  return {
    async latestRelease(repo) {
      const r = await get(`/repos/${repo}/releases/latest`);
      if (r.status === 404) return null;
      if (r.status !== 200) throw new Error(`GET /repos/${repo}/releases/latest answered HTTP ${r.status}`);
      return r.json.tag_name as string;
    },
    async tagCommit(repo, tag) {
      let obj = (await must(`/repos/${repo}/git/ref/tags/${encodeURIComponent(tag)}`)).object;
      while (obj.type === "tag") obj = (await must(`/repos/${repo}/git/tags/${obj.sha}`)).object;
      if (obj.type !== "commit") throw new Error(`${repo} tag ${tag} points to a ${obj.type}, not a commit`);
      return obj.sha as string;
    },
    async isRelease(repo, tag) {
      const r = await get(`/repos/${repo}/releases/tags/${encodeURIComponent(tag)}`);
      return r.status === 200 && r.json?.draft === false;
    },
    async commitExists(repo, sha) {
      return (await get(`/repos/${repo}/commits/${sha}`)).status === 200;
    },
  };
}
