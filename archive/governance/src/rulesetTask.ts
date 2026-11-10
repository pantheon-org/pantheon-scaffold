import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { assertConfirmed, assertGoverned, type Gh } from "./gh";
import { compareRulesets, toCreateShape, type Ruleset } from "./ruleset";
import type { Request } from "./settings";

/** A ruleset as GitHub holds it: its id, the full GET body (kept in the record) and its create shape. */
export type LiveRuleset = { id: number; got: Record<string, unknown>; shape: Ruleset };

export type RulesetAction =
  | { kind: "unchanged"; name: string }
  | { kind: "create"; name: string; request: Request }
  | { kind: "update"; name: string; id: number; differences: string[]; request: Request }
  /** Only a restore deletes, and only a ruleset the apply created. */
  | { kind: "delete"; name: string; id: number; request: Request }
  /** A live ruleset the desired state does not mention. The task never deletes it. */
  | { kind: "untouched"; name: string; reason: string };
export type RulesetPlan = { repo: string; actions: RulesetAction[] };
/** What is written to disk before anything changes, and what restore reads. `managed` is the names the apply was allowed to touch. */
export type RulesetRecord = { kind: "rulesets"; owner: string; repo: string; createdAt: string; managed: string[]; before: Record<string, unknown>[]; after?: Record<string, unknown>[] };
export type Options = { governed: string[]; recordDir: string; confirm: string | undefined; now?: () => Date };

export async function readRulesets(gh: Gh, owner: string, repo: string): Promise<LiveRuleset[]> {
  const list = await gh.get(`repos/${owner}/${repo}/rulesets`);
  if (!Array.isArray(list)) throw new Error(`${owner}/${repo}: the rulesets answer is not a list`);
  const out: LiveRuleset[] = [];
  for (const item of list as { id: number }[]) {
    if (typeof item?.id !== "number") throw new Error(`${owner}/${repo}: a ruleset has no id`);
    const got = (await gh.get(`repos/${owner}/${repo}/rulesets/${item.id}`)) as Record<string, unknown>;
    // Without admin rights GitHub leaves the bypass actors out. A plan made from that would silently drop them, so it is refused.
    if (!("bypass_actors" in got)) throw new Error(`${owner}/${repo}: this token cannot see the bypass actors of ruleset ${item.id}. The ruleset task needs admin rights on the repository`);
    out.push({ id: item.id, got, shape: toCreateShape(got) });
  }
  return out;
}

/** What to send so the repository holds the desired rulesets. Only a ruleset that differs or is missing produces a request. */
export function planRulesets(owner: string, repo: string, wanted: Ruleset[], live: LiveRuleset[]): RulesetPlan {
  const base = `repos/${owner}/${repo}/rulesets`;
  const actions: RulesetAction[] = [];
  for (const w of wanted) {
    const hits = live.filter((l) => l.shape.name === w.name);
    if (hits.length > 1) throw new Error(`${repo} has ${hits.length} rulesets named ${JSON.stringify(w.name)}: the task will not guess which one to change`);
    if (!hits.length) actions.push({ kind: "create", name: w.name, request: { method: "POST", path: base, body: w } });
    else {
      const differences = compareRulesets(w, hits[0].shape);
      actions.push(differences.length ? { kind: "update", name: w.name, id: hits[0].id, differences, request: { method: "PUT", path: `${base}/${hits[0].id}`, body: w } } : { kind: "unchanged", name: w.name });
    }
  }
  for (const l of live) if (!wanted.some((w) => w.name === l.shape.name)) actions.push({ kind: "untouched", name: l.shape.name, reason: "not in the desired state, and the task never deletes a ruleset it was not asked about" });
  return { repo, actions };
}

const pending = (a: RulesetAction): a is Exclude<RulesetAction, { kind: "unchanged" | "untouched" }> => a.kind !== "unchanged" && a.kind !== "untouched";
export const requestsOf = (plan: RulesetPlan): Request[] => plan.actions.filter(pending).map((a) => a.request);

/** A dry run: reads, plans, and sends nothing. */
export async function dryRunRulesets(gh: Gh, governed: string[], owner: string, repo: string, wanted: Ruleset[]): Promise<RulesetPlan> {
  assertGoverned(governed, owner, repo);
  return planRulesets(owner, repo, wanted, await readRulesets(gh, owner, repo));
}

const send = (gh: Gh, r: Request) => gh.send(r.method, r.path, r.body);

/**
 * Applies the plan to ONE repository. Every ruleset as it is now is written to a record first (and the write must succeed), the requests
 * are sent in order and stop at the first failure, and afterwards the rulesets are read again and must equal the desired ones. A second
 * apply sends nothing. A failure names the record to restore from.
 */
