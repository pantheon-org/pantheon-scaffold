import { afterEach, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { Gh } from "../src/governance/gh";
import { applyRulesets, dryRunRulesets, planRulesets, readRulesetRecord, readRulesets, requestsOf, restoreRulesets } from "../src/governance/rulesetTask";
import { compareRulesets, toCreateShape, type Ruleset } from "../src/governance/ruleset";
import { cleanupRepos, tempRepo } from "./helpers";

afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");
const fx = (name: string): Record<string, unknown> => JSON.parse(readFileSync(join(import.meta.dir, "fixtures/governance", name), "utf8"));
const desired = (stem: string): Ruleset => JSON.parse(readFileSync(join(root, "governance/desired", `${stem}.json`), "utf8"));
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));
const OWNER = "pantheon-org";
const REPO = "pantheon-adr";
const GOVERNED = [REPO, "pantheon-journal"];
const CONFIRM = `${OWNER}/${REPO}`;
const NOW = () => new Date("2026-11-09T12:00:00.000Z");
const MAIN = desired("protect-main");
const TAGS: Ruleset = { name: "Protect release tags", target: "tag", enforcement: "active", bypass_actors: [], conditions: { ref_name: { include: ["refs/tags/v*"], exclude: [] } }, rules: [{ type: "deletion" }, { type: "non_fast_forward" }] };

type Fake = { gh: Gh; sends: { method: string; path: string; body?: unknown }[]; rulesets: Record<string, unknown>[] };
/** A repository with rulesets, as GitHub keeps them. A POST gives the new ruleset an id and the fields only a GET carries. */
function fake(initial: Record<string, unknown>[], o: { ignoreWrites?: boolean; failOnSend?: number } = {}): Fake {
  const f: Fake = { sends: [], rulesets: clone(initial), gh: undefined as unknown as Gh };
  const idOf = (path: string): number => Number(path.split("/").pop());
  f.gh = {
    get: async (path) => {
      if (path.endsWith("/rulesets")) return f.rulesets.map((r) => ({ id: r.id, name: r.name }));
      const hit = f.rulesets.find((r) => r.id === idOf(path));
      if (!hit) throw new Error(`HTTP 404 ${path}`);
      return clone(hit);
    },
    send: async (method, path, body) => {
      f.sends.push({ method, path, body });
      if (o.failOnSend === f.sends.length) throw new Error("HTTP 422");
      if (o.ignoreWrites) return undefined;
      if (method === "POST") f.rulesets.push({ ...clone(body as object), id: Math.max(0, ...f.rulesets.map((r) => r.id as number)) + 1, node_id: "RRS_new", source: `${OWNER}/${REPO}`, created_at: "2026-11-09" });
      else if (method === "PUT") f.rulesets = f.rulesets.map((r) => (r.id === idOf(path) ? { ...clone(body as object), id: r.id, node_id: r.node_id, source: r.source, created_at: r.created_at } : r));
      else if (method === "DELETE") f.rulesets = f.rulesets.filter((r) => r.id !== idOf(path));
      return undefined;
    },
  };
  return f;
}
const START = (): Record<string, unknown>[] => [{ ...fx("journal-protect-main.get.json"), id: 100, name: "Protect main" }];
const opts = (confirm: string | undefined, recordDir: string) => ({ governed: GOVERNED, recordDir, confirm, now: NOW });
const names = (f: Fake): string[] => f.rulesets.map((r) => r.name as string).sort();
/** The create shapes of everything the fake holds, by name: the state a restore must give back. */
const shapes = (f: Fake): Record<string, Ruleset> => Object.fromEntries(f.rulesets.map((r) => [r.name as string, toCreateShape(r)]));

test("the plan creates what is missing, replaces what differs, leaves alone what matches and what it was not asked about", async () => {
  const changed = clone(MAIN);
  changed.enforcement = "evaluate";
  const live = await readRulesets(fake([...START(), { ...clone(START()[0]), id: 101, name: "Surprise" }]).gh, OWNER, REPO);
  const plan = planRulesets(OWNER, REPO, [changed, TAGS], live);
  expect(plan.actions.map((a) => `${a.kind}:${a.name}`)).toEqual(["update:Protect main", "create:Protect release tags", "untouched:Surprise"]);
  expect(requestsOf(plan).map((r) => `${r.method} ${r.path}`)).toEqual(["PUT repos/pantheon-org/pantheon-adr/rulesets/100", "POST repos/pantheon-org/pantheon-adr/rulesets"]);
  expect(plan.actions[0]).toMatchObject({ differences: ["enforcement: should be evaluate, is active"] });
  expect(requestsOf(planRulesets(OWNER, REPO, [MAIN], live.slice(0, 1)))).toEqual([]);
});

