/** What the governance tasks need from GitHub. A read never changes anything. A write is only called after a plan, a confirmation and a written record. */
export type Gh = {
  get(path: string): Promise<unknown>;
  send(method: "PATCH" | "POST" | "PUT" | "DELETE", path: string, body?: unknown): Promise<unknown>;
};

/** The real thing, through the `gh` CLI and whatever token it holds. */
export function realGh(): Gh {
  const run = async (args: string[], input?: string): Promise<unknown> => {
    const p = Bun.spawn(["gh", "api", ...args], { stdin: input === undefined ? "ignore" : new Blob([input]), stdout: "pipe", stderr: "pipe" });
    const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
    if (code !== 0) throw new Error(`gh api ${args.join(" ")} failed (exit ${code}): ${err.trim()}`);
    return out.trim() === "" ? undefined : JSON.parse(out);
  };
  return {
    get: (path) => run([path]),
    send: (method, path, body) => run(["--method", method, path, ...(body === undefined ? [] : ["--input", "-"])], body === undefined ? undefined : JSON.stringify(body)),
  };
}

/** A repository the tasks may touch: a plain slug that is one of the governed repositories. */
export function assertGoverned(governed: string[], owner: string, repo: string): void {
  if (!/^[a-z0-9][a-z0-9._-]*$/.test(owner) || !/^[a-z0-9][a-z0-9._-]*$/.test(repo)) throw new Error(`${owner}/${repo} is not a valid repository slug`);
  if (!governed.includes(repo)) throw new Error(`${repo} is not one of the governed repositories (${governed.join(", ")})`);
}

/** Applying needs the repository written out in full, as a second look at what is about to change. */
export function assertConfirmed(owner: string, repo: string, confirm: string | undefined): void {
  if (confirm !== `${owner}/${repo}`) throw new Error(`to apply, pass --confirm ${owner}/${repo}`);
}
