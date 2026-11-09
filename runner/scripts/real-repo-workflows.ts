// Phase 4 wave D: render every workflow builtin into an empty folder, once per real repository, and compare each rendered file with that
// repository's own copy. Read-only: the repositories are cloned (https, depth 1) into a temporary folder and nothing is written to them or pushed.
// A workflow or YAML config is compared under the P.1 rule (parsed data, `run:` byte for byte) and its directive comments are checked;
// any other file is compared byte for byte. Rendered workflows must also pass actionlint (found through mise).
// usage: bun runner/scripts/real-repo-workflows.ts
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { compareDirectives, compareWorkflows } from "../src/equality";

const root = resolve(import.meta.dir, "../..");
const cli = join(root, "runner/src/cli.ts");
const REPOS = ["pantheon-adr", "pantheon-journal", "pantheon-skill-auditor", "pantheon-agentic-harness-dev-kit"];
// Differences the plan records and accepts, by repository and path. Anything else that differs fails the run.
const DIST_NOTE = "two action commits (upload-artifact v7.0.2, download-artifact v8.0.2) are the ones the repository's release.yml uses today, so that dist generate --check passes (decision 85)";
const DEPENDABOT_NOTE = "the github-actions entry also ignores actions/upload-artifact and actions/download-artifact, which only dist generate may change in release.yml (decision 86)";
const RECORDED: Record<string, Record<string, string>> = {
  "pantheon-adr": { "dist-workspace.toml": DIST_NOTE, ".github/dependabot.yml": DEPENDABOT_NOTE },
  "pantheon-journal": { "dist-workspace.toml": DIST_NOTE, ".github/dependabot.yml": DEPENDABOT_NOTE },
  "pantheon-agentic-harness-dev-kit": { ".gitignore": "decision 74 picks target/, the repository has /target" },
  "pantheon-skill-auditor": { "SECURITY.md": "a seeded file: the repository words its scope bullets differently", "dist-workspace.toml": DIST_NOTE, ".github/dependabot.yml": DEPENDABOT_NOTE },
};
// Files the package adds that no repository has yet: a new workflow, expected to be absent.
const NEW = [".github/workflows/dist-check.yml", ".github/workflows/scaffold-check.yml"];
// Listed by the package but never written: dist generate writes it, and the drift check of phase 5 wave D compares it.
const EXTERNAL = [".github/workflows/release.yml"];

