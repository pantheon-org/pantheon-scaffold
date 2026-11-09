// Phase 7 wave E (task 7.5, and the sample check of 7.2): the review dossier. Read-only. It loads the PUBLISHED package of the release through its real
// package:// URI, evaluates the standard set for the four repositories, and writes two documents: the security dossier (every rendered workflow, what
// the public release assets reveal) and the sample check (20 percent of the categorised differences, at least one per category, chosen by a written seed).
// Nothing is written to the four repositories: they are cloned (https, depth 1) into a temporary folder. Nothing in the output is a secret, a personal
// e-mail address or a user name: the scan reports counts, file names and domains, never the matched value.
// usage: bun runner/scripts/proof-dossier.ts [version]   (default: the version in pkl/PklProject)
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { categorise } from "./proof-categories";

const root = resolve(import.meta.dir, "../..");
const REPO = "pantheon-org/pantheon-scaffold";
const version = Bun.argv[2] ?? /version = "([^"]+)"/.exec(readFileSync(join(root, "pkl/PklProject"), "utf8"))![1];
const SEED = `pantheon-scaffold-${version}-wave-e`;
const today = new Date().toISOString().slice(0, 10).split("-").reverse().join("-");

type Run = { out: string; err: string; code: number };
const ghEnv = (): Record<string, string> =>
  Object.fromEntries(Object.entries(process.env).filter(([k, v]) => k !== "GH_TOKEN" && v !== undefined)) as Record<string, string>;
const sh = async (cmd: string[], cwd: string, env?: Record<string, string>): Promise<Run> => {
  const p = Bun.spawn(cmd, { cwd, stdout: "pipe", stderr: "pipe", ...(env ? { env } : {}) });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};
const need = (r: Run, what: string): Run => {
  if (r.code !== 0) throw new Error(`${what} failed (exit ${r.code}): ${r.err || r.out}`);
  return r;
};
const modeOf = (m: string): string => (m === "100755" ? "0755" : "0644");

const work = mkdtempSync(join(tmpdir(), "proof-dossier-"));
try {
  await main();
} finally {
  rmSync(work, { recursive: true, force: true });
}

