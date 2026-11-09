// Governance of the four Rust repositories (Phase 6). Read-only: every call is a GET.
// usage: bun runner/scripts/governance.ts check [repo ...]            compares the live rulesets with governance/desired/ (exit 4 on drift)
//        bun runner/scripts/governance.ts show <repo> <ruleset name>   prints one live ruleset in the create shape (how a desired file is made)
// env:   GH_TOKEN (any token that can read the repositories: the rulesets of a public repository are public)
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { checkRepo, type Api, type DesiredRepos } from "../src/governance/live";
import { toCreateShape, type Ruleset } from "../src/governance/ruleset";

const root = join(import.meta.dir, "../..");
const desiredDir = join(root, "governance/desired");

const api: Api = async (path) => {
  const p = Bun.spawn(["gh", "api", path], { stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  if (code !== 0) throw new Error(`gh api ${path} failed (exit ${code}): ${err.trim()}`);
  return JSON.parse(out);
};

const desired = JSON.parse(readFileSync(join(desiredDir, "repos.json"), "utf8")) as DesiredRepos;
const loadRuleset = (stem: string): Ruleset => toCreateShape(JSON.parse(readFileSync(join(desiredDir, `${stem}.json`), "utf8")));

const [cmd, ...rest] = Bun.argv.slice(2);
try {
  if (cmd === "check") {
    const repos = rest.length ? rest : Object.keys(desired.repos);
    for (const r of repos) if (!desired.repos[r]) throw new Error(`${r} is not one of the governed repositories (${Object.keys(desired.repos).join(", ")})`);
    let drift = 0;
    for (const repo of repos) {
      const f = await checkRepo(desired.owner, repo, desired.repos[repo].map(loadRuleset), api);
      console.log(f.problems.length ? `DRIFT ${repo}\n${f.problems.map((p) => `  ${p}`).join("\n")}` : `ok    ${repo}: ${desired.repos[repo].length} ruleset(s) match`);
      drift += f.problems.length;
    }
    console.log(drift ? `${drift} difference(s).` : `no drift in ${repos.length} repositories.`);
    process.exit(drift ? 4 : 0);
  } else if (cmd === "show" && rest.length === 2) {
    const [repo, name] = rest;
    if (!desired.repos[repo]) throw new Error(`${repo} is not one of the governed repositories`);
    const list = (await api(`repos/${desired.owner}/${repo}/rulesets`)) as { id: number; name: string }[];
    const hit = list.filter((x) => x.name === name);
    if (hit.length !== 1) throw new Error(`${repo} has ${hit.length} rulesets named ${JSON.stringify(name)}`);
    console.log(JSON.stringify(toCreateShape((await api(`repos/${desired.owner}/${repo}/rulesets/${hit[0].id}`)) as Record<string, unknown>), null, 2));
  } else {
    console.error("usage: governance.ts check [repo ...] | show <repo> <ruleset name>");
    process.exit(2);
  }
} catch (e) {
  console.error((e as Error).message);
  process.exit(1);
}
