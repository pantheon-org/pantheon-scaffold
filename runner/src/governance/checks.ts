type Job = { name?: string; uses?: string; strategy?: { matrix?: Record<string, unknown> } };

/** The matrix variables that take a plain list of values, in the order they were written. */
function matrixLists(job: Job): [string, string[]][] {
  const m = job.strategy?.matrix ?? {};
  if (typeof m !== "object" || "include" in m || "exclude" in m) throw new Error("a matrix with include or exclude is not supported");
  return Object.entries(m).map(([k, v]) => {
    if (!Array.isArray(v)) throw new Error(`matrix.${k} is not a plain list`);
    return [k, v.map(String)];
  });
}

/**
 * The names GitHub reports as status checks for the jobs of one workflow: the job `name:` when it is set, otherwise the job id, and for a
 * matrix job either the name with its matrix values filled in or `name (value, value)`. Anything this cannot know (a reusable workflow, an
 * expression other than `matrix.x`) is an error, so a check name is never guessed.
 */
export function checkNames(workflowYaml: string): string[] {
  const wf = Bun.YAML.parse(workflowYaml) as { jobs?: Record<string, Job> } | null;
  const names: string[] = [];
  for (const [id, job] of Object.entries(wf?.jobs ?? {})) {
    if (job.uses) throw new Error(`job ${id} calls a reusable workflow: its check names are "caller / callee" and are not known here`);
    const lists = job.strategy?.matrix ? matrixLists(job) : [];
    const name = job.name ?? id;
    const expressions = [...name.matchAll(/\$\{\{\s*([^}]+?)\s*\}\}/g)].map((m) => m[1]);
    for (const e of expressions) if (!/^matrix\.[A-Za-z0-9_-]+$/.test(e)) throw new Error(`job ${id}: the name uses "${e}", which is not known without running the workflow`);
    if (!lists.length) {
      if (expressions.length) throw new Error(`job ${id}: the name uses a matrix but the job has none`);
      names.push(name);
      continue;
    }
    const combos = lists.reduce<Record<string, string>[]>((acc, [k, vals]) => acc.flatMap((a) => vals.map((v) => ({ ...a, [k]: v }))), [{}]);
    for (const c of combos) {
      if (expressions.length) names.push(name.replace(/\$\{\{\s*matrix\.([A-Za-z0-9_-]+)\s*\}\}/g, (_, k) => c[k] ?? ""));
      else names.push(`${name} (${Object.values(c).join(", ")})`);
    }
  }
  return names;
}

/** Required checks that no job produces (these block every pull request), and jobs that nothing requires. */
export function compareRequired(required: string[], offered: string[]): { missing: string[]; unrequired: string[] } {
  return {
    missing: [...new Set(required)].filter((r) => !offered.includes(r)).sort(),
    unrequired: [...new Set(offered)].filter((o) => !required.includes(o)).sort(),
  };
}
