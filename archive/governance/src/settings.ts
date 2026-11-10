import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { assertConfirmed, assertGoverned, type Gh } from "./gh";

/** The merge settings the task manages: squash only, and a branch is deleted when its pull request is merged (task 6.1). */
export type Settings = { allow_squash_merge: boolean; allow_merge_commit: boolean; allow_rebase_merge: boolean; delete_branch_on_merge: boolean };
const KEYS = ["allow_squash_merge", "allow_merge_commit", "allow_rebase_merge", "delete_branch_on_merge"] as const;

export type Change = { key: (typeof KEYS)[number]; from: boolean; to: boolean };
export type Request = { method: "PATCH" | "POST" | "PUT" | "DELETE"; path: string; body?: unknown };
export type SettingsPlan = { repo: string; current: Settings; changes: Change[]; request?: Request };
/** What is written to disk before anything changes, and what restore reads. */
export type SettingsRecord = { kind: "settings"; owner: string; repo: string; createdAt: string; before: Settings; desired: Settings; after?: Settings };

const pick = (j: unknown, where: string): Settings => {
  if (typeof j !== "object" || j === null) throw new Error(`${where}: not an object`);
  const o = j as Record<string, unknown>;
  for (const k of KEYS) if (typeof o[k] !== "boolean") throw new Error(`${where}: ${k} is not true or false`);
  return Object.fromEntries(KEYS.map((k) => [k, o[k]])) as Settings;
};

/** The desired settings file: exactly the four booleans, so a typo is an error and not a silent no-op. */
export function parseDesiredSettings(text: string): Settings {
  const j = JSON.parse(text) as Record<string, unknown>;
  const extra = Object.keys(j).filter((k) => !(KEYS as readonly string[]).includes(k));
  if (extra.length) throw new Error(`the desired settings name ${extra.join(", ")}, which the task does not manage`);
  return pick(j, "the desired settings");
}

export async function readSettings(gh: Gh, owner: string, repo: string): Promise<Settings> {
  return pick(await gh.get(`repos/${owner}/${repo}`), `repos/${owner}/${repo}`);
}

export function planSettings(owner: string, repo: string, desired: Settings, current: Settings): SettingsPlan {
  const changes = KEYS.filter((k) => current[k] !== desired[k]).map((k) => ({ key: k, from: current[k], to: desired[k] }));
  const body = Object.fromEntries(changes.map((c) => [c.key, c.to]));
  return { repo, current, changes, ...(changes.length ? { request: { method: "PATCH" as const, path: `repos/${owner}/${repo}`, body } } : {}) };
}

/** A dry run: reads, plans, and sends nothing. */
export async function dryRunSettings(gh: Gh, governed: string[], owner: string, repo: string, desired: Settings): Promise<SettingsPlan> {
  assertGoverned(governed, owner, repo);
  return planSettings(owner, repo, desired, await readSettings(gh, owner, repo));
}

export type Options = { governed: string[]; recordDir: string; confirm: string | undefined; now?: () => Date };

/**
 * Applies the plan to ONE repository. The settings before are written to a record file first, and a failed write stops everything. The
 * request is sent only when something differs, so a second apply sends nothing. The settings are read again afterwards and must equal
 * the desired ones, otherwise this is an error that names the record to restore from.
 */
export async function applySettings(gh: Gh, owner: string, repo: string, desired: Settings, o: Options): Promise<{ plan: SettingsPlan; recordFile?: string; applied: boolean }> {
  assertGoverned(o.governed, owner, repo);
  assertConfirmed(owner, repo, o.confirm);
  const plan = planSettings(owner, repo, desired, await readSettings(gh, owner, repo));
  if (!plan.request) return { plan, applied: false };
  mkdirSync(o.recordDir, { recursive: true });
  const createdAt = (o.now?.() ?? new Date()).toISOString();
  const recordFile = join(o.recordDir, `${repo}-settings-${createdAt.replace(/[:.]/g, "-")}.json`);
  const record: SettingsRecord = { kind: "settings", owner, repo, createdAt, before: plan.current, desired };
  writeFileSync(recordFile, JSON.stringify(record, null, 2) + "\n", { flag: "wx" });
  await gh.send(plan.request.method, plan.request.path, plan.request.body);
  const after = await readSettings(gh, owner, repo);
  writeFileSync(recordFile, JSON.stringify({ ...record, after }, null, 2) + "\n");
  const wrong = KEYS.filter((k) => after[k] !== desired[k]);
  if (wrong.length) throw new Error(`after the change ${wrong.join(", ")} still differ from the desired settings. Restore with the record ${recordFile}`);
  return { plan, recordFile, applied: true };
}

export function readSettingsRecord(file: string): SettingsRecord {
  const r = JSON.parse(readFileSync(file, "utf8")) as SettingsRecord;
  if (r.kind !== "settings" || typeof r.owner !== "string" || typeof r.repo !== "string") throw new Error(`${file} is not a settings record`);
  return { ...r, before: pick(r.before, `${file}: before`), desired: pick(r.desired, `${file}: desired`) };
}

/** Puts the settings in a record back. Dry run unless applied and confirmed, and it sends only what differs now. */
export async function restoreSettings(gh: Gh, recordFile: string, o: { governed: string[]; apply: boolean; confirm: string | undefined }): Promise<{ plan: SettingsPlan; restored: boolean }> {
  const rec = readSettingsRecord(recordFile);
  assertGoverned(o.governed, rec.owner, rec.repo);
  const plan = planSettings(rec.owner, rec.repo, rec.before, await readSettings(gh, rec.owner, rec.repo));
  if (!o.apply || !plan.request) return { plan, restored: false };
  assertConfirmed(rec.owner, rec.repo, o.confirm);
  await gh.send(plan.request.method, plan.request.path, plan.request.body);
  const after = await readSettings(gh, rec.owner, rec.repo);
  const wrong = KEYS.filter((k) => after[k] !== rec.before[k]);
  if (wrong.length) throw new Error(`after the restore ${wrong.join(", ")} still differ from the recorded settings`);
  return { plan, restored: true };
}