async function main(): Promise<void> {
  // 1. The release: every asset, the checksums, and the attestation of the checksums file, verified before anything is read.
  const rel = join(work, "release");
  mkdirSync(rel);
  need(await sh(["gh", "release", "download", `v${version}`, "--repo", REPO, "-D", rel], work, ghEnv()), "gh release download");
  need(await sh(["gh", "attestation", "verify", join(rel, "checksums.txt"), "--repo", REPO], work, ghEnv()), "attestation of checksums.txt");
  const sums = new Map(readFileSync(join(rel, "checksums.txt"), "utf8").split("\n").filter(Boolean).map((l) => { const [h, n] = l.trim().split(/\s+/); return [n, h] as const; }));
  const pkgSum = sums.get(`pantheon-scaffold@${version}`);
  if (!pkgSum) throw new Error(`checksums.txt has no entry for pantheon-scaffold@${version}`);
  for (const [name, hash] of sums) {
    const got = new Bun.CryptoHasher("sha256").update(readFileSync(join(rel, name))).digest("hex");
    if (got !== hash) throw new Error(`${name} does not match checksums.txt`);
  }
  const notes = need(await sh(["gh", "release", "view", `v${version}`, "--repo", REPO, "--json", "body,isPrerelease", "--jq", "."], work, ghEnv()), "release notes").out;
  const visibility = need(await sh(["gh", "api", `repos/${REPO}`, "--jq", ".visibility"], work, ghEnv()), "repository visibility").out;

  // 2. pkl, found without a mise shim, and the standard set of each repository evaluated from the published package.
  const where = (await sh(["mise", "where", "pkl@0.32.2"], root)).out;
  const pkl = (await sh(["find", where, "-maxdepth", "3", "-name", "pkl", "-type", "f", "-perm", "-u+x"], root)).out.split("\n")[0];
  if (!pkl) throw new Error("pkl 0.32.2 is not installed");
  const uri = `package://github.com/${REPO}/releases/download/v${version}/pantheon-scaffold@${version}::sha256:${pkgSum}`;
  const REPOS: { name: string; options: string }[] = [
    { name: "pantheon-adr", options: `crateName = "adr"\n  skillName = "adr-creator"\n  gitDependencyIgnore { "pahdk-core"; "pahdk-install" }` },
    { name: "pantheon-journal", options: `crateName = "journal"\n  skillName = "journal-entry-creator"\n  gitDependencyIgnore { "pahdk-core"; "pahdk-install" }` },
    { name: "pantheon-skill-auditor", options: `crateName = "skill-auditor"\n  skillName = "skill-quality-auditor"\n  compositeAction = true\n  gitDependencyIgnore { "pahdk-core"; "pahdk-install" }` },
    { name: "pantheon-agentic-harness-dev-kit", options: `binary = false\n  skills = false\n  features = "all"` },
  ];
  const config = (options: string): string =>
    `amends "${uri}#/Config.pkl"\nimport "${uri}#/Builtins.pkl"\n\noptions {\n  toolchain = "1.96.1"\n  ${options}\n}\n\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles()\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n`;
  const runEnv: Record<string, string> = { HOME: process.env.HOME ?? "", PATH: `${dirname(pkl)}:/usr/bin:/bin`, NO_COLOR: "1" };

  type CFile = { path: string; class: string; mode: string; content?: string };
  type Diff = { repo: string; path: string; category: string; detail: string; unified: string };
  const rendered = new Map<string, CFile[]>();
  const diffs: Diff[] = [];
  const identical = new Map<string, number>();
  for (const spec of REPOS) {
    const cfg = join(work, `${spec.name}.pkl`);
    writeFileSync(cfg, config(spec.options));
    const json = need(await sh([pkl, "eval", "-f", "json", "--allowed-modules", "pkl:,file:,package:,projectpackage:", "--allowed-resources", "package:,projectpackage:,https:", cfg], work, runEnv), `pkl eval for ${spec.name}`);
    const files = (JSON.parse(json.out) as { files: CFile[] }).files.filter((f) => f.class !== "external");
    rendered.set(spec.name, files);

    const repo = join(work, spec.name);
    need(await sh(["git", "clone", "--quiet", "--depth", "1", `https://github.com/pantheon-org/${spec.name}.git`, repo], work), `clone of ${spec.name}`);
    const tracked = new Map(
      (await sh(["git", "ls-tree", "-r", "HEAD"], repo)).out.split("\n").filter(Boolean).map((l) => { const m = /^(\d+) \w+ \w+\t(.+)$/.exec(l)!; return [m[2], m[1]] as const; }),
    );
    for (const f of files) {
      const exists = tracked.has(f.path);
      if (f.class === "seed" && exists && f.path !== "hk.pkl") continue;
      const abs = join(repo, f.path);
      mkdirSync(dirname(abs), { recursive: true });
      writeFileSync(abs, f.content!, { mode: f.mode === "0755" ? 0o755 : 0o644 });
    }
    for (const f of files.sort((a, b) => a.path.localeCompare(b.path))) {
      const theirs = tracked.has(f.path) ? (await Bun.$`git -C ${repo} show HEAD:${f.path}`.quiet().text()) : undefined;
      const stat = (await sh(["git", "diff", "--numstat", "HEAD", "--", f.path], repo)).out.split(/\s+/);
      const r = categorise({ repo: spec.name, path: f.path, theirs, mine: f.content!, theirsMode: tracked.has(f.path) ? modeOf(tracked.get(f.path)!) : undefined, mode: f.mode, numstat: { added: Number(stat[0] || 0), removed: Number(stat[1] || 0) } });
      if (r.category === "UNCATEGORISED") throw new Error(`uncategorised difference: ${spec.name} ${f.path}: ${r.detail}`);
      if (r.category === "identical") { identical.set(spec.name, (identical.get(spec.name) ?? 0) + 1); continue; }
      let unified = "";
      if (theirs === undefined) unified = `(new file, ${f.content!.split("\n").length - 1} lines)`;
      else if (theirs === f.content) unified = `(content identical, mode ${tracked.has(f.path) ? modeOf(tracked.get(f.path)!) : "?"} to ${f.mode})`;
      else {
        // The two texts are compared directly: a seeded file that exists is kept by the render, so the clone's own diff of it would be empty.
        writeFileSync(join(work, "theirs.tmp"), theirs);
        writeFileSync(join(work, "mine.tmp"), f.content!);
        unified = (await sh(["git", "diff", "--no-index", "--no-color", "-U1", "theirs.tmp", "mine.tmp"], work)).out.split("\n").slice(4).join("\n");
      }
      diffs.push({ repo: spec.name, path: f.path, category: r.category, detail: r.detail, unified });
    }
  }

  // 3. The sample: 20 percent of the differences (rounded up), at least one per category, from a seeded generator.
  const rand = mulberry32(hash32(SEED));
  const byCategory = new Map<string, Diff[]>();
  for (const d of diffs) byCategory.set(d.category, [...(byCategory.get(d.category) ?? []), d]);
  const target = Math.max(Math.ceil(diffs.length * 0.2), byCategory.size);
  const chosen = new Set<Diff>();
  for (const [, list] of [...byCategory.entries()].sort()) chosen.add(list[Math.floor(rand() * list.length)]);
  const rest = diffs.filter((d) => !chosen.has(d));
  while (chosen.size < target && rest.length) chosen.add(rest.splice(Math.floor(rand() * rest.length), 1)[0]);
  const sample = [...chosen].sort((a, b) => a.repo.localeCompare(b.repo) || a.path.localeCompare(b.path));

  // 4. The workflow table: every rendered workflow of every repository, from the parsed YAML.
  type WF = { sig: string; row: string; repos: string[]; name: string };
  const wfRows = new Map<string, WF>();
  const flags: string[] = [];
  for (const [repoName, files] of rendered) {
    for (const f of files.filter((f) => /^\.github\/workflows\/[^/]+\.ya?ml$/.test(f.path))) {
      const a = analyse(f.content!);
      const row = `| ${f.path.split("/").pop()} | ${a.triggers} | ${a.perms} | ${a.secrets || "none"} | ${a.tokens} | ${a.actions} | ${a.untrusted} |`;
      const key = `${f.path}|${row}`;
      const cur = wfRows.get(key);
      if (cur) cur.repos.push(repoName);
      else wfRows.set(key, { sig: key, row, repos: [repoName], name: f.path });
      if (a.prt) flags.push(`${f.path.split("/").pop()} uses \`pull_request_target\` (permissions ${a.perms}; ${a.shape}; event data reaches the script through: ${a.untrusted})`);
    }
  }
  const wfTable = [...wfRows.values()].sort((a, b) => a.name.localeCompare(b.name)).map((w) => `${w.row} ${w.repos.length === REPOS.length ? "all four" : w.repos.map((r) => r.replace("pantheon-", "")).join(", ")} |`);
  const workflowCount = new Set([...wfRows.values()].map((w) => w.name)).size;

  // 5. What the public release reveals: counts and file names, never the matched value.
  const scan = scanAssets(rel, version, notes);

  const totals = new Map<string, number>();
  for (const d of diffs) totals.set(d.category, (totals.get(d.category) ?? 0) + 1);
  const categoryRows = [...totals.entries()].sort().map(([c, n]) => `| ${c} | ${n} | ${sample.filter((s) => s.category === c).length} |`);
  const ids = REPOS.map((r) => `${r.name}: ${identical.get(r.name) ?? 0} identical, ${diffs.filter((d) => d.repo === r.name).length} differences`);

  writeFileSync(join(root, `docs/security-dossier-${version}.md`), dossier({ wfTable, flags, scan, visibility, workflowCount, ids, files: [...sums.keys()] }));
  // A sample check the owner has reviewed holds their ticks and the review record: it is never regenerated over.
  const samplePath = join(root, `docs/sample-check-${version}.md`);
  const reviewed = existsSync(samplePath) && readFileSync(samplePath, "utf8").includes("**Review record (");
  if (!reviewed) writeFileSync(samplePath, sampleDoc({ sample, diffs: diffs.length, categoryRows, target }));
  console.log(`dossier and sample written: ${workflowCount} workflow files, ${wfTable.length} table rows, ${diffs.length} differences, ${sample.length} sampled, ${scan.findings.length} scan finding lines`);

  function dossier(d: { wfTable: string[]; flags: string[]; scan: Scan; visibility: string; workflowCount: number; ids: string[]; files: string[] }): string {
    return `# Security dossier of the v${version} release candidate

Phase 7, wave E (task 7.5). Generated on ${today} by \`runner/scripts/proof-dossier.ts\` from the **published** package and assets of v${version} (the checksums file's attestation and every asset's digest are verified first). Nothing is written to the four repositories. The repository \`${REPO}\` is **${d.visibility}**, so this file is public: it holds counts, file names and domains, never a matched secret, an e-mail address or a user name.

## Scope and what is not covered

- **Security approval is not obtained.** No Security reviewer is named and none was asked (decision 78, an accepted risk of the owner, 07-11-2026). This dossier is input for that review. It does not replace it.
- **The reviewer is the maintainer.** The author of the package, the owner of the four repositories and the person who reviews this dossier are the same person (decisions 69, 70 and 72), so the review and the sample check are not independent. A second reviewer is not named.
- **Covered:** the ${d.workflowCount} workflow files that the package renders for the four repositories, and the 8 assets and the release notes of v${version}.
- **Not covered:** the content of \`release.yml\` (see below), anything in the four repositories that the package does not render, the GitHub organisation's settings, and the App \`pantheon-ai-bot\` itself.

## Every rendered workflow

Rendered with the published runner's package for each repository (options as in the wave A report), parsed with Bun.YAML. Columns: triggers; permissions (top level, then each job that sets its own); secrets read; tokens (\`GITHUB_TOKEN\` or an App token and its permissions); third-party and first-party actions and how many are pinned to a full commit; \`\${{ }}\` expressions inside \`run:\` that read event data (untrusted input). A row repeated for several repositories is shown once.

| Workflow | Triggers | Permissions | Secrets | Tokens | Actions pinned | Untrusted input in run | Repositories |
|---|---|---|---|---|---|---|---|
${d.wfTable.join("\n")}

${d.flags.length ? `Review items from the table:\n\n${[...new Set(d.flags)].map((f) => `- ${f}. It runs with base-repository context on a pull request, so it must never check out or run pull-request code, and event data must reach a script only through an environment variable, never inline in the script text. The reviewer checks the line above against that rule.`).join("\n")}\n` : "No workflow uses `pull_request_target`.\n"}
## The \`release.yml\` that \`dist generate\` produces

The package does **not** render \`release.yml\`. \`cargo-dist\` generates it in each repository that ships a binary, and the package renders two things around it: \`dist-workspace.toml\` (with the two action commits that the generated file uses today, decision 85) and \`dist-check.yml\`, which runs \`dist generate --check\` and fails when the committed \`release.yml\` differs from what \`dist\` would generate. So this dossier covers the **check** and not the **content**: what \`release.yml\` is allowed to do (its permissions, its tokens, its tag trigger) is reviewed in each repository, and is outside this review. The library repository renders no \`dist-check.yml\` (\`binary = false\`).

## Token and App key custody

What the files show: the workflows \`pr-auto-rebase.yml\`, \`release-automerge.yml\` and \`release-please.yml\` read the secret \`GH_APP_PRIVATE_KEY\` and mint a short-lived installation token for the App \`pantheon-ai-bot\` (its client id is written in the file: a public identifier). Each asks for contents and pull-requests write on the repository it runs in. The token step runs before every other step, and a key that is not valid stops the job there (shown in the wave C run).

What is **not** in the files, and that the review token cannot read (organisation and App settings): the App's permission scope and installation list, who holds the key and where, rotation, who can read the secret, and whether the custody is written down. Six questions about these were put to the owner and answered on 09-11-2026.

The answers are **not recorded in this repository**, which is public: they describe how a secret is held and who can reach it. They are held in the owner's private record, together with an analysis of what the key could do if it were misused and the gaps that analysis found. This dossier therefore does not claim that the custody is verified. It records that the questions were asked and answered, that the answers are held privately, and that the private record lists open items.

## What the public release reveals

Scanned: ${d.files.map((f) => `\`${f}\``).join(", ")}, the release notes, and the files inside \`pantheon-scaffold@${version}.zip\`. A match is counted and located, never printed.

| Pattern | Matches | Where |
|---|---|---|
${d.scan.rows.join("\n")}

${d.scan.findings.length ? `Notes:\n\n${d.scan.findings.map((f) => `- ${f}`).join("\n")}\n` : ""}
Token and key patterns are the GitHub token prefixes, \`github_pat_\`, AWS access key ids, Slack tokens, \`-----BEGIN ... PRIVATE KEY-----\` and \`sk-\` keys. A non-zero count there is an incident for the owner's internal route and not a line in this file.

## Findings carried from the earlier waves

1. **Organisation secrets are visible to a new repository** (wave C). The names are deliberately not recorded in this repository, which is public. The owner reviews the organisation setting.
2. **\`[skip ci]\` does not stop Dependabot** (wave C): the rendered \`dependabot.yml\` starts Dependabot updates on the first push of a new repository.
3. **\`rumdl\` fails in all four repositories today** (decision 80): fix or exempt each before \`hk check --all\` is enforced in CI.
4. **\`sync-labels.sh\` becomes executable** in each adopting repository (decision 79).
5. **\`upgrade\` bugs found and fixed in Phase 7** (decisions 88 and the \`mise.toml\` seal): the published 0.1.3 and 0.1.4 runners still have them. 0.1.5 is the candidate.

## Differences by repository

${d.ids.map((i) => `- ${i}`).join("\n")}
`;
  }
}

