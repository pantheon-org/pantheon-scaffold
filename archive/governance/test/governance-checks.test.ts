import { afterEach, expect, test } from "bun:test";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { checkNames, compareRequired } from "../src/governance/checks";
import { requiredChecks, toCreateShape, type Ruleset } from "../src/governance/ruleset";
import { evalContractJson } from "../src/pkl";
import { cleanupRepos, tempRepo } from "./helpers";

afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");
const desiredFile = (stem: string): Ruleset => JSON.parse(readFileSync(join(root, "governance/desired", `${stem}.json`), "utf8"));

test("a check is named by the job name when it is set, otherwise by the job id", () => {
  expect(checkNames("jobs:\n  build:\n    name: Build, test, lint\n    runs-on: x\n  zizmor:\n    runs-on: x\n")).toEqual(["Build, test, lint", "zizmor"]);
});

test("a matrix job is named with its values, or with the matrix filled into its name", () => {
  const wf = (name: string) => `jobs:\n  t:\n${name}    strategy:\n      matrix:\n        os: [linux, mac]\n        v: [1, 2]\n    runs-on: x\n`;
  expect(checkNames(wf(""))).toEqual(["t (linux, 1)", "t (linux, 2)", "t (mac, 1)", "t (mac, 2)"]);
  expect(checkNames(wf("    name: Test on ${{ matrix.os }}\n"))).toEqual(["Test on linux", "Test on linux", "Test on mac", "Test on mac"]);
});

test("what cannot be known is an error, so a check name is never guessed", () => {
  expect(() => checkNames("jobs:\n  c:\n    uses: ./.github/workflows/x.yml\n")).toThrow("reusable workflow");
  expect(() => checkNames("jobs:\n  c:\n    name: ${{ github.ref_name }}\n    runs-on: x\n")).toThrow("not known without running");
  expect(() => checkNames("jobs:\n  c:\n    strategy:\n      matrix:\n        include: [{a: 1}]\n    runs-on: x\n")).toThrow("include or exclude");
  expect(() => checkNames("jobs:\n  c:\n    strategy:\n      matrix:\n        a: ${{ fromJSON(x) }}\n    runs-on: x\n")).toThrow("not a plain list");
});

test("required checks that no job produces, and jobs that nothing requires, are told apart", () => {
  expect(compareRequired(["a", "b", "a"], ["b", "c"])).toEqual({ missing: ["a"], unrequired: ["c"] });
  expect(compareRequired([], [])).toEqual({ missing: [], unrequired: [] });
});

// The four repositories as the package renders them (the options of the wave A proof).
type Options = { crate?: string; skill?: string; skills: boolean; binary: boolean; composite: boolean; features: "default" | "all" };
const REPOS: Record<string, { options: Options; rulesets: string[] }> = {
  "pantheon-adr": { options: { crate: "adr", skill: "adr-creator", skills: true, binary: true, composite: false, features: "default" }, rulesets: ["protect-main"] },
  "pantheon-journal": { options: { crate: "journal", skill: "journal-entry-creator", skills: true, binary: true, composite: false, features: "default" }, rulesets: ["protect-main", "require-cargo-deny"] },
  "pantheon-skill-auditor": { options: { crate: "skill-auditor", skill: "skill-quality-auditor", skills: true, binary: true, composite: true, features: "default" }, rulesets: ["protect-main", "require-cargo-deny"] },
  "pantheon-agentic-harness-dev-kit": { options: { skills: false, binary: false, composite: false, features: "all" }, rulesets: ["protect-main", "require-cargo-deny"] },
};

let n = 0;
async function renderedWorkflows(o: Options): Promise<Map<string, string>> {
  const f = join(tempRepo(), `pantheon-${n++}.pkl`);
  const names = [o.crate ? `crateName = "${o.crate}"` : "", o.skill ? `skillName = "${o.skill}"` : ""].filter(Boolean).join("\n  ");
  writeFileSync(
    f,
    `amends "file://${root}/pkl/Config.pkl"\nimport "file://${root}/pkl/Builtins.pkl"\noptions {\n  toolchain = "1.96.1"\n  ${names}\n  skills = ${o.skills}\n  binary = ${o.binary}\n  compositeAction = ${o.composite}\n  features = "${o.features}"\n}\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles { ...Builtins.standardFiles(options) }\n`,
  );
  const c = (await evalContractJson(f, root, { allowFilesUnder: [join(root, "pkl")] })) as { files: { path: string; content?: string }[] };
  return new Map(c.files.filter((x) => /^\.github\/workflows\/[^/]+\.ya?ml$/.test(x.path) && x.content !== undefined).map((x) => [x.path, x.content!]));
}
const offeredBy = (wfs: Map<string, string>): string[] => [...wfs.values()].flatMap(checkNames);
const requiredFor = (rulesets: string[]): string[] => rulesets.flatMap((s) => requiredChecks(toCreateShape({ ...desiredFile(s) })));

test("every required check of every Rust repository is a job name that the package renders for it", async () => {
  for (const [repo, spec] of Object.entries(REPOS)) {
    const { missing } = compareRequired(requiredFor(spec.rulesets), offeredBy(await renderedWorkflows(spec.options)));
    expect(missing, `${repo}: required checks no rendered job produces`).toEqual([]);
  }
});

test("renaming a job that is a required check is reported as a missing required check (the transition test)", async () => {
  const spec = REPOS["pantheon-journal"];
  const wfs = await renderedWorkflows(spec.options);
  const file = [...wfs.entries()].find(([, text]) => checkNames(text).includes("Cargo Deny"))!;
  expect(file[0]).toBe(".github/workflows/cargo-deny.yml");
  wfs.set(file[0], file[1].replace(/^(\s{4}name: )Cargo Deny$/m, "$1Cargo Deny (renamed)"));
  const after = compareRequired(requiredFor(spec.rulesets), offeredBy(wfs));
  expect(after.missing).toEqual(["Cargo Deny"]);
  expect(after.unrequired).toContain("Cargo Deny (renamed)");
});

test("a new job that nothing requires is reported as unrequired, and is not a failure", async () => {
  const spec = REPOS["pantheon-adr"];
  const wfs = await renderedWorkflows(spec.options);
  wfs.set(".github/workflows/new.yml", "jobs:\n  fresh:\n    name: Fresh check\n    runs-on: x\n");
  const after = compareRequired(requiredFor(spec.rulesets), offeredBy(wfs));
  expect(after.missing).toEqual([]);
  expect(after.unrequired).toContain("Fresh check");
});

test("the jobs of the standard set that are not required are the ones a repository has chosen not to require yet", async () => {
  const { unrequired } = compareRequired(requiredFor(REPOS["pantheon-journal"].rulesets), offeredBy(await renderedWorkflows(REPOS["pantheon-journal"].options)));
  // The new checks of the package are among them: requiring them is a decision for the owner, once a repository has adopted the package.
  expect(unrequired).toContain("Scaffold check");
});
