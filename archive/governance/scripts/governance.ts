// Governance of the four Rust repositories (Phase 6). Read-only: every call is a GET.
// usage: bun runner/scripts/governance.ts check [repo ...]            compares the live rulesets with governance/desired/ (exit 4 on drift)
//        bun runner/scripts/governance.ts show <repo> <ruleset name>   prints one live ruleset in the create shape (how a desired file is made)
//        bun runner/scripts/governance.ts settings [repo ...]          dry run of the merge settings task: what would change, nothing is sent
//        bun runner/scripts/governance.ts settings <repo> --apply --confirm <owner>/<repo> [--record-dir <dir>]
//        bun runner/scripts/governance.ts settings-restore <record file> [--apply --confirm <owner>/<repo>]
//        bun runner/scripts/governance.ts rulesets [repo ...] [--show-body]   dry run of the ruleset task: what would be created or replaced, nothing is sent
//        bun runner/scripts/governance.ts rulesets <repo> --apply --confirm <owner>/<repo> [--record-dir <dir>]
//        bun runner/scripts/governance.ts rulesets-restore <record file> [--apply --confirm <owner>/<repo>]
//        bun runner/scripts/governance.ts drift [--issue]              the scheduled check: compares the rulesets; with --issue it opens, updates or closes the drift issue
// option: --desired-dir <dir> reads the desired state from another folder (a dry run of a change before it is committed)
// env:   GH_TOKEN (any token that can read the repositories: the rulesets of a public repository are public)
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { realGh } from "../src/governance/gh";
import { ISSUE_LABEL, reconcileIssue, renderIssue, type Issues } from "../src/governance/drift";
import { applyRulesets, dryRunRulesets, restoreRulesets, type RulesetPlan } from "../src/governance/rulesetTask";
import { checkRepo, type Api, type DesiredRepos } from "../src/governance/live";
import { applySettings, dryRunSettings, parseDesiredSettings, restoreSettings, type SettingsPlan } from "../src/governance/settings";
import { toCreateShape, type Ruleset } from "../src/governance/ruleset";

const root = join(import.meta.dir, "../..");
const desiredIdx = Bun.argv.indexOf("--desired-dir");
const desiredDir = desiredIdx < 0 ? join(root, "governance/desired") : resolve(Bun.argv[desiredIdx + 1]);

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
const positional = Bun.argv.slice(3).filter((a, i, all) => !a.startsWith("--") && !["--confirm", "--record-dir", "--desired-dir"].includes(all[i - 1] ?? ""));
const describe = (p: SettingsPlan): string =>
  p.changes.length
    ? `CHANGE ${p.repo}\n${p.changes.map((c) => `  ${c.key}: ${c.from} -> ${c.to}`).join("\n")}\n  would send: ${p.request!.method} ${p.request!.path} ${JSON.stringify(p.request!.body)}`
    : `ok     ${p.repo}: the merge settings already match`;

const showBody = Bun.argv.includes("--show-body");
const describeRulesets = (p: RulesetPlan): string => {
  const lines = p.actions.map((a) => {
    if (a.kind === "unchanged") return `  unchanged ${JSON.stringify(a.name)}`;
    if (a.kind === "untouched") return `  untouched ${JSON.stringify(a.name)}: ${a.reason}`;
    if (a.kind === "delete") return `  delete    ${JSON.stringify(a.name)}\n    would send: ${a.request.method} ${a.request.path}`;
    const what = a.kind === "create" ? `  create    ${JSON.stringify(a.name)}` : `  replace   ${JSON.stringify(a.name)} (id ${a.id})\n${a.differences.map((d) => `      ${d}`).join("\n")}`;
    return `${what}\n    would send: ${a.request.method} ${a.request.path}${showBody ? ` ${JSON.stringify(a.request.body)}` : ""}`;
  });
  const pending = p.actions.some((a) => a.kind === "create" || a.kind === "update" || a.kind === "delete");
  return `${pending ? "CHANGE" : "ok    "} ${p.repo}\n${lines.join("\n")}`;
};

/** The drift issue, through the gh CLI, in this repository. The label is created when it is missing. */
function realIssues(): Issues {
  const repo = process.env.GH_REPO ?? process.env.GITHUB_REPOSITORY ?? "pantheon-org/pantheon-scaffold";
  const run = async (args: string[], input?: string): Promise<string> => {
    const proc = Bun.spawn(["gh", ...args, "--repo", repo], { stdin: input === undefined ? "ignore" : new Blob([input]), stdout: "pipe", stderr: "pipe" });
    const [out, err, code] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
    if (code !== 0) throw new Error(`gh ${args.slice(0, 2).join(" ")} failed (exit ${code}): ${err.trim()}`);
    return out.trim();
  };
  const assignee = (JSON.parse(readFileSync(join(root, "governance/drift-owner.json"), "utf8")) as { login: string }).login;
  return {
    list: async () => JSON.parse((await run(["issue", "list", "--label", ISSUE_LABEL, "--state", "open", "--limit", "20", "--json", "number,title,body"])) || "[]"),
    create: async (title, body) => {
      await run(["label", "create", ISSUE_LABEL, "--description", "Live rulesets differ from governance/desired", "--color", "d93f0b", "--force"]);
      const url = await run(["issue", "create", "--title", title, "--label", ISSUE_LABEL, "--assignee", assignee, "--body-file", "-"], body);
      return Number(url.split("/").pop());
    },
    update: async (n, body) => void (await run(["issue", "edit", String(n), "--body-file", "-"], body)),
    comment: async (n, text) => void (await run(["issue", "comment", String(n), "--body-file", "-"], text)),
    close: async (n) => void (await run(["issue", "close", String(n)])),
  };
}