function sampleDoc(d: { sample: { repo: string; path: string; category: string; detail: string; unified: string }[]; diffs: number; categoryRows: string[]; target: number }): string {
  const entries = d.sample.map((s, i) => `### ${i + 1}. ${s.repo}: ${s.path}\n\n- Category: ${s.category}${s.detail ? `. ${s.detail}` : ""}\n- Owner reviewed: [ ]\n\n\`\`\`diff\n${s.unified.split("\n").slice(0, 40).join("\n")}${s.unified.split("\n").length > 40 ? "\n... (cut at 40 lines)" : ""}\n\`\`\`\n`);
  return `# Sample check of the v${version} differences

Phase 7, wave E (task 7.2, decision 56). Generated on ${today} by \`runner/scripts/proof-dossier.ts\`. The package renders the standard set into each of the four repositories and ${d.diffs} files differ from the repository's own copy (the others are identical). This is **${d.sample.length} of ${d.diffs}** of them (20 percent rounded up is ${Math.ceil(d.diffs * 0.2)}, and at least one of every category), chosen with a seeded generator so the choice can be repeated: the seed is \`${SEED}\`.

The owner reads each diff below and ticks it. A sample the owner does not finish is recorded as not signed. The owner is also the maintainer, so this check is **not independent** (decisions 69 and 72).

| Category | Differences | In the sample |
|---|---|---|
${d.categoryRows.join("\n")}

${entries.join("\n")}`;
}