export async function applyRulesets(gh: Gh, owner: string, repo: string, wanted: Ruleset[], o: Options): Promise<{ plan: RulesetPlan; recordFile?: string; applied: boolean }> {
  assertGoverned(o.governed, owner, repo);
  assertConfirmed(owner, repo, o.confirm);
  const live = await readRulesets(gh, owner, repo);
  const plan = planRulesets(owner, repo, wanted, live);
  const requests = requestsOf(plan);
  if (!requests.length) return { plan, applied: false };
  mkdirSync(o.recordDir, { recursive: true });
  const createdAt = (o.now?.() ?? new Date()).toISOString();
  const recordFile = join(o.recordDir, `${repo}-rulesets-${createdAt.replace(/[:.]/g, "-")}.json`);
  const record: RulesetRecord = { kind: "rulesets", owner, repo, createdAt, managed: wanted.map((w) => w.name), before: live.map((l) => l.got) };
  writeFileSync(recordFile, JSON.stringify(record, null, 2) + "\n", { flag: "wx" });
  for (const r of requests) {
    try {
      await send(gh, r);
    } catch (e) {
      throw new Error(`${r.method} ${r.path} failed: ${(e as Error).message}. Some requests may have been sent. Restore with the record ${recordFile}`);
    }
  }
  const afterLive = await readRulesets(gh, owner, repo);
  writeFileSync(recordFile, JSON.stringify({ ...record, after: afterLive.map((l) => l.got) }, null, 2) + "\n");
  const wrong = planRulesets(owner, repo, wanted, afterLive).actions.filter(pending);
  if (wrong.length) throw new Error(`after the change ${wrong.map((a) => a.name).join(", ")} still differ from the desired rulesets. Restore with the record ${recordFile}`);
  return { plan, recordFile, applied: true };
}

export function readRulesetRecord(file: string): RulesetRecord {
  const r = JSON.parse(readFileSync(file, "utf8")) as RulesetRecord;
  if (r.kind !== "rulesets" || typeof r.owner !== "string" || typeof r.repo !== "string" || !Array.isArray(r.before) || !Array.isArray(r.managed)) throw new Error(`${file} is not a ruleset record`);
  return r;
}

/**
 * What puts the recorded rulesets back: a recorded ruleset that is missing is created, one that differs is replaced by the recorded one,
 * and a ruleset the apply was allowed to manage that did not exist before is deleted. Nothing else is touched.
 */
export function planRestore(rec: RulesetRecord, live: LiveRuleset[]): RulesetPlan {
  const base = `repos/${rec.owner}/${rec.repo}/rulesets`;
  const before = rec.before.map(toCreateShape);
  const actions: RulesetAction[] = [];
  for (const b of before) {
    const hit = live.find((l) => l.shape.name === b.name);
    if (!hit) actions.push({ kind: "create", name: b.name, request: { method: "POST", path: base, body: b } });
    else {
      const differences = compareRulesets(b, hit.shape);
      actions.push(differences.length ? { kind: "update", name: b.name, id: hit.id, differences, request: { method: "PUT", path: `${base}/${hit.id}`, body: b } } : { kind: "unchanged", name: b.name });
    }
  }
  for (const l of live) {
    if (before.some((b) => b.name === l.shape.name)) continue;
    actions.push(rec.managed.includes(l.shape.name) ? { kind: "delete", name: l.shape.name, id: l.id, request: { method: "DELETE", path: `${base}/${l.id}` } } : { kind: "untouched", name: l.shape.name, reason: "not managed by the apply that made this record" });
  }
  return { repo: rec.repo, actions };
}

/** Puts the rulesets in a record back. A dry run unless applied and confirmed. */
export async function restoreRulesets(gh: Gh, recordFile: string, o: { governed: string[]; apply: boolean; confirm: string | undefined }): Promise<{ plan: RulesetPlan; restored: boolean }> {
  const rec = readRulesetRecord(recordFile);
  assertGoverned(o.governed, rec.owner, rec.repo);
  const plan = planRestore(rec, await readRulesets(gh, rec.owner, rec.repo));
  const requests = requestsOf(plan);
  if (!o.apply || !requests.length) return { plan, restored: false };
  assertConfirmed(rec.owner, rec.repo, o.confirm);
  for (const r of requests) await send(gh, r);
  const wrong = planRestore(rec, await readRulesets(gh, rec.owner, rec.repo)).actions.filter(pending);
  if (wrong.length) throw new Error(`after the restore ${wrong.map((a) => a.name).join(", ")} still differ from the recorded rulesets`);
  return { plan, restored: true };
}
