// Phase 7 wave A (tasks 7.1 and 7.2): render the PUBLISHED package into scratch clones of the four repositories with the PUBLISHED runner, and
// judge the result. Read-only: the repositories are cloned (https, depth 1) into a temporary folder and nothing is written to them or pushed.
// It downloads the runner of the release and the checksums, verifies the attestation and the digest first, loads the package through its real
// package:// URI with the published checksum, categorises every difference (proof-categories.ts, zero may be uncategorised), compares the
// required status checks of each repository's rulesets (read-only GET) with the job names of the rendered workflows, and runs `hk check --all`
// before and after, as real-repo-tooling.ts does.
// usage: bun runner/scripts/proof-render.ts [version]   (default: the version in pkl/PklProject)
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createHash } from "node:crypto";
import { categorise } from "./proof-categories";

const root = resolve(import.meta.dir, "../..");
const REPO = "pantheon-org/pantheon-scaffold";
const version = Bun.argv[2] ?? /version = "([^"]+)"/.exec(readFileSync(join(root, "pkl/PklProject"), "utf8"))![1];
const asset = `pantheon-scaffold-${process.platform === "darwin" ? "darwin" : "linux"}-${process.arch === "arm64" ? "arm64" : "x64"}`;

type Run = { out: string; err: string; code: number };
const sh = async (cmd: string[], cwd: string, env?: Record<string, string>): Promise<Run> => {
  const p = Bun.spawn(cmd, { cwd, stdout: "pipe", stderr: "pipe", ...(env ? { env } : {}) });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};
/** The environment for gh: an invalid ambient GH_TOKEN would shadow the keyring login, so it is dropped. */
const ghEnv = (): Record<string, string> =>
  Object.fromEntries(Object.entries(process.env).filter(([k, v]) => k !== "GH_TOKEN" && v !== undefined)) as Record<string, string>;
const strip = (s: string): string => s.replace(/\x1b\[[0-9;]*m/g, "");
const need = (r: Run, what: string): Run => {
  if (r.code !== 0) throw new Error(`${what} failed (exit ${r.code}): ${r.err || r.out}`);
  return r;
};

/** The real executable of an installed tool, found without a mise shim, so a repository's own mise.toml cannot change which one runs. */
async function installed(tool: string, name = tool.split("@")[0]): Promise<string> {
  const where = (await sh(["mise", "where", tool], root)).out;
  const found = where ? (await sh(["find", where, "-maxdepth", "3", "-name", name, "-type", "f", "-perm", "-u+x"], root)).out.split("\n")[0] : "";
  if (!found) throw new Error(`${tool} is not installed (mise where ${tool} found nothing)`);
  return found;
}

const work = mkdtempSync(join(tmpdir(), "proof-render-"));
const lines: string[] = [];
const say = (s = "") => lines.push(s);

// 1. The release: the runner and the checksums, verified before anything is run.
const rel = join(work, "release");
mkdirSync(rel);
need(await sh(["gh", "release", "download", `v${version}`, "--repo", REPO, "-p", "checksums.txt", "-p", asset, "-D", rel], work, ghEnv()), "gh release download");
need(await sh(["gh", "attestation", "verify", join(rel, "checksums.txt"), "--repo", REPO], work, ghEnv()), "attestation of checksums.txt");
need(await sh(["gh", "attestation", "verify", join(rel, asset), "--repo", REPO], work, ghEnv()), `attestation of ${asset}`);
const sums = new Map(
  readFileSync(join(rel, "checksums.txt"), "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => {
      const [h, n] = l.trim().split(/\s+/);
      return [n, h] as const;
    }),
);
const runnerSum = createHash("sha256").update(readFileSync(join(rel, asset))).digest("hex");
if (sums.get(asset) !== runnerSum) throw new Error(`the downloaded ${asset} does not match checksums.txt`);
const pkgSum = sums.get(`pantheon-scaffold@${version}`);
if (!pkgSum) throw new Error(`checksums.txt has no entry for pantheon-scaffold@${version}`);
chmodSync(join(rel, asset), 0o755);
const runner = join(rel, asset);
const reported = (await sh([runner, "--version"], work)).out;
if (reported !== version) throw new Error(`the runner reports ${reported}, expected ${version}`);