type Scan = { rows: string[]; findings: string[] };
function scanAssets(rel: string, v: string, notes: string): Scan {
  const unzip = join(work, "zip");
  mkdirSync(unzip);
  Bun.spawnSync(["unzip", "-q", join(rel, `pantheon-scaffold@${v}.zip`), "-d", unzip]);
  const targets: { name: string; buf: Buffer }[] = [];
  const walk = (dir: string, prefix: string): void => {
    for (const e of readdirSync(dir)) {
      const p = join(dir, e);
      if (statSync(p).isDirectory()) walk(p, `${prefix}${e}/`);
      else targets.push({ name: `${prefix}${e}`, buf: readFileSync(p) });
    }
  };
  for (const e of readdirSync(rel)) if (!e.endsWith(".zip") && e !== "checksums.txt") targets.push({ name: e, buf: readFileSync(join(rel, e)) });
  targets.push({ name: "checksums.txt", buf: readFileSync(join(rel, "checksums.txt")) });
  walk(unzip, `zip:${v}/`);
  targets.push({ name: "release notes", buf: Buffer.from(notes) });
  const patterns: { label: string; re: RegExp; secret?: boolean; domainOnly?: boolean; hostNames?: boolean }[] = [
    { label: "GitHub token prefixes (ghp_, gho_, ghu_, ghs_, ghr_, github_pat_)", re: /\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})/g, secret: true },
    { label: "AWS access key ids", re: /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/g, secret: true },
    { label: "Slack tokens", re: /\bxox[abprs]-[A-Za-z0-9-]{10,}/g, secret: true },
    { label: "Private key blocks", re: /-----BEGIN [A-Z ]*PRIVATE KEY-----/g, secret: true },
    { label: "sk- style keys", re: /\bsk-[A-Za-z0-9_-]{32,}/g, secret: true },
    { label: "E-mail addresses (domains only)", re: /[A-Za-z][A-Za-z0-9._%+-]*@([A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.(?:com|org|net|io|dev|edu|gov|uk|nl|de|se|ca|eu|co|fr|info|app|ai))\b/g, domainOnly: true },
    { label: "Home-directory paths (/Users/, /home/)", re: /\/(?:Users|home)\/[A-Za-z0-9._-]+\//g },
    { label: "Internal host names (.internal, .corp, .lan)", re: /\b[a-z0-9-]+(?:\.[a-z0-9-]+)*\.(?:internal|corp|lan)\b/g, hostNames: true },
    { label: "Organisation or company words (postcodelottery, novamedia)", re: /postcode-?lotter|novamedia/gi },
  ];
  const rows: string[] = [];
  const findings: string[] = [];
  for (const p of patterns) {
    let total = 0;
    const where = new Map<string, number>();
    const domains = new Map<string, number>();
    const hosts = new Set<string>();
    for (const t of targets) {
      const text = t.buf.toString("latin1");
      const hits = [...text.matchAll(p.re)];
      if (!hits.length) continue;
      total += hits.length;
      where.set(t.name, (where.get(t.name) ?? 0) + hits.length);
      if (p.hostNames) for (const h of hits) hosts.add(h[0].toLowerCase());
      if (p.domainOnly) for (const h of hits) domains.set(h[1].toLowerCase(), (domains.get(h[1].toLowerCase()) ?? 0) + 1);
    }
    const loc = [...where.entries()].slice(0, 6).map(([n, c]) => `${n} (${c})`).join(", ") || "none";
    rows.push(`| ${p.label} | ${total} | ${loc}${where.size > 6 ? `, and ${where.size - 6} more files` : ""} |`);
    if (p.secret && total > 0) findings.push(`**${p.label}: ${total} match(es). Stop and treat as an incident for the owner's internal route. The values are not printed.**`);
    if (p.domainOnly && domains.size) findings.push(`E-mail domains seen: ${[...domains.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([d, c]) => `${d} (${c})`).join(", ")}. Most of the matches sit in the three compiled runners, which bundle the bun runtime and third-party code (certificate and package metadata), so they are not authored here. Whether an address belongs to a person is for the owner to check in the build inputs.`);
    if (p.domainOnly && domains.has("users.noreply.github.com")) findings.push("The `authors` field of the package metadata (set in `pkl/PklProject`) holds the maintainer's GitHub noreply address, which names a GitHub account. The owner confirms that this public attribution is intended.");
    if (p.hostNames && hosts.size) findings.push(`Internal-style host names seen (not a secret, listed so the owner can judge them): ${[...hosts].slice(0, 6).map((h) => `\`${h}\``).join(", ")}.`);
  }
  return { rows, findings };
}

function analyse(text: string): { triggers: string; perms: string; secrets: string; tokens: string; actions: string; untrusted: string; prt: boolean; shape: string } {
  const y = Bun.YAML.parse(text) as { on?: Record<string, unknown> | string | string[]; permissions?: unknown; jobs?: Record<string, { permissions?: unknown; steps?: { uses?: string; run?: string; with?: Record<string, unknown>; env?: Record<string, unknown> }[] }> };
  const on = y.on ?? {};
  const triggers = Array.isArray(on) ? on.join(", ") : typeof on === "string" ? on : Object.keys(on).join(", ");
  const fmt = (p: unknown): string => (p === undefined ? "unset" : typeof p === "string" ? p : Object.keys(p as object).length ? Object.entries(p as Record<string, string>).map(([k, v]) => `${k}:${v}`).join(" ") : "none");
  const jobPerms = Object.entries(y.jobs ?? {}).filter(([, j]) => j.permissions !== undefined).map(([id, j]) => `${id}=${fmt(j.permissions)}`);
  const perms = `top ${fmt(y.permissions)}${jobPerms.length ? `; jobs ${jobPerms.join("; ")}` : ""}`;
  const secrets = [...new Set([...text.matchAll(/secrets\.([A-Za-z_][A-Za-z0-9_]*)/g)].map((m) => m[1]).filter((s) => s !== "GITHUB_TOKEN"))].join(", ");
  const steps = Object.values(y.jobs ?? {}).flatMap((j) => j.steps ?? []);
  const apps = steps.filter((s) => s.uses?.startsWith("actions/create-github-app-token@"));
  const appPerms = apps.flatMap((s) => Object.keys(s.with ?? {}).filter((k) => k.startsWith("permission-")).map((k) => `${k.slice(11)}:${s.with![k]}`));
  const usesGithubToken = /github\.token|secrets\.GITHUB_TOKEN|GITHUB_TOKEN/.test(text);
  const tokens = [usesGithubToken ? "GITHUB_TOKEN" : "", apps.length ? `App token (${appPerms.join(" ") || "no permissions set"})` : ""].filter(Boolean).join("; ") || "none";
  const uses = steps.map((s) => s.uses).filter((u): u is string => !!u && !u.startsWith("./"));
  const pinned = uses.filter((u) => /@[0-9a-f]{40}$/.test(u)).length;
  const actions = `${pinned} of ${uses.length}`;
  const eventData = /github\.(event|head_ref|ref_name|actor)|inputs\./;
  const inline = [...new Set(steps.flatMap((s) => [...(s.run ?? "").matchAll(/\$\{\{\s*([^}]+?)\s*\}\}/g)].map((m) => m[1])).filter((e) => eventData.test(e)))];
  const viaEnv = [...new Set(steps.flatMap((s) => Object.values(s.env ?? {}).map(String)).flatMap((v) => [...v.matchAll(/\$\{\{\s*([^}]+?)\s*\}\}/g)].map((m) => m[1])).filter((e) => eventData.test(e)))];
  const untrusted = `inline: ${inline.length ? inline.map((e) => `\`${e}\``).join(", ") : "none"}; via env: ${viaEnv.length ? viaEnv.map((e) => `\`${e}\``).join(", ") : "none"}`;
  const checkout = uses.some((u) => u.startsWith("actions/checkout@"));
  const runs = steps.filter((s) => s.run).length;
  return { triggers, perms, secrets, tokens, actions, untrusted, prt: triggers.includes("pull_request_target"), shape: `${steps.length} step(s), ${runs} run step(s), checkout ${checkout ? "yes" : "no"}, event data inline in a run: ${inline.length ? "yes" : "no"}` };
}

function hash32(s: string): number {
  let h = 2166136261;
  for (const c of s) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return h >>> 0;
}
function mulberry32(a: number): () => number {
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
