// Governance of the four Rust repositories (Phase 6). Read-only: every call is a GET.
// usage: bun runner/scripts/governance.ts check [repo ...]            compares the live rulesets with governance/desired/ (exit 4 on drift)
//        bun runner/scripts/governance.ts show <repo> <ruleset name>   prints one live ruleset in the create shape (how a desired file is made)
//        bun runner/scripts/governance.ts settings [repo ...]          dry run of the merge settings task: what would change, nothing is sent
//        bun runner/scripts/governance.ts settings <repo> --apply --confirm <owner>/<repo> [--record-dir <dir>]
//        bun runner/scripts/governance.ts settings-restore <record file> [--apply --confirm <owner>/<repo>]
// env:   GH_TOKEN (any token that can read the repositories: the rulesets of a public repository are public)
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { realGh } from "../src/governance/gh";
import { checkRepo, type Api, type DesiredRepos } from "../src/governance/live";
import { applySettings, dryRunSettings, parseDesiredSettings, restoreSettings, type SettingsPlan } from "../src/governance/settings";
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

const flag = (name: string): string | undefined => {
  const i = Bun.argv.indexOf(name);
  return i < 0 ? undefined : Bun.argv[i + 1];
};
const apply = Bun.argv.includes("--apply");
const recordDir = flag("--record-dir") ?? join(root, ".context/governance");
const positional = Bun.argv.slice(3).filter((a, i, all) => !a.startsWith("--") && !["--confirm", "--record-dir"].includes(all[i - 1] ?? ""));
const describe = (p: SettingsPlan): string =>
  p.changes.length
    ? `CHANGE ${p.repo}\n${p.changes.map((c) => `  ${c.key}: ${c.from} -> ${c.to}`).join("\n")}\n  would send: ${p.request!.method} ${p.request!.path} ${JSON.stringify(p.request!.body)}`
    : `ok     ${p.repo}: the merge settings already match`;

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
  } else if (cmd === "settings") {
    const desiredSettings = parseDesiredSettings(readFileSync(join(desiredDir, "settings.json"), "utf8"));
    const governed = Object.keys(desired.repos);
    const repos = positional.length ? positional : governed;
    if (apply) {
      if (repos.length !== 1) throw new Error("apply works on one repository at a time: name it");
      const r = await applySettings(realGh(), desired.owner, repos[0], desiredSettings, { governed, recordDir, confirm: flag("--confirm") });
      console.log(r.applied ? `${describe(r.plan)}\napplied. Restore with: settings-restore ${r.recordFile}` : describe(r.plan));
    } else {
      let n = 0;
      for (const repo of repos) {
        const plan = await dryRunSettings(realGh(), governed, desired.owner, repo, desiredSettings);
        console.log(describe(plan));
        n += plan.changes.length;
      }
      console.log(`dry run: ${n} change(s) would be made in ${repos.length} repositories. Nothing was sent.`);
    }
  } else if (cmd === "settings-restore" && positional.length === 1) {
    const r = await restoreSettings(realGh(), positional[0], { governed: Object.keys(desired.repos), apply, confirm: flag("--confirm") });
    console.log(describe(r.plan));
    console.log(r.restored ? "restored." : "dry run: nothing was sent.");
  } else if (cmd === "show" && rest.length === 2) {
    const [repo, name] = rest;
    if (!desired.repos[repo]) throw new Error(`${repo} is not one of the governed repositories`);
    const list = (await api(`repos/${desired.owner}/${repo}/rulesets`)) as { id: number; name: string }[];
    const hit = list.filter((x) => x.name === name);
    if (hit.length !== 1) throw new Error(`${repo} has ${hit.length} rulesets named ${JSON.stringify(name)}`);
    console.log(JSON.stringify(toCreateShape((await api(`repos/${desired.owner}/${repo}/rulesets/${hit[0].id}`)) as Record<string, unknown>), null, 2));
  } else {
    console.error("usage: governance.ts check [repo ...] | show <repo> <ruleset name> | settings [repo ...] [--apply --confirm <owner>/<repo>] | settings-restore <record> [--apply --confirm <owner>/<repo>]");
    process.exit(2);
  }
} catch (e) {
  console.error((e as Error).message);
  process.exit(1);
}