// 2. The tools hk needs, found through mise, and a PATH that holds nothing else.
const bin = join(work, "bin");
mkdirSync(bin);
const cargo = (await sh(["rustup", "which", "cargo", "--toolchain", "1.96.1"], root)).out;
if (!cargo) throw new Error("the Rust toolchain 1.96.1 the repositories pin is not installed");
const tools: Record<string, string> = {
  rumdl: await installed("rumdl@0.2.77"),
  shellcheck: await installed("shellcheck"),
  yq: await installed("yq"),
  actionlint: await installed("actionlint"),
  cargo,
  "cargo-fmt": join(dirname(cargo), "cargo-fmt"),
  rustfmt: join(dirname(cargo), "rustfmt"),
};
for (const [name, p] of Object.entries(tools)) {
  if (!existsSync(p)) throw new Error(`${name} not found at ${p}`);
  symlinkSync(p, join(bin, name));
}
const hkBin = dirname(await installed("hk@2.10.0"));
const pklBin = dirname(await installed("pkl@0.32.2"));
const hkEnv: Record<string, string> = { HOME: process.env.HOME ?? "", PATH: `${bin}:${hkBin}:${pklBin}:/usr/bin:/bin`, HK_MISE: "0", NO_COLOR: "1" };
const runEnv: Record<string, string> = { HOME: process.env.HOME ?? "", PATH: `${pklBin}:/usr/bin:/bin`, NO_COLOR: "1" };

async function hkCheck(repo: string): Promise<{ code: number; steps: string[]; failures: string[] }> {
  const r = await sh(["hk", "check", "--all", "--no-fail-fast"], repo, hkEnv);
  const steps = new Set<string>();
  for (const line of strip(r.out + "\n" + r.err).split("\n")) {
    const m = /^(✔|✗|⚠) ([a-z][a-z-]*)\b/.exec(line.trim());
    if (m && m[2] !== "files") steps.add(`${m[1]} ${m[2]}`);
  }
  const failures = strip(r.out + "\n" + r.err)
    .split("\n")
    .filter((l) => /^\s+(rumdl|rustfmt|shellcheck|actionlint|yamllint) – /.test(l) && !/ – \d+ files? – /.test(l))
    .map((l) => l.trim().slice(0, 160));
  return { code: r.code, steps: [...steps].sort(), failures };
}

// 3. The four repositories and how each is set up (the options of the real-workflows fixtures, now with the package loaded by its URI).
const uri = `package://github.com/${REPO}/releases/download/v${version}/pantheon-scaffold@${version}::sha256:${pkgSum}`;
const REPOS: { name: string; options: string }[] = [
  { name: "pantheon-adr", options: `crateName = "adr"\n  skillName = "adr-creator"\n  gitDependencyIgnore { "pahdk-core"; "pahdk-install" }` },
  { name: "pantheon-journal", options: `crateName = "journal"\n  skillName = "journal-entry-creator"\n  gitDependencyIgnore { "pahdk-core"; "pahdk-install" }` },
  { name: "pantheon-skill-auditor", options: `crateName = "skill-auditor"\n  skillName = "skill-quality-auditor"\n  compositeAction = true\n  gitDependencyIgnore { "pahdk-core"; "pahdk-install" }` },
  { name: "pantheon-agentic-harness-dev-kit", options: `binary = false\n  skills = false\n  features = "all"` },
];
const config = (options: string): string =>
  `amends "${uri}#/Config.pkl"\nimport "${uri}#/Builtins.pkl"\n\noptions {\n  toolchain = "1.96.1"\n  ${options}\n}\n\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles()\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n`;
