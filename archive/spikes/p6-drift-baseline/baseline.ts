// SP-0.1 baseline: classifies every file across the four Rust repos' default branch, records commit SHAs, toggles,
// repo settings and rulesets. Read-only (GET only). Usage: bun baseline.ts [out.json]
import { $ } from "bun";

const ORG = "pantheon-org";
const REPOS = ["pantheon-adr", "pantheon-journal", "pantheon-skill-auditor", "pantheon-agentic-harness-dev-kit"];
const BY_DESIGN = [/^Cargo\.(toml|lock)$/, /^release-please-config\.json$/, /^\.release-please-manifest\.json$/, /^README\.md$/, /^CLAUDE\.md$/, /^crates\//, /^docs\//, /^skills\//, /^src\//, /^tests\//, /^dist-workspace\.toml$/, /^CHANGELOG\.md$/];
const gh = async (path: string) => {
  const r = await $`env -u GH_TOKEN gh api ${path}`.quiet().nothrow();
  return r.exitCode === 0 ? JSON.parse(r.stdout.toString()) : { __error: r.stderr.toString().trim().split("\n")[0].slice(0, 100) };
};

const trees = new Map<string, Map<string, string>>();
const heads: Record<string, string> = {};
for (const repo of REPOS) {
  const head = (await gh(`repos/${ORG}/${repo}/commits/main`)).sha as string;
  const raw = await gh(`repos/${ORG}/${repo}/git/trees/${head}?recursive=1`);
  if (raw.truncated) throw new Error(`tree of ${repo} truncated`);
  heads[repo] = head;
  trees.set(repo, new Map((raw.tree as { path: string; type: string; sha: string }[]).filter((e) => e.type === "blob").map((e) => [e.path, e.sha])));
}

const paths = [...new Set([...trees.values()].flatMap((t) => [...t.keys()]))].sort();
const files = paths.map((path) => {
  const holders = REPOS.filter((r) => trees.get(r)!.has(path));
  const versions = new Map<string, string[]>();
  for (const r of holders) {
    const sha = trees.get(r)!.get(path)!;
    versions.set(sha, [...(versions.get(sha) ?? []), r.replace("pantheon-", "")]);
  }
  const cls = BY_DESIGN.some((re) => re.test(path)) ? "per-repo" : holders.length === 1 ? "single-repo" : versions.size === 1 ? "identical" : "drifted";
  return { path, class: cls, repos: holders.length, versions: [...versions.values()] };
});

const has = (r: string, re: RegExp) => [...trees.get(r)!.keys()].some((p) => re.test(p));
const toggles = Object.fromEntries(REPOS.map((r) => [r.replace("pantheon-", ""), {
  skills: has(r, /^skills\//) || has(r, /^\.github\/workflows\/audit-skill\.yml$/),
  binary: has(r, /^\.github\/workflows\/release\.yml$/) && has(r, /^dist-workspace\.toml$/),
  hasBinaryTarget: has(r, /(^|\/)src\/main\.rs$/),
  agnix: has(r, /^\.agnix\.toml$/) || has(r, /^\.github\/workflows\/agnix\.yml$/),
  commitTypes: has(r, /workflows\/commit-types\.yml$/),
  conventionalCommits: has(r, /workflows\/conventional-commits\.yml$/),
  zizmor: has(r, /workflows\/zizmor\.yml$/),
}]));

const settings: Record<string, unknown> = {};
const rulesets: Record<string, unknown> = {};
for (const repo of REPOS) {
  const s = await gh(`repos/${ORG}/${repo}`);
  settings[repo] = s.__error ? s : { visibility: s.visibility, default_branch: s.default_branch, allow_merge_commit: s.allow_merge_commit, allow_squash_merge: s.allow_squash_merge, allow_rebase_merge: s.allow_rebase_merge, delete_branch_on_merge: s.delete_branch_on_merge };
  const list = await gh(`repos/${ORG}/${repo}/rulesets`);
  if (list.__error) { rulesets[repo] = list; continue; }
  const out: unknown[] = [];
  for (const rs of list as { id: number }[]) {
    const d = await gh(`repos/${ORG}/${repo}/rulesets/${rs.id}`);
    out.push(d.__error ? d : {
      name: d.name, target: d.target, enforcement: d.enforcement,
      rules: (d.rules ?? []).map((x: { type: string }) => x.type),
      required_checks: (d.rules ?? []).flatMap((x: { type: string; parameters?: { required_status_checks?: { context: string }[] } }) => x.type === "required_status_checks" ? (x.parameters?.required_status_checks ?? []).map((c) => c.context) : []),
      bypass_actors: (d.bypass_actors ?? []).map((b: { actor_type: string; bypass_mode: string }) => `${b.actor_type}:${b.bypass_mode}`),
    });
  }
  rulesets[repo] = out;
}
const org = await gh(`orgs/${ORG}`);
const orgRulesets = await gh(`orgs/${ORG}/rulesets`);
const orgProps = await gh(`orgs/${ORG}/properties/schema`);

const result = {
  taken: new Date().toISOString().slice(0, 10),
  heads: Object.fromEntries(Object.entries(heads).map(([r, h]) => [r, h.slice(0, 7)])),
  counts: Object.fromEntries(["identical", "drifted", "per-repo", "single-repo"].map((c) => [c, files.filter((f) => f.class === c).length])),
  toggles, settings, rulesets,
  org: { plan: org.plan?.name, members_can_create_public_repositories: org.members_can_create_public_repositories },
  orgRulesets: orgRulesets.__error ?? orgRulesets, orgProperties: orgProps.__error ?? orgProps,
  files,
};
await Bun.write(Bun.argv[2] ?? "baseline.json", JSON.stringify(result, null, 2));
console.log(JSON.stringify({ taken: result.taken, heads: result.heads, counts: result.counts, org: result.org, orgRulesets: Array.isArray(result.orgRulesets) ? `${result.orgRulesets.length} org rulesets` : result.orgRulesets, orgProperties: Array.isArray(result.orgProperties) ? `${result.orgProperties.length} properties` : result.orgProperties }, null, 2));
