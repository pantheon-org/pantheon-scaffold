import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { checkRepo, type DesiredRepos } from "../src/governance/live";
import { compareRulesets, requiredChecks, toCreateShape, type Ruleset } from "../src/governance/ruleset";

const fx = (name: string): Record<string, unknown> => JSON.parse(readFileSync(join(import.meta.dir, "fixtures/governance", name), "utf8"));
const desiredDir = join(import.meta.dir, "../../governance/desired");
const desiredFile = (stem: string): Ruleset => JSON.parse(readFileSync(join(desiredDir, `${stem}.json`), "utf8"));
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));

// The fixtures are recorded GETs (09-11-2026) of the live rulesets of the Rust repositories.
const journalMain = (): Record<string, unknown> => fx("journal-protect-main.get.json");

test("a GET becomes the create shape: the fields of one particular ruleset are stripped", () => {
  const r = toCreateShape(journalMain());
  for (const k of ["id", "node_id", "_links", "source", "source_type", "created_at", "updated_at", "current_user_can_bypass"]) expect(r, k).not.toHaveProperty(k);
  expect(Object.keys(r).sort()).toEqual(["bypass_actors", "conditions", "enforcement", "name", "rules", "target"]);
  expect(r.name).toBe("Protect main");
});

test("converting twice changes nothing", () => {
  const once = toCreateShape(journalMain());
  expect(toCreateShape({ ...once })).toEqual(once);
});

test("a ruleset without a name, a target, an enforcement or rules is refused", () => {
  for (const k of ["name", "target", "enforcement", "rules"]) {
    const g = journalMain();
    delete g[k];
    expect(() => toCreateShape(g), k).toThrow(`a ruleset needs a "${k}"`);
  }
});

test("an integration_id that is null means any app, and is the same as no integration_id", () => {
  const withNull = journalMain();
  const rules = clone(withNull.rules as { type: string; parameters?: { required_status_checks?: { context: string; integration_id?: number | null }[] } }[]);
  for (const c of rules.find((r) => r.type === "required_status_checks")!.parameters!.required_status_checks!) c.integration_id = null;
  withNull.rules = rules;
  expect(compareRulesets(toCreateShape(journalMain()), toCreateShape(withNull))).toEqual([]);
});

test("the order GitHub returns checks, rules and bypass actors in does not matter", () => {
  // The skill auditor lists the same five required checks in another order.
  expect(compareRulesets(toCreateShape(fx("journal-protect-main.get.json")), toCreateShape(fx("skill-auditor-protect-main.get.json")))).toEqual([]);
  const shuffled = journalMain();
  shuffled.rules = [...(shuffled.rules as unknown[])].reverse();
  expect(compareRulesets(toCreateShape(journalMain()), toCreateShape(shuffled))).toEqual([]);
});

test("the four Rust repositories hold the same five required checks", () => {
  const sets = ["journal-protect-main.get.json", "skill-auditor-protect-main.get.json", "adr-protect-main.get.json"].map((f) => requiredChecks(toCreateShape(fx(f))));
  for (const s of sets) expect(s).toEqual(["Build, test, lint", "Cargo Deny", "Conventional Commit Title", "Inline Scripts", "zizmor"]);
});

test("the desired files are the create shape of the recorded journal rulesets, and need no conversion", () => {
  expect(compareRulesets(desiredFile("protect-main"), toCreateShape(journalMain()))).toEqual([]);
  expect(compareRulesets(desiredFile("require-cargo-deny"), toCreateShape(fx("journal-require-cargo-deny.get.json")))).toEqual([]);
  for (const stem of ["protect-main", "require-cargo-deny"]) expect(toCreateShape({ ...desiredFile(stem) }), stem).toEqual(desiredFile(stem));
});