const [cmd, ...rest] = Bun.argv.slice(2);
try {
  if (cmd === "check") {
    const repos = rest.length ? rest : Object.keys(desired.repos);
    for (const r of repos) if (!desired.repos[r]) throw new Error(`${r} is not one of the governed repositories (${Object.keys(desired.repos).join(", ")})`);
    let drift = 0;
    for (const repo of repos) {
      const f = await checkRepo(desired.owner, repo, desired.repos[repo].map(loadRuleset), api);
      console.log(f.problems.length ? `DRIFT ${repo}\n${f.problems.map((p) => `  ${p}`).join("\n")}` : `ok    ${repo}: ${desired.repos[repo].length} ruleset(s) match`);
      for (const n of f.notes) console.log(`  note: ${n}`);
      drift += f.problems.length;
    }
    console.log(drift ? `${drift} difference(s).` : `no drift in ${repos.length} repositories.`);
    process.exit(drift ? 4 : 0);
  } else if (cmd === "drift") {
    const repos = Object.keys(desired.repos);
    const findings = [];
    for (const repo of repos) findings.push(await checkRepo(desired.owner, repo, desired.repos[repo].map(loadRuleset), api));
    for (const f of findings) console.log(f.problems.length ? `DRIFT ${f.repo}\n${f.problems.map((p) => `  ${p}`).join("\n")}` : `ok    ${f.repo}`);
    for (const n of new Set(findings.flatMap((f) => f.notes))) console.log(`note: ${n}`);
    const assignee = (JSON.parse(readFileSync(join(root, "governance/drift-owner.json"), "utf8")) as { login: string }).login;
    const drifted = findings.some((f) => f.problems.length);
    if (Bun.argv.includes("--issue")) {
      const runUrl = process.env.GITHUB_SERVER_URL && process.env.GITHUB_RUN_ID ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}` : undefined;
      const r = await reconcileIssue(realIssues(), findings, assignee, runUrl, false);
      console.log(`issue: ${r.outcome}${r.number ? ` #${r.number}` : ""}`);
      process.exit(0);
    }
    if (drifted) console.log(`\nNo issue was opened (run with --issue to do that). It would read:\n\n# ${renderIssue(findings, assignee).title}\n\n${renderIssue(findings, assignee).body}`);
    else console.log("no drift.");
    process.exit(drifted ? 4 : 0);
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
  } else if (cmd === "rulesets") {
    const governed = Object.keys(desired.repos);
    const repos = positional.length ? positional : governed;
    if (apply) {
      if (repos.length !== 1) throw new Error("apply works on one repository at a time: name it");
      const r = await applyRulesets(realGh(), desired.owner, repos[0], desired.repos[repos[0]].map(loadRuleset), { governed, recordDir, confirm: flag("--confirm") });
      console.log(r.applied ? `${describeRulesets(r.plan)}\napplied. Restore with: rulesets-restore ${r.recordFile}` : describeRulesets(r.plan));
    } else {
      let n = 0;
      for (const repo of repos) {
        if (!desired.repos[repo]) throw new Error(`${repo} is not one of the governed repositories (${governed.join(", ")})`);
        const plan = await dryRunRulesets(realGh(), governed, desired.owner, repo, desired.repos[repo].map(loadRuleset));
        console.log(describeRulesets(plan));
        n += plan.actions.filter((a) => a.kind === "create" || a.kind === "update" || a.kind === "delete").length;
      }
      console.log(`dry run: ${n} request(s) would be sent for ${repos.length} repositories. Nothing was sent.`);
    }
  } else if (cmd === "rulesets-restore" && positional.length === 1) {
    const r = await restoreRulesets(realGh(), positional[0], { governed: Object.keys(desired.repos), apply, confirm: flag("--confirm") });
    console.log(describeRulesets(r.plan));
    console.log(r.restored ? "restored." : "dry run: nothing was sent.");
  } else if (cmd === "show" && rest.length === 2) {
    const [repo, name] = rest;
    if (!desired.repos[repo]) throw new Error(`${repo} is not one of the governed repositories`);
    const list = (await api(`repos/${desired.owner}/${repo}/rulesets`)) as { id: number; name: string }[];
    const hit = list.filter((x) => x.name === name);
    if (hit.length !== 1) throw new Error(`${repo} has ${hit.length} rulesets named ${JSON.stringify(name)}`);
    console.log(JSON.stringify(toCreateShape((await api(`repos/${desired.owner}/${repo}/rulesets/${hit[0].id}`)) as Record<string, unknown>), null, 2));
  } else {
    console.error("usage: governance.ts check [repo ...] | show <repo> <ruleset name> | settings [repo ...] | settings-restore <record> | rulesets [repo ...] [--apply --confirm <owner>/<repo>] | rulesets-restore <record>");
    process.exit(2);
  }
} catch (e) {
  console.error((e as Error).message);
  process.exit(1);
}
