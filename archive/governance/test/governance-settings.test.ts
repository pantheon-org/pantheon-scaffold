import { afterEach, expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { Gh } from "../src/governance/gh";
import { applySettings, dryRunSettings, parseDesiredSettings, planSettings, readSettingsRecord, restoreSettings, type Settings } from "../src/governance/settings";
import { cleanupRepos, tempRepo } from "./helpers";

afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");
const DESIRED: Settings = parseDesiredSettings(readFileSync(join(root, "governance/desired/settings.json"), "utf8"));
const GOVERNED = ["pantheon-adr", "pantheon-journal"];
const OWNER = "pantheon-org";
const BEFORE: Settings = { allow_squash_merge: true, allow_merge_commit: true, allow_rebase_merge: true, delete_branch_on_merge: false };
const NOW = () => new Date("2026-11-09T12:00:00.000Z");

type Fake = { gh: Gh; sends: { method: string; path: string; body: unknown }[]; gets: string[]; state: Record<string, Record<string, unknown>> };
/** A GitHub with two repositories. `ignoreWrites` makes a write that does not take effect, `failWrites` one that fails. */
function fake(o: { ignoreWrites?: boolean; failWrites?: boolean } = {}): Fake {
  const state: Record<string, Record<string, unknown>> = {
    "pantheon-adr": { name: "pantheon-adr", private: false, ...BEFORE },
    "pantheon-journal": { name: "pantheon-journal", private: false, ...BEFORE },
  };
  const f: Fake = { sends: [], gets: [], state, gh: undefined as unknown as Gh };
  f.gh = {
    get: async (path) => {
      f.gets.push(path);
      const repo = path.split("/")[2];
      if (!state[repo]) throw new Error(`HTTP 404 ${path}`);
      return structuredClone(state[repo]);
    },
    send: async (method, path, body) => {
      f.sends.push({ method, path, body });
      if (o.failWrites) throw new Error("HTTP 403");
      if (!o.ignoreWrites) Object.assign(state[path.split("/")[2]], body as object);
      return undefined;
    },
  };
  return f;
}
const opts = (confirm: string | undefined, recordDir: string) => ({ governed: GOVERNED, recordDir, confirm, now: NOW });

test("the desired settings file names exactly the four settings, and a typo is an error", () => {
  expect(DESIRED).toEqual({ allow_squash_merge: true, allow_merge_commit: false, allow_rebase_merge: false, delete_branch_on_merge: true });
  expect(() => parseDesiredSettings('{"allow_squash_merge":true,"allow_merge_commit":false,"allow_rebase_merge":false,"delete_branch_on_merge":true,"allow_auto_merge":true}')).toThrow("allow_auto_merge");
  expect(() => parseDesiredSettings('{"allow_squash_merge":true}')).toThrow("allow_merge_commit is not true or false");
  expect(() => parseDesiredSettings('{"allow_squash_merge":"yes","allow_merge_commit":false,"allow_rebase_merge":false,"delete_branch_on_merge":true}')).toThrow("allow_squash_merge");
});

test("the plan lists only what differs, and the request carries only that", () => {
  const p = planSettings(OWNER, "pantheon-adr", DESIRED, BEFORE);
  expect(p.changes).toEqual([
    { key: "allow_merge_commit", from: true, to: false },
    { key: "allow_rebase_merge", from: true, to: false },
    { key: "delete_branch_on_merge", from: false, to: true },
  ]);
  expect(p.request).toEqual({ method: "PATCH", path: "repos/pantheon-org/pantheon-adr", body: { allow_merge_commit: false, allow_rebase_merge: false, delete_branch_on_merge: true } });
  expect(planSettings(OWNER, "pantheon-adr", DESIRED, DESIRED)).toEqual({ repo: "pantheon-adr", current: DESIRED, changes: [] });
});

test("a dry run reads and sends nothing", async () => {
  const f = fake();
  const p = await dryRunSettings(f.gh, GOVERNED, OWNER, "pantheon-adr", DESIRED);
  expect(p.changes.length).toBe(3);
  expect(f.sends).toEqual([]);
  expect(f.gets).toEqual(["repos/pantheon-org/pantheon-adr"]);
});

test("only a governed repository is touched, and a repository that is not readable is an error", async () => {
  const f = fake();
  await expect(dryRunSettings(f.gh, GOVERNED, OWNER, "pantheon-scaffold", DESIRED)).rejects.toThrow("not one of the governed repositories");
  await expect(dryRunSettings(f.gh, GOVERNED, OWNER, "../x", DESIRED)).rejects.toThrow("not a valid repository slug");
  await expect(applySettings(f.gh, OWNER, "pantheon-scaffold", DESIRED, opts("pantheon-org/pantheon-scaffold", tempRepo()))).rejects.toThrow("not one of the governed repositories");
  expect(f.gets).toEqual([]);
  expect(f.sends).toEqual([]);
  await expect(dryRunSettings(fake().gh, ["pantheon-gone"], OWNER, "pantheon-gone", DESIRED)).rejects.toThrow("HTTP 404");
});

test("applying needs the repository written out in full, and sends nothing without it", async () => {
  const f = fake();
  for (const confirm of [undefined, "", "pantheon-adr", "pantheon-org/pantheon-journal", "PANTHEON-ORG/pantheon-adr"]) {
    await expect(applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts(confirm, tempRepo())), String(confirm)).rejects.toThrow("to apply, pass --confirm pantheon-org/pantheon-adr");
  }
  expect(f.sends).toEqual([]);
});