type Run = { out: string; err: string; code: number };
const sh = async (cmd: string[], cwd: string): Promise<Run> => {
  const p = Bun.spawn(cmd, { cwd, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};
const walk = (dir: string, base = dir): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name), base) : [join(dir, e.name).slice(base.length + 1)]));
const isYaml = (p: string) => /\.ya?ml$/.test(p);
// A pin that moved since the repository last updated it is a pin difference, listed in the report, and not a difference in content.
const unpin = (text: string): string => text.replace(/@[0-9a-f]{40}\b/g, "@<pin>");
const pinsOf = (text: string): string[] => [...text.matchAll(/uses:\s*([^@\s]+)@([0-9a-f]{40})/g)].map((m) => `${m[1]}@${m[2].slice(0, 7)}`);
const comments = (t: string) => t.split("\n").filter((l) => /^\s*#/.test(l)).map((l) => l.trim());

const actionlint = (await sh(["mise", "where", "actionlint"], root)).out;
const actionlintBin = actionlint ? (await sh(["find", actionlint, "-maxdepth", "3", "-name", "actionlint", "-type", "f"], root)).out.split("\n")[0] : "";
if (!actionlintBin) throw new Error("actionlint is not installed (mise where actionlint found nothing)");

const work = mkdtempSync(join(tmpdir(), "real-repo-workflows-"));
const lines: string[] = [];
const say = (s = "") => lines.push(s);
say("# Real-repository render of the workflow layer");
say();
say("Each repository is cloned (https, depth 1) into a temporary folder. The builtins are rendered into an empty folder and compared with the repository's own files. Nothing is written to the repositories.");
say();

let allPass = true;
const summary: string[] = [];
for (const name of REPOS) {
  const repo = join(work, name);
  const clone = await sh(["git", "clone", "--quiet", "--depth", "1", `https://github.com/pantheon-org/${name}.git`, repo], work);
  if (clone.code !== 0) throw new Error(`clone of ${name} failed: ${clone.err}`);
  const head = (await sh(["git", "rev-parse", "--short", "HEAD"], repo)).out;
  const out = join(work, `${name}-rendered`);
  mkdirSync(out);
  const fixture = join(root, "runner/test/fixtures", `real-workflows-${name}`, "pantheon.pkl");
  const rendered = await sh(["bun", cli, "render", fixture, "--root", out, "--allow-local-package", join(root, "pkl")], root);
  if (rendered.code !== 0) throw new Error(`render for ${name} failed: ${rendered.err}`);

  const built = walk(out).filter((p) => !p.startsWith(".pkl-gen/")).sort();
  const results: { path: string; kind: string; detail: string[] }[] = [];
  for (const path of built) {
    const mine = readFileSync(join(out, path), "utf8");
    const theirsPath = join(repo, path);
    if (!existsSync(theirsPath)) { results.push({ path, kind: NEW.includes(path) ? "new" : "ABSENT", detail: [NEW.includes(path) ? "a file the package adds: the repository has none yet" : "the repository has no such file"] }); continue; }
    const recorded = RECORDED[name]?.[path];
    const theirs = readFileSync(theirsPath, "utf8");
    if (mine === theirs) { results.push({ path, kind: "identical", detail: [] }); continue; }
    if (isYaml(path)) {
      const c = compareWorkflows(unpin(theirs), unpin(mine));
      const d = compareDirectives(theirs, mine);
      if (!c.equal || c.duplicateKeys.length || d.length) { results.push({ path, kind: recorded ? "recorded" : "DIFFERS", detail: [...(recorded ? [recorded] : []), ...c.differences, ...c.duplicateKeys.map((k) => `duplicate key ${k}`), ...d.map((x) => `directive: ${x}`)] }); continue; }
      const a = comments(theirs), b = comments(mine);
      const moved = pinsOf(mine).filter((p) => !pinsOf(theirs).includes(p));
      const lost = a.filter((l) => !b.includes(l)).length, added = b.filter((l) => !a.includes(l)).length;
      results.push({ path, kind: "equal (P.1)", detail: [...(moved.length ? [`pin moved since the repository's: ${moved.join(", ")}`] : []), ...(lost || added ? [`comments only: ${lost} line(s) of the repository's not rendered, ${added} rendered that it lacks`] : moved.length ? [] : ["formatting only"])] });
    } else {
      results.push({ path, kind: recorded ? "recorded" : "DIFFERS", detail: [recorded ?? "bytes differ"] });
    }
  }
  const wfBuilt = new Set(built.filter((p) => p.startsWith(".github/workflows/")));
  const notBuilt = [...(existsSync(join(repo, ".github")) ? walk(join(repo, ".github")).map((p) => `.github/${p}`) : [])].filter((p) => !built.includes(p) && !EXTERNAL.includes(p) && (p.startsWith(".github/workflows/") || /^\.github\/[^/]+\.(ya?ml|txt|md)$/.test(p))).sort();
  const lint = await sh([actionlintBin, ...[...wfBuilt].sort().map((p) => join(out, p))], out);

  const bad = results.filter((r) => r.kind === "DIFFERS" || r.kind === "ABSENT");
  const pass = bad.length === 0 && lint.code === 0;
  if (!pass) allPass = false;
  summary.push(`| ${name} | ${head} | ${built.length} | ${results.filter((r) => r.kind === "identical").length} | ${results.filter((r) => r.kind === "equal (P.1)").length} | ${results.filter((r) => r.kind === "recorded").length} | ${results.filter((r) => r.kind === "new").length} | ${bad.length} | ${lint.code === 0 ? "clean" : "FAIL"} | ${pass ? "PASS" : "FAIL"} |`);

  say(`## ${name} at ${head}`);
  say();
  say("| File | Result | Detail |");
  say("|---|---|---|");
  for (const r of results) say(`| ${r.path} | ${r.kind} | ${r.detail.join("; ").replace(/\|/g, "\\|") || "none"} |`);
  say();
  say(`- actionlint over the ${wfBuilt.size} rendered workflows: exit ${lint.code}${lint.code ? `\n${lint.out}\n${lint.err}` : ""}.`);
  say(`- Files the repository has that no builtin builds yet (${notBuilt.length}): ${notBuilt.join(", ") || "none"}.`);
  say();
}

say("## Summary");
say();
say("| Repository | Commit | Files built | Identical | Equal under P.1 | Recorded | New | Differ or absent | actionlint | Result |");
say("|---|---|---|---|---|---|---|---|---|---|");
for (const s of summary) say(s);
say();
say(`Verdict: ${allPass ? "PASS" : "FAIL"}`);
console.log(lines.join("\n"));
process.exit(allPass ? 0 : 1);