test("every difference is reported in words", () => {
  const d = toCreateShape(journalMain());
  const mut = (f: (r: Ruleset) => void): string[] => {
    const l = clone(d);
    f(l);
    return compareRulesets(d, l);
  };
  expect(mut((l) => (l.enforcement = "disabled"))).toEqual(["enforcement: should be active, is disabled"]);
  expect(mut((l) => (l.target = "tag"))).toEqual(["target: should be branch, is tag"]);
  expect(mut((l) => (l.rules = l.rules.filter((r) => r.type !== "deletion")))).toEqual(["rule deletion: should be present, is missing"]);
  expect(mut((l) => l.rules.push({ type: "creation" }))).toEqual(["rule creation: is present, should not be"]);
  expect(mut((l) => (l.bypass_actors = []))[0]).toStartWith("bypass actors:");
  expect(mut((l) => (l.conditions = { ref_name: { include: ["refs/heads/other"], exclude: [] } }))[0]).toStartWith("conditions:");
  const checks = (l: Ruleset) => (l.rules.find((r) => r.type === "required_status_checks")!.parameters!.required_status_checks as { context: string }[]);
  expect(mut((l) => checks(l).pop())).toEqual([expect.stringMatching(/^rule required_status_checks: missing from live: /)]);
  expect(mut((l) => checks(l).push({ context: "Extra" }))).toEqual(["rule required_status_checks: extra in live: Extra"]);
  expect(mut((l) => ((l.rules.find((r) => r.type === "pull_request")!.parameters as Record<string, unknown>).required_approving_review_count = 1))[0]).toStartWith("rule pull_request: should be");
});

// The live check, against a fake API that answers like GitHub does.
const ruleset = (id: number, name: string): Record<string, unknown> => ({ ...fx("journal-protect-main.get.json"), id, name });
function fakeApi(rulesets: Record<string, unknown>[] | "unreadable" | "not a list") {
  const calls: string[] = [];
  const api = async (path: string): Promise<unknown> => {
    calls.push(path);
    if (rulesets === "unreadable") throw new Error("HTTP 404");
    if (rulesets === "not a list") return { message: "x" };
    if (path.endsWith("/rulesets")) return rulesets.map((r) => ({ id: r.id, name: r.name }));
    const hit = rulesets.find((r) => path.endsWith(`/rulesets/${r.id}`));
    if (!hit) throw new Error(`no ${path}`);
    return hit;
  };
  return { api, calls };
}
const wanted = (...names: string[]): Ruleset[] => names.map((n) => ({ ...desiredFile("protect-main"), name: n }));

test("a repository whose rulesets match the desired ones has no problem, and only GETs are made", async () => {
  const { api, calls } = fakeApi([ruleset(1, "Protect main")]);
  expect(await checkRepo("pantheon-org", "pantheon-journal", wanted("Protect main"), api)).toEqual({ repo: "pantheon-journal", problems: [] });
  expect(calls).toEqual(["repos/pantheon-org/pantheon-journal/rulesets", "repos/pantheon-org/pantheon-journal/rulesets/1"]);
});

test("a missing ruleset, an extra one and a changed one are each a problem", async () => {
  const missing = await checkRepo("pantheon-org", "pantheon-journal", wanted("Protect main", "Require Cargo Deny"), fakeApi([ruleset(1, "Protect main")]).api);
  expect(missing.problems).toEqual(['ruleset "Require Cargo Deny" is missing']);
  const extra = await checkRepo("pantheon-org", "pantheon-journal", wanted("Protect main"), fakeApi([ruleset(1, "Protect main"), ruleset(2, "Surprise")]).api);
  expect(extra.problems).toEqual(['ruleset "Surprise" is present and is not in the desired state']);
  const changed = { ...ruleset(1, "Protect main"), enforcement: "evaluate" };
  expect((await checkRepo("pantheon-org", "pantheon-journal", wanted("Protect main"), fakeApi([changed]).api)).problems).toEqual(['ruleset "Protect main": enforcement: should be active, is evaluate']);
});

test("a repository that cannot be read is an error, never a pass", async () => {
  await expect(checkRepo("pantheon-org", "pantheon-journal", wanted("Protect main"), fakeApi("unreadable").api)).rejects.toThrow("HTTP 404");
  await expect(checkRepo("pantheon-org", "pantheon-journal", wanted("Protect main"), fakeApi("not a list").api)).rejects.toThrow("not a list");
});

test("a slug that is not a repository slug is refused before anything is read", async () => {
  const { api, calls } = fakeApi([]);
  for (const repo of ["x/../y", "UPPER", "a b", "", "-x"]) await expect(checkRepo("pantheon-org", repo, [], api), repo).rejects.toThrow("not a valid repository slug");
  expect(calls).toEqual([]);
});

test("repos.json names only desired files that exist, and the four repositories", () => {
  const d = JSON.parse(readFileSync(join(desiredDir, "repos.json"), "utf8")) as DesiredRepos;
  expect(d.owner).toBe("pantheon-org");
  expect(Object.keys(d.repos).sort()).toEqual(["pantheon-adr", "pantheon-agentic-harness-dev-kit", "pantheon-journal", "pantheon-skill-auditor"]);
  for (const stems of Object.values(d.repos)) for (const s of stems) expect(() => desiredFile(s), s).not.toThrow();
});
