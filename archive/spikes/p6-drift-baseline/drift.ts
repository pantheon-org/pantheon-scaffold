// Do-nothing baseline for gate A: today's copies plus a drift check. Read-only: one gh api call per repo (git trees),
// compares git blob SHAs across the repos' default branch. Usage: bun drift.ts [--json]
import { $ } from "bun";

const ORG = "pantheon-org";
const REPOS = ["pantheon-adr", "pantheon-journal", "pantheon-skill-auditor", "pantheon-agentic-harness-dev-kit"];
// Files that differ by design (shared plan, evidence section): never counted as drift.
const BY_DESIGN = [/^Cargo\.(toml|lock)$/, /^release-please-config\.json$/, /^\.release-please-manifest\.json$/, /^README\.md$/, /^CLAUDE\.md$/, /^crates\//, /^docs\//, /^skills\//, /^src\//, /^tests\//, /^dist-workspace\.toml$/];

type Tree = Map<string, string>;
const started = performance.now();
const trees = new Map<string, Tree>();
const heads = new Map<string, string>();
for (const repo of REPOS) {
  const head = (await $`env -u GH_TOKEN gh api repos/${ORG}/${repo}/commits/main --jq .sha`.text()).trim();
  const raw = await $`env -u GH_TOKEN gh api repos/${ORG}/${repo}/git/trees/${head}?recursive=1`.json();
  if (raw.truncated) throw new Error(`tree of ${repo} was truncated`);
  const t: Tree = new Map();
  for (const e of raw.tree as { path: string; type: string; sha: string }[]) if (e.type === "blob") t.set(e.path, e.sha);
  trees.set(repo, t);
  heads.set(repo, head.slice(0, 7));
}

const paths = new Set<string>();
for (const t of trees.values()) for (const p of t.keys()) paths.add(p);

const rows = [...paths].sort().filter((p) => !BY_DESIGN.some((re) => re.test(p))).map((p) => {
  const shas = REPOS.map((r) => trees.get(r)!.get(p));
  const present = shas.filter((s): s is string => !!s);
  const distinct = new Set(present).size;
  return { path: p, present: present.length, distinct, class: present.length === 1 ? "single-repo" : distinct === 1 ? "identical" : "drifted" };
});

const count = (c: string) => rows.filter((r) => r.class === c).length;
const shared = rows.filter((r) => r.present >= 2);
if (Bun.argv.includes("--json")) {
  console.log(JSON.stringify({ heads: Object.fromEntries(heads), rows }, null, 2));
} else {
  console.log(`commits: ${[...heads].map(([r, h]) => `${r.replace("pantheon-", "")}@${h}`).join(", ")}`);
  console.log(`files in at least one repo, excluding by-design: ${rows.length}`);
  console.log(`present in 2+ repos: ${shared.length} -> identical ${count("identical")}, drifted ${count("drifted")}`);
  console.log(`present in only one repo: ${count("single-repo")}`);
  console.log("drifted files (distinct versions / repos having it):");
  for (const r of rows.filter((x) => x.class === "drifted")) console.log(`  ${r.distinct}/${r.present}  ${r.path}`);
  console.log(`runtime: ${((performance.now() - started) / 1000).toFixed(1)} s, gh api calls: ${REPOS.length * 2}`);
}
