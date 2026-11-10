import { compareRulesets, toCreateShape, type Ruleset } from "./ruleset";

/** `governance/desired/repos.json`: which rulesets each repository should hold, by the file stem under `governance/desired/`. */
export type DesiredRepos = { owner: string; repos: Record<string, string[]> };

/** Reads one GitHub API path as JSON. Replaceable, which is how the tests run without GitHub. */
export type Api = (path: string) => Promise<unknown>;

/** `notes` are things the comparison could not do, so a clean result is not read as more than it is. */
export type RepoFinding = { repo: string; problems: string[]; notes: string[] };

const SLUG = /^[a-z0-9][a-z0-9._-]*$/;

/**
 * Compares the rulesets a repository has with the ones it should have. Read-only: it only calls GET. A ruleset that is missing, one that
 * is there and should not be, and every difference inside one that is there are each a problem. A repository that cannot be read is an
 * error and is never reported as "no drift".
 */
export async function checkRepo(owner: string, repo: string, wanted: Ruleset[], api: Api): Promise<RepoFinding> {
  if (!SLUG.test(owner) || !SLUG.test(repo)) throw new Error(`${owner}/${repo} is not a valid repository slug`);
  const list = await api(`repos/${owner}/${repo}/rulesets`);
  if (!Array.isArray(list)) throw new Error(`${owner}/${repo}: the rulesets answer is not a list`);
  const live: Ruleset[] = [];
  let bypassHidden = false;
  for (const item of list as { id: number }[]) {
    if (typeof item?.id !== "number") throw new Error(`${owner}/${repo}: a ruleset has no id`);
    const got = (await api(`repos/${owner}/${repo}/rulesets/${item.id}`)) as Record<string, unknown>;
    if (!("bypass_actors" in got)) bypassHidden = true;
    live.push(toCreateShape(got));
  }
  const problems: string[] = [];
  for (const w of wanted) {
    const l = live.find((x) => x.name === w.name);
    if (!l) problems.push(`ruleset "${w.name}" is missing`);
    else for (const d of compareRulesets(w, l, { skipBypass: bypassHidden })) problems.push(`ruleset "${w.name}": ${d}`);
  }
  for (const l of live) if (!wanted.some((w) => w.name === l.name)) problems.push(`ruleset "${l.name}" is present and is not in the desired state`);
  return { repo, problems, notes: bypassHidden ? ["the bypass actors were not compared: this token cannot see them (it is not an admin of the repository)"] : [] };
}