test("applying writes the record first, sends only what differs, reads again and checks", async () => {
  const f = fake();
  const dir = tempRepo();
  const r = await applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir));
  expect(r.applied).toBe(true);
  expect(f.sends).toEqual([{ method: "PATCH", path: "repos/pantheon-org/pantheon-adr", body: { allow_merge_commit: false, allow_rebase_merge: false, delete_branch_on_merge: true } }]);
  expect(f.state["pantheon-journal"]).toMatchObject(BEFORE);
  const rec = readSettingsRecord(r.recordFile!);
  expect(rec.before).toEqual(BEFORE);
  expect(rec.desired).toEqual(DESIRED);
  expect(JSON.parse(readFileSync(r.recordFile!, "utf8")).after).toEqual(DESIRED);
  expect(r.recordFile).toContain("pantheon-adr-settings-2026-11-09T12-00-00-000Z.json");
});

test("a second apply sends nothing and writes no record (it is idempotent)", async () => {
  const f = fake();
  const dir = tempRepo();
  await applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir));
  f.sends.length = 0;
  const again = await applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir));
  expect(again.applied).toBe(false);
  expect(f.sends).toEqual([]);
  expect(readdirSync(dir).length).toBe(1);
});

test("a write that fails leaves the record that was written first, and says so", async () => {
  const f = fake({ failWrites: true });
  const dir = tempRepo();
  await expect(applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir))).rejects.toThrow("HTTP 403");
  const files = readdirSync(dir);
  expect(files.length).toBe(1);
  expect(readSettingsRecord(join(dir, files[0])).before).toEqual(BEFORE);
});

test("a write that does not take effect is an error that names the record to restore from", async () => {
  const f = fake({ ignoreWrites: true });
  const dir = tempRepo();
  await expect(applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir))).rejects.toThrow(/still differ from the desired settings\. Restore with the record /);
});

test("a record is never overwritten: two applies in the same instant stop at the second", async () => {
  const f = fake();
  const dir = tempRepo();
  await applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir));
  Object.assign(f.state["pantheon-adr"], BEFORE);
  await expect(applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir))).rejects.toThrow();
  expect(readSettingsRecord(join(dir, readdirSync(dir)[0])).before).toEqual(BEFORE);
});

test("restore puts the recorded settings back, by the record alone", async () => {
  const f = fake();
  const dir = tempRepo();
  const r = await applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir));
  expect(f.state["pantheon-adr"]).toMatchObject(DESIRED);
  f.sends.length = 0;
  const dry = await restoreSettings(f.gh, r.recordFile!, { governed: GOVERNED, apply: false, confirm: undefined });
  expect(dry.restored).toBe(false);
  expect(dry.plan.changes.length).toBe(3);
  expect(f.sends).toEqual([]);
  const done = await restoreSettings(f.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: "pantheon-org/pantheon-adr" });
  expect(done.restored).toBe(true);
  expect(f.state["pantheon-adr"]).toMatchObject(BEFORE);
  expect(f.sends).toEqual([{ method: "PATCH", path: "repos/pantheon-org/pantheon-adr", body: { allow_merge_commit: true, allow_rebase_merge: true, delete_branch_on_merge: false } }]);
});

test("restoring needs the confirmation, sends only what differs now, and does nothing when nothing differs", async () => {
  const f = fake();
  const dir = tempRepo();
  const r = await applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir));
  f.sends.length = 0;
  await expect(restoreSettings(f.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: undefined })).rejects.toThrow("to apply, pass --confirm");
  Object.assign(f.state["pantheon-adr"], { allow_merge_commit: true });
  const partial = await restoreSettings(f.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: "pantheon-org/pantheon-adr" });
  expect(partial.plan.request?.body).toEqual({ allow_rebase_merge: true, delete_branch_on_merge: false });
  f.sends.length = 0;
  const none = await restoreSettings(f.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: "pantheon-org/pantheon-adr" });
  expect(none.restored).toBe(false);
  expect(f.sends).toEqual([]);
});

test("a file that is not a settings record, or names a repository that is not governed, is refused", async () => {
  const f = fake();
  const dir = tempRepo();
  const r = await applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir));
  await expect(restoreSettings(f.gh, r.recordFile!, { governed: ["pantheon-journal"], apply: false, confirm: undefined })).rejects.toThrow("not one of the governed repositories");
  await Bun.write(join(dir, "other.json"), JSON.stringify({ kind: "ruleset", owner: OWNER, repo: "pantheon-adr" }));
  await expect(restoreSettings(f.gh, join(dir, "other.json"), { governed: GOVERNED, apply: false, confirm: undefined })).rejects.toThrow("not a settings record");
  expect(existsSync(r.recordFile!)).toBe(true);
});

test("a restore that does not take effect is an error, not a success", async () => {
  const f = fake();
  const dir = tempRepo();
  const r = await applySettings(f.gh, OWNER, "pantheon-adr", DESIRED, opts("pantheon-org/pantheon-adr", dir));
  const stuck = fake({ ignoreWrites: true });
  Object.assign(stuck.state["pantheon-adr"], DESIRED);
  await expect(restoreSettings(stuck.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: "pantheon-org/pantheon-adr" })).rejects.toThrow(/after the restore .* still differ from the recorded settings/);
});
