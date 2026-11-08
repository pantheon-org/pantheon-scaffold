import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { parseFreshness } from "../src/freshness";
import { applyChanges, parsePins, renderFreshness, repoOf, updatePins, type PinApi } from "../src/pinupdate";

const sha = (c: string) => c.repeat(40);
const PINS = `/// Action pins.
module x

class Pin { action: String }

checkout: Pin = new { action = "actions/checkout"; sha = "${sha("a")}"; version = "v1.0.0" }
setup: Pin = new { action = "actions/setup-node"; sha = "${sha("b")}"; version = "v2.1.0" }
nested: Pin = new { action = "owner/repo/.github/workflows/x.yml"; sha = "${sha("c")}"; version = "v3.0.0" }
`;

/** A fake of the four GitHub answers the update needs. Releases: repo -> latest tag. Commits: "repo@tag" -> sha. */
function fake(o: { latest?: Record<string, string | null>; commits?: Record<string, string>; releases?: string[]; missing?: string[]; throws?: string[] } = {}): PinApi {
  return {
    async latestRelease(repo) {
      if (o.throws?.includes(repo)) throw new Error("HTTP 504");
      return o.latest?.[repo] ?? null;
    },
    async tagCommit(repo, tag) {
      const c = o.commits?.[`${repo}@${tag}`];
      if (!c) throw new Error(`no such tag ${tag}`);
      return c;
    },
    async isRelease(repo, tag) {
      return (o.releases ?? Object.keys(o.commits ?? {})).includes(`${repo}@${tag}`);
    },
    async commitExists(repo, c) {
      return !(o.missing ?? []).includes(`${repo}@${c}`);
    },
  };
}
const current = (): PinApi =>
  fake({
    latest: { "actions/checkout": "v1.0.0", "actions/setup-node": "v2.1.0", "owner/repo": "v3.0.0" },
    commits: { "actions/checkout@v1.0.0": sha("a"), "actions/setup-node@v2.1.0": sha("b"), "owner/repo@v3.0.0": sha("c") },
  });

test("every pin line is read, with the repository part of a workflow path", () => {
  const pins = parsePins(PINS);
  expect(pins.map((p) => [p.name, p.action, p.version])).toEqual([["checkout", "actions/checkout", "v1.0.0"], ["setup", "actions/setup-node", "v2.1.0"], ["nested", "owner/repo/.github/workflows/x.yml", "v3.0.0"]]);
  expect(repoOf("owner/repo/.github/workflows/x.yml")).toBe("owner/repo");
});

test("a pin line that cannot be read is an error, never skipped", () => {
  expect(() => parsePins(`bad: Pin = new { action = "a/b"; sha = "short"; version = "v1" }`)).toThrow("cannot read the pin");
});