test("two live rulesets with the same name are an error: the task will not guess which to change", async () => {
  const live = await readRulesets(fake([...START(), { ...clone(START()[0]), id: 101 }]).gh, OWNER, REPO);
  expect(() => planRulesets(OWNER, REPO, [MAIN], live)).toThrow("2 rulesets named");
});

test("a dry run reads and sends nothing, and only for a governed repository", async () => {
  const f = fake(START());
  const plan = await dryRunRulesets(f.gh, GOVERNED, OWNER, REPO, [MAIN, TAGS]);
  expect(requestsOf(plan).length).toBe(1);
  expect(f.sends).toEqual([]);
  await expect(dryRunRulesets(f.gh, GOVERNED, OWNER, "pantheon-scaffold", [MAIN])).rejects.toThrow("not one of the governed repositories");
  await expect(dryRunRulesets(f.gh, GOVERNED, OWNER, "../x", [MAIN])).rejects.toThrow("not a valid repository slug");
});

test("applying needs the repository written out in full, and sends nothing without it", async () => {
  const f = fake(START());
  for (const confirm of [undefined, "", REPO, "pantheon-org/pantheon-journal"]) await expect(applyRulesets(f.gh, OWNER, REPO, [MAIN, TAGS], opts(confirm, tempRepo())), String(confirm)).rejects.toThrow(`to apply, pass --confirm ${CONFIRM}`);
  expect(f.sends).toEqual([]);
});

test("applying writes the record first, sends the requests in order, reads again, and ends equal to the desired state", async () => {
  const changed = clone(MAIN);
  changed.enforcement = "evaluate";
  const f = fake(START());
  const dir = tempRepo();
  const r = await applyRulesets(f.gh, OWNER, REPO, [changed, TAGS], opts(CONFIRM, dir));
  expect(r.applied).toBe(true);
  expect(f.sends.map((s) => `${s.method} ${s.path}`)).toEqual(["PUT repos/pantheon-org/pantheon-adr/rulesets/100", "POST repos/pantheon-org/pantheon-adr/rulesets"]);
  const rec = readRulesetRecord(r.recordFile!);
  expect(rec.managed).toEqual(["Protect main", "Protect release tags"]);
  expect(rec.before).toEqual(START());
  expect((JSON.parse(readFileSync(r.recordFile!, "utf8")) as { after: unknown[] }).after.length).toBe(2);
  expect(compareRulesets(changed, shapes(f)["Protect main"])).toEqual([]);
  expect(compareRulesets(TAGS, shapes(f)["Protect release tags"])).toEqual([]);
});

test("a second apply sends nothing and writes no record (it is idempotent)", async () => {
  const f = fake(START());
  const dir = tempRepo();
  await applyRulesets(f.gh, OWNER, REPO, [MAIN, TAGS], opts(CONFIRM, dir));
  f.sends.length = 0;
  const again = await applyRulesets(f.gh, OWNER, REPO, [MAIN, TAGS], opts(CONFIRM, dir));
  expect(again.applied).toBe(false);
  expect(f.sends).toEqual([]);
  expect(readdirSync(dir).length).toBe(1);
});

test("a ruleset the desired state does not mention is never deleted or changed", async () => {
  const f = fake([...START(), { ...clone(START()[0]), id: 101, name: "Surprise" }]);
  await applyRulesets(f.gh, OWNER, REPO, [MAIN, TAGS], opts(CONFIRM, tempRepo()));
  expect(names(f)).toEqual(["Protect main", "Protect release tags", "Surprise"]);
  expect(f.sends.some((s) => s.method === "DELETE")).toBe(false);
});

test("a request that fails stops the rest, and the error names the record that was written first", async () => {
  const changed = clone(MAIN);
  changed.enforcement = "evaluate";
  const f = fake(START(), { failOnSend: 2 });
  const dir = tempRepo();
  await expect(applyRulesets(f.gh, OWNER, REPO, [changed, TAGS], opts(CONFIRM, dir))).rejects.toThrow(/POST .* failed: HTTP 422\. Some requests may have been sent\. Restore with the record /);
  expect(f.sends.length).toBe(2);
  const files = readdirSync(dir);
  expect(files.length).toBe(1);
  expect(readRulesetRecord(join(dir, files[0])).before).toEqual(START());
});

test("a write that does not take effect is an error that names the record to restore from", async () => {
  const f = fake(START(), { ignoreWrites: true });
  await expect(applyRulesets(f.gh, OWNER, REPO, [MAIN, TAGS], opts(CONFIRM, tempRepo()))).rejects.toThrow(/still differ from the desired rulesets\. Restore with the record /);
});