const modeOf = (m: string): string => (m === "100755" ? "0755" : "0644");
const jobNames = (text: string): { id: string; name: string }[] => {
  const doc = Bun.YAML.parse(text) as { jobs?: Record<string, { name?: string }> } | null;
  return Object.entries(doc?.jobs ?? {}).map(([id, j]) => ({ id, name: String(j?.name ?? id) }));
};

say(`# Proof of the v${version} release candidate against the four repositories`);
say();
say(
  `Phase 7, wave A (tasks 7.1 and 7.2). Generated by \`runner/scripts/proof-render.ts\`. The runner is the published \`${asset}\` of v${version}, its attestation and the attestation of \`checksums.txt\` verified and its digest matched to \`checksums.txt\`; it reports ${reported}. The package is loaded through \`${uri.replace(pkgSum, `${pkgSum.slice(0, 12)}...`)}\`. Each repository is cloned (https, depth 1) into a temporary folder; nothing is written to the repositories.`,
);
say();

let allPass = true;
const summary: string[] = [];
const totals = new Map<string, number>();
for (const spec of REPOS) {
  const repo = join(work, spec.name);
  need(await sh(["git", "clone", "--quiet", "--depth", "1", `https://github.com/pantheon-org/${spec.name}.git`, repo], work), `clone of ${spec.name}`);
  const head = (await sh(["git", "rev-parse", "--short", "HEAD"], repo)).out;
  const cfg = join(work, `${spec.name}.pkl`);
  writeFileSync(cfg, config(spec.options));

  const before = await hkCheck(repo);
  need(await sh([runner, "contract", cfg], work, runEnv), `contract check for ${spec.name}`);
  // `contract` validates and prints a summary, so the JSON itself comes from pkl, with the flags the runner uses (src/pkl.ts).
  const pklJson = await sh([join(pklBin, "pkl"), "eval", "-f", "json", "--allowed-modules", "pkl:,file:,package:,projectpackage:", "--allowed-resources", "package:,projectpackage:,https:", cfg], work, runEnv);
  const contract = JSON.parse(need(pklJson, `pkl eval for ${spec.name}`).out) as {
    files: { path: string; class: string; mode: string; content?: string }[];
  };
  const tracked = new Map(
    (await sh(["git", "ls-tree", "-r", "HEAD"], repo)).out
      .split("\n")
      .filter(Boolean)
      .map((l) => {
        const m = /^(\d+) \w+ \w+\t(.+)$/.exec(l)!;
        return [m[2], m[1]] as const;
      }),
  );

  const adoption = await sh([runner, "render", cfg, "--root", repo], work, runEnv);
  const refused = adoption.err.split("\n").filter((l) => /^  \S.*?: /.test(l)).map((l) => l.trim().split(":")[0]);
  need(await sh([runner, "render", cfg, "--root", repo, "--force"], work, runEnv), `render --force for ${spec.name}`);
  // The migration of decision 31: hk.pkl is a seed, so the repository's own is replaced by the seeded one, as real-repo-tooling.ts does.
  writeFileSync(join(repo, "hk.pkl"), contract.files.find((f) => f.path === "hk.pkl")!.content!);

  const rows: { path: string; category: string; detail: string }[] = [];
  for (const f of contract.files.filter((f) => f.class !== "external").sort((a, b) => a.path.localeCompare(b.path))) {
    const original = tracked.has(f.path) ? (await Bun.$`git -C ${repo} show HEAD:${f.path}`.quiet().text()) : undefined;
    const diff = (await sh(["git", "diff", "--numstat", "HEAD", "--", f.path], repo)).out.split(/\s+/);
    const r = categorise({
      repo: spec.name,
      path: f.path,
      theirs: original,
      mine: f.content!,
      theirsMode: tracked.has(f.path) ? modeOf(tracked.get(f.path)!) : undefined,
      mode: f.mode,
      numstat: { added: Number(diff[0] || 0), removed: Number(diff[1] || 0) },
    });
    rows.push({ path: f.path, category: r.category, detail: r.detail });
    totals.set(r.category, (totals.get(r.category) ?? 0) + 1);
  }
  const uncategorised = rows.filter((r) => r.category === "UNCATEGORISED");

  const checkAfter = await sh([runner, "check", cfg, "--root", repo], work, runEnv);
  const again = await sh([runner, "render", cfg, "--root", repo], work, runEnv);
  const after = await hkCheck(repo);
  const hkSame = before.code === after.code && JSON.stringify(before.steps) === JSON.stringify(after.steps);

  // The required checks of the repository's rulesets (read-only GET) against the job names of every workflow in the clone after the render.
  const rules = need(await sh(["gh", "api", `repos/pantheon-org/${spec.name}/rules/branches/main`], work, ghEnv()), `rules of ${spec.name}`);
  type Rule = { type: string; parameters?: { required_status_checks?: { context: string }[] } };
  const required = [
    ...new Set((JSON.parse(rules.out) as Rule[]).filter((r) => r.type === "required_status_checks").flatMap((r) => r.parameters?.required_status_checks?.map((c) => c.context) ?? [])),
  ].sort();
  const names = readdirSync(join(repo, ".github/workflows"))
    .filter((n) => /\.ya?ml$/.test(n))
    .flatMap((n) => jobNames(readFileSync(join(repo, ".github/workflows", n), "utf8")));
  const missing = required.filter((c) => !names.some((n) => n.name === c || n.id === c || c.startsWith(`${n.name} (`)));

  const pass = uncategorised.length === 0 && checkAfter.code === 0 && again.out.includes("written 0 file(s)") && hkSame && missing.length === 0;
  if (!pass) allPass = false;
  const count = (c: string) => rows.filter((r) => r.category === c).length;
  summary.push(
    `| ${spec.name} | ${head} | ${rows.length} | ${count("identical")} | ${rows.length - count("identical") - uncategorised.length} | ${uncategorised.length} | ${required.length} | ${missing.length} | ${hkSame ? "same" : "DIFFERENT"} | ${pass ? "PASS" : "FAIL"} |`,
  );

  say(`## ${spec.name} at ${head}`);
  say();
  say("| File | Category | Detail |");
  say("|---|---|---|");
  for (const r of rows) say(`| ${r.path} | ${r.category} | ${r.detail.replace(/\|/g, "\\|") || "none"} |`);
  say();
  say(`- First render without \`--force\`: ${refused.length} file(s) refused as existing and different: ${refused.join(", ") || "none"}. The rest is the adoption rendered with \`--force\`, plus the decided \`hk.pkl\` migration.`);
  say(`- \`check\` after the render: exit ${checkAfter.code}. Second render: ${again.out.split("\n").pop()}.`);
  say(`- Required status checks of the rulesets on \`main\` (${required.length}): ${required.map((c) => `\`${c}\``).join(", ") || "none"}. Not produced by any workflow after the render: ${missing.join(", ") || "none"}.`);
  say(`- \`hk check --all --no-fail-fast\` before: exit ${before.code}, ${before.steps.join(", ")}. After: exit ${after.code}, ${after.steps.join(", ")}. Same outcome: ${hkSame ? "yes" : "NO"}.`);
  const findings = [...new Set([...before.failures, ...after.failures])];
  say(`- Findings of the hook steps (decision 80, \`rumdl\` and the others), before or after: ${findings.length}${findings.length ? "\n" + findings.map((l) => `  - ${l}`).join("\n") : ""}`);
  say();
}

say("## Categories over the four repositories");
say();
say("| Category | Files |");
say("|---|---|");
for (const [c, n] of [...totals.entries()].sort()) say(`| ${c} | ${n} |`);
say();
say("## Summary");
say();
say("| Repository | Commit | Files | Identical | Categorised differences | Uncategorised | Required checks | Missing | hk | Result |");
say("|---|---|---|---|---|---|---|---|---|---|");
for (const s of summary) say(s);
say();
say(`Verdict: ${allPass ? "PASS" : "FAIL"}`);
console.log(lines.join("\n"));
// The scratch clones and the downloaded runner are deleted.
rmSync(work, { recursive: true, force: true });
process.exit(allPass ? 0 : 1);