test("the real Pins.pkl is read completely", () => {
  const text = readFileSync(join(import.meta.dir, "../../pkl/Pins.pkl"), "utf8");
  const pins = parsePins(text);
  expect(pins.length).toBe(text.split("\n").filter((l) => /: Pin = new \{/.test(l)).length);
  expect(pins.map((p) => p.name).sort()).toEqual(["checkout", "createGithubAppToken", "createOrUpdateComment", "findComment", "miseAction", "plumber", "releasePleaseAction", "setupNode", "zizmorAction"]);
  for (const p of pins) expect(p.sha).toMatch(/^[0-9a-f]{40}$/);
});

test("pins that are already at the latest release and match their tag change nothing and raise no problem", async () => {
  expect(await updatePins(parsePins(PINS), current())).toEqual({ changes: [], problems: [] });
});

test("a pin behind its latest release is updated to that release's commit, and a major bump is flagged", async () => {
  const api = fake({
    latest: { "actions/checkout": "v1.2.0", "actions/setup-node": "v3.0.0", "owner/repo": "v3.0.0" },
    commits: { "actions/checkout@v1.2.0": sha("d"), "actions/setup-node@v3.0.0": sha("e"), "owner/repo@v3.0.0": sha("c") },
  });
  const r = await updatePins(parsePins(PINS), api);
  expect(r.problems).toEqual([]);
  expect(r.changes).toEqual([
    { name: "checkout", action: "actions/checkout", from: "v1.0.0", to: "v1.2.0", toSha: sha("d"), major: false },
    { name: "setup", action: "actions/setup-node", from: "v2.1.0", to: "v3.0.0", toSha: sha("e"), major: true },
  ]);
});

test("a pin is never moved to an older release", async () => {
  const api = fake({ latest: { "actions/checkout": "v0.9.0", "actions/setup-node": "v2.1.0", "owner/repo": "v3.0.0" }, commits: { "actions/checkout@v1.0.0": sha("a"), "actions/setup-node@v2.1.0": sha("b"), "owner/repo@v3.0.0": sha("c") } });
  expect((await updatePins(parsePins(PINS), api)).changes).toEqual([]);
});

test("a pin whose commit is not the commit of its recorded tag is a problem even when it is up to date", async () => {
  const api = fake({ latest: { "actions/checkout": "v1.0.0", "actions/setup-node": "v2.1.0", "owner/repo": "v3.0.0" }, commits: { "actions/checkout@v1.0.0": sha("f"), "actions/setup-node@v2.1.0": sha("b"), "owner/repo@v3.0.0": sha("c") } });
  const r = await updatePins(parsePins(PINS), api);
  expect(r.problems).toEqual([{ name: "checkout", message: `actions/checkout v1.0.0 resolves to ${sha("f")}, not the pinned ${sha("a")}` }]);
});

test("a latest tag that is not a published release is a problem and nothing is changed for it", async () => {
  const api = fake({ latest: { "actions/checkout": "v1.2.0", "actions/setup-node": "v2.1.0", "owner/repo": "v3.0.0" }, commits: { "actions/checkout@v1.2.0": sha("d"), "actions/setup-node@v2.1.0": sha("b"), "owner/repo@v3.0.0": sha("c") }, releases: ["actions/setup-node@v2.1.0", "owner/repo@v3.0.0"] });
  const r = await updatePins(parsePins(PINS), api);
  expect(r.changes).toEqual([]);
  expect(r.problems.map((p) => p.name)).toEqual(["checkout"]);
  expect(r.problems[0].message).toContain("not a published release");
});

test("a commit that does not exist in the upstream repository is a problem", async () => {
  const api = fake({ latest: { "actions/checkout": "v1.2.0", "actions/setup-node": "v2.1.0", "owner/repo": "v3.0.0" }, commits: { "actions/checkout@v1.2.0": sha("d"), "actions/setup-node@v2.1.0": sha("b"), "owner/repo@v3.0.0": sha("c") }, missing: [`actions/checkout@${sha("d")}`] });
  const r = await updatePins(parsePins(PINS), api);
  expect(r.problems[0].message).toContain("does not belong to actions/checkout");
});

test("a repository with no release, and a GitHub error, are problems and never read as unchanged", async () => {
  const r = await updatePins(parsePins(PINS), fake({ latest: { "actions/setup-node": "v2.1.0", "owner/repo": "v3.0.0" }, commits: { "actions/setup-node@v2.1.0": sha("b"), "owner/repo@v3.0.0": sha("c") }, throws: ["actions/checkout"] }));
  expect(r.problems.map((p) => p.name)).toEqual(["checkout"]);
  expect(r.problems[0].message).toContain("HTTP 504");
  const none = await updatePins(parsePins(PINS), fake({ latest: { "actions/checkout": null, "actions/setup-node": "v2.1.0", "owner/repo": "v3.0.0" }, commits: { "actions/setup-node@v2.1.0": sha("b"), "owner/repo@v3.0.0": sha("c") } }));
  expect(none.problems[0].message).toContain("no release");
});

test("applying changes rewrites only the pin lines that changed, and applying them twice changes nothing more", () => {
  const changes = [{ name: "checkout", action: "actions/checkout", from: "v1.0.0", to: "v1.2.0", toSha: sha("d"), major: false }];
  const once = applyChanges(PINS, changes);
  expect(once).toBe(PINS.replace(`sha = "${sha("a")}"; version = "v1.0.0"`, `sha = "${sha("d")}"; version = "v1.2.0"`));
  expect(applyChanges(once, changes)).toBe(once);
});

test("applying a change to a pin that is not in the file is an error", () => {
  expect(() => applyChanges(PINS, [{ name: "ghost", action: "a/b", from: "v1", to: "v2", toSha: sha("d"), major: false }])).toThrow("ghost");
});

test("the freshness record the update writes is one the freshness check accepts", () => {
  const text = renderFreshness(Date.parse("2026-11-08T12:00:00Z"));
  expect(text).toBe('{\n  "schemaVersion": 1,\n  "lastSuccessfulPinCheck": "2026-11-08T12:00:00Z"\n}\n');
  expect(parseFreshness(text).toISOString()).toBe("2026-11-08T12:00:00.000Z");
});