test("a record is never overwritten", async () => {
  const f = fake(START());
  const dir = tempRepo();
  await applyRulesets(f.gh, OWNER, REPO, [MAIN, TAGS], opts(CONFIRM, dir));
  f.rulesets = f.rulesets.filter((r) => r.name === "Protect main");
  await expect(applyRulesets(f.gh, OWNER, REPO, [MAIN, TAGS], opts(CONFIRM, dir))).rejects.toThrow();
  expect(readRulesetRecord(join(dir, readdirSync(dir)[0])).before).toEqual(START());
});

// The rollback path: apply, then restore from the record alone, and the repository is as it was.
test("rollback: restoring puts back what was replaced and deletes what the apply created, and nothing else", async () => {
  const changed = clone(MAIN);
  changed.enforcement = "evaluate";
  const f = fake(START());
  const original = shapes(f);
  const dir = tempRepo();
  const r = await applyRulesets(f.gh, OWNER, REPO, [changed, TAGS], opts(CONFIRM, dir));
  expect(names(f)).toEqual(["Protect main", "Protect release tags"]);
  // Someone adds a ruleset after the apply: the restore must not touch it.
  f.rulesets.push({ ...clone(START()[0]), id: 900, name: "Added later" });
  f.sends.length = 0;

  const dry = await restoreRulesets(f.gh, r.recordFile!, { governed: GOVERNED, apply: false, confirm: undefined });
  expect(dry.restored).toBe(false);
  expect(f.sends).toEqual([]);
  expect(dry.plan.actions.map((a) => `${a.kind}:${a.name}`)).toEqual(["update:Protect main", "delete:Protect release tags", "untouched:Added later"]);

  await expect(restoreRulesets(f.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: undefined })).rejects.toThrow("to apply, pass --confirm");
  expect(f.sends).toEqual([]);
  const done = await restoreRulesets(f.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: CONFIRM });
  expect(done.restored).toBe(true);
  expect(f.sends.map((s) => `${s.method} ${s.path}`)).toEqual(["PUT repos/pantheon-org/pantheon-adr/rulesets/100", "DELETE repos/pantheon-org/pantheon-adr/rulesets/101"]);
  expect(names(f)).toEqual(["Added later", "Protect main"]);
  expect(compareRulesets(original["Protect main"], shapes(f)["Protect main"])).toEqual([]);
  expect(f.rulesets.find((x) => x.name === "Protect main")!.id).toBe(100);
});

test("rollback: a recorded ruleset that has been deleted since is created again, and a restore with nothing to do sends nothing", async () => {
  const changed = clone(MAIN);
  changed.enforcement = "evaluate";
  const f = fake(START());
  const r = await applyRulesets(f.gh, OWNER, REPO, [changed], opts(CONFIRM, tempRepo()));
  f.rulesets = [];
  const done = await restoreRulesets(f.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: CONFIRM });
  expect(done.plan.actions.map((a) => a.kind)).toEqual(["create"]);
  expect(compareRulesets(MAIN, shapes(f)["Protect main"])).toEqual([]);
  f.sends.length = 0;
  const none = await restoreRulesets(f.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: CONFIRM });
  expect(none.restored).toBe(false);
  expect(f.sends).toEqual([]);
});

test("rollback: a restore that does not take effect is an error, not a success", async () => {
  const changed = clone(MAIN);
  changed.enforcement = "evaluate";
  const f = fake(START());
  const r = await applyRulesets(f.gh, OWNER, REPO, [changed], opts(CONFIRM, tempRepo()));
  const stuck = fake(f.rulesets, { ignoreWrites: true });
  await expect(restoreRulesets(stuck.gh, r.recordFile!, { governed: GOVERNED, apply: true, confirm: CONFIRM })).rejects.toThrow(/after the restore .* still differ from the recorded rulesets/);
});

test("a file that is not a ruleset record, or names a repository that is not governed, is refused", async () => {
  const f = fake(START());
  const dir = tempRepo();
  const r = await applyRulesets(f.gh, OWNER, REPO, [MAIN, TAGS], opts(CONFIRM, dir));
  await expect(restoreRulesets(f.gh, r.recordFile!, { governed: ["pantheon-journal"], apply: false, confirm: undefined })).rejects.toThrow("not one of the governed repositories");
  await Bun.write(join(dir, "other.json"), JSON.stringify({ kind: "settings", owner: OWNER, repo: REPO, managed: [], before: [] }));
  await expect(restoreRulesets(f.gh, join(dir, "other.json"), { governed: GOVERNED, apply: false, confirm: undefined })).rejects.toThrow("not a ruleset record");
});

test("an answer that is not a list, or a ruleset without an id, is an error and never an empty repository", async () => {
  const notList: Gh = { get: async () => ({ message: "x" }), send: async () => undefined };
  await expect(readRulesets(notList, OWNER, REPO)).rejects.toThrow("not a list");
  const noId: Gh = { get: async () => [{ name: "x" }], send: async () => undefined };
  await expect(readRulesets(noId, OWNER, REPO)).rejects.toThrow("has no id");
});
