// The delete guard (task 2.3, decision 47). It is evaluated once, before the runner writes anything, and it only ever
// answers "refuse, and why". git is a required tool of the runner, so a root that is not a git work tree cannot delete.

const gitEnv = (): Record<string, string> =>
  Object.fromEntries(Object.entries(process.env).filter(([k, v]) => v !== undefined && !k.startsWith("GIT_"))) as Record<string, string>;

/** Runs git in `root` with every GIT_* variable removed, so a hook or a linked worktree cannot redirect it. */
function git(root: string, args: string[]): { ok: boolean; out: string } {
  const r = Bun.spawnSync(["git", "-C", root, ...args], { env: gitEnv(), stdout: "pipe", stderr: "pipe" });
  return { ok: r.exitCode === 0, out: r.stdout.toString() };
}

/**
 * Returns the reasons deletion is refused, empty when it is allowed. The tracked tree must be clean, a sparse or partial
 * checkout counts as not clean, and a file git does not track is never deleted. A shallow checkout is fine.
 */
export function deleteGuard(root: string, paths: string[]): string[] {
  if (git(root, ["rev-parse", "--is-inside-work-tree"]).out.trim() !== "true")
    return [`not a git repository: orphan files cannot be deleted without git to confirm the tree is clean`];
  const reasons: string[] = [];
  const sparse = git(root, ["config", "--get", "core.sparseCheckout"]).out.trim() === "true";
  const partial = git(root, ["config", "--get", "extensions.partialClone"]).out.trim() !== "";
  if (sparse || partial) reasons.push("a sparse or partial checkout is treated as not clean, so nothing is deleted");
  const status = git(root, ["status", "--porcelain", "--untracked-files=no"]);
  if (!status.ok) reasons.push("git status failed, so the tree cannot be confirmed clean");
  else if (status.out.trim() !== "") reasons.push(`the tracked tree is not clean, commit or stash first:\n${status.out.trimEnd().split("\n").map((l) => `    ${l}`).join("\n")}`);
  for (const path of paths) {
    const tracked = git(root, ["ls-files", "--error-unmatch", "--", path]);
    if (!tracked.ok) reasons.push(`${path}: not tracked by git, so it is never deleted`);
  }
  return reasons;
}
