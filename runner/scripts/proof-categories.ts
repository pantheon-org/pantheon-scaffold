// Phase 7: the categories of the proof report. Every difference between a rendered file and the repository's own copy is assigned one of
// them by the rules below, which were written before the proof was run. A difference no rule names is UNCATEGORISED and fails the proof.
import { compareDirectives, compareWorkflows } from "../src/equality";

export type Input = {
  repo: string;
  path: string;
  /** The repository's own file, absent when it has none. */
  theirs?: string;
  mine: string;
  theirsMode?: string;
  mode: string;
  /** Lines added and removed in the repository's clone, for the files the rule judges by size. */
  numstat?: { added: number; removed: number };
};
export type Result = { category: string; detail: string };

const NEW_FILES = ["hk.shared.pkl", ".mise/conf.d/repo.toml", ".github/workflows/dist-check.yml", ".github/workflows/scaffold-check.yml"];
const unpin = (text: string): string => text.replace(/@[0-9a-f]{40}\b/g, "@<pin>");
const pinsOf = (text: string): string[] => [...text.matchAll(/uses:\s*([^@\s]+)@([0-9a-f]{40})/g)].map((m) => `${m[1]}@${m[2]}`);
const lines = (t: string): string[] => t.split("\n").filter((l) => l !== "");
const isYaml = (p: string): boolean => /\.ya?ml$/.test(p);
const result = (category: string, detail: string): Result => ({ category, detail });
const uncategorised = (detail: string): Result => result("UNCATEGORISED", detail);

function gitignore(theirs: string, mine: string): Result | undefined {
  const a = lines(theirs), b = lines(mine);
  const added = b.filter((l) => !a.includes(l)), removed = a.filter((l) => !b.includes(l));
  const okAdded = added.every((l) => l === ".pkl-gen/cache/" || l === "target/");
  const okRemoved = removed.every((l) => l === "/target");
  return okAdded && okRemoved ? result("recorded difference: decisions 74 and 87", `added ${added.join(", ") || "nothing"}, removed ${removed.join(", ") || "nothing"}`) : undefined;
}

function distWorkspace(theirs: string, mine: string): Result | undefined {
  const a = theirs.split("\n"), b = mine.split("\n");
  if (a.length !== b.length) return undefined;
  const changed = a.map((l, i) => (l === b[i] ? -1 : i)).filter((i) => i >= 0);
  const only = changed.every((i) => /^"actions\/(upload|download)-artifact" = /.test(a[i]) && /^"actions\/(upload|download)-artifact" = /.test(b[i]));
  return only && changed.length > 0 ? result("recorded difference: decision 85", `${changed.length} action commit line(s)`) : undefined;
}

function dependabot(theirs: string, mine: string): Result | undefined {
  const c = compareWorkflows(unpin(theirs), unpin(mine));
  if (c.duplicateKeys.length) return undefined;
  const only = c.differences.length > 0 && c.differences.every((d) => d.startsWith("$.updates[1].ignore"));
  return only ? result("recorded difference: decision 86", c.differences.join("; ")) : undefined;
}

function security(repo: string, theirs: string, mine: string): Result | undefined {
  if (repo !== "pantheon-skill-auditor") return undefined;
  const a = lines(theirs), b = lines(mine);
  const differing = [...a.filter((l) => !b.includes(l)), ...b.filter((l) => !a.includes(l))];
  return differing.length > 0 && differing.every((l) => l.startsWith("- The ")) ? result("recorded difference: seeded file, the repository's own scope wording", `${differing.length} scope line(s)`) : undefined;
}

export function categorise(i: Input): Result {
  if (i.theirs === undefined) return NEW_FILES.includes(i.path) ? result("new file", "a file the package adds") : uncategorised("the repository has no such file and the package is not known to add it");
  if (i.theirs === i.mine) {
    if (i.theirsMode === i.mode) return result("identical", "");
    return i.path === ".github/scripts/sync-labels.sh" ? result("mode change only (decision 79)", `${i.theirsMode} to ${i.mode}`) : uncategorised(`only the mode differs: ${i.theirsMode} to ${i.mode}`);
  }
  if (i.path === "hk.pkl") return result("migration: hk.pkl replaced by the seeded one (decision 31)", "");
  if (i.path === "mise.toml") {
    const n = i.numstat;
    return n && n.added === 10 && n.removed === 4 ? result("migration: mise.toml, 10 lines added and 4 removed", "") : uncategorised(`mise.toml changed by ${n ? `${n.added} added and ${n.removed} removed` : "an unknown number of lines"}, expected 10 and 4`);
  }
  const recorded =
    i.path === ".gitignore" ? gitignore(i.theirs, i.mine)
    : i.path === "dist-workspace.toml" ? distWorkspace(i.theirs, i.mine)
    : i.path === ".github/dependabot.yml" ? dependabot(i.theirs, i.mine)
    : i.path === "SECURITY.md" ? security(i.repo, i.theirs, i.mine)
    : undefined;
  if (recorded) return recorded;
  if (isYaml(i.path)) {
    const c = compareWorkflows(unpin(i.theirs), unpin(i.mine));
    if (c.equal && c.duplicateKeys.length === 0 && compareDirectives(i.theirs, i.mine).length === 0) {
      const moved = pinsOf(i.mine).filter((p) => !pinsOf(i.theirs!).includes(p));
      return moved.length ? result("pin moved", moved.map((p) => p.replace(/@([0-9a-f]{7})[0-9a-f]+$/, "@$1")).join(", ")) : result("comments and formatting only (P.1)", "");
    }
    return uncategorised([...c.differences, ...compareDirectives(i.theirs, i.mine).map((d) => `directive: ${d}`)].slice(0, 3).join("; ") || "differs");
  }
  return uncategorised("bytes differ");
}
