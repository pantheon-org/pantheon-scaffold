/** A rule as GitHub reports it and as the create call takes it. */
export type Rule = { type: string; parameters?: Record<string, unknown> };
export type BypassActor = { actor_id: number | null; actor_type: string; bypass_mode: string };
export type Conditions = { ref_name?: { include?: string[]; exclude?: string[] } } & Record<string, unknown>;
/** The create shape of a ruleset: what is sent to `POST /repos/{owner}/{repo}/rulesets`. */
export type Ruleset = { name: string; target: string; enforcement: string; bypass_actors: BypassActor[]; conditions: Conditions; rules: Rule[] };

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** Removes nulls and sorts keys, recursively, so two equal rulesets serialise equally. */
function canonical(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(canonical);
  if (!isObject(v)) return v;
  return Object.fromEntries(
    Object.entries(v)
      .filter(([, x]) => x !== null && x !== undefined)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
      .map(([k, x]) => [k, canonical(x)]),
  );
}

const byJson = (a: unknown, b: unknown): number => {
  const x = JSON.stringify(a);
  const y = JSON.stringify(b);
  return x < y ? -1 : x > y ? 1 : 0;
};

/**
 * The create shape of a ruleset read with GET. It is built from the six fields the create call takes, so everything else a GET carries
 * (`id`, `node_id`, `_links`, `source`, the dates and any field GitHub adds later) is left out. Nulls are dropped (an `integration_id` that
 * is null means "any app"), and everything that is a set is sorted, so the order GitHub happens to return does not matter.
 */
export function toCreateShape(got: Record<string, unknown>): Ruleset {
  const body = got;
  for (const k of ["name", "target", "enforcement", "rules"]) if (!(k in body)) throw new Error(`a ruleset needs a "${k}"`);
  const rules = (body.rules as Rule[]).map((r) => {
    const parameters = r.parameters === undefined ? undefined : (canonical(r.parameters) as Record<string, unknown>);
    if (parameters && Array.isArray(parameters.required_status_checks)) parameters.required_status_checks = [...(parameters.required_status_checks as unknown[])].sort(byJson);
    return parameters && Object.keys(parameters).length ? { type: r.type, parameters } : { type: r.type };
  });
  const conditions = canonical(body.conditions ?? {}) as Conditions;
  for (const k of ["include", "exclude"] as const) if (conditions.ref_name?.[k]) conditions.ref_name[k] = [...conditions.ref_name[k]!].sort();
  return {
    name: body.name as string,
    target: body.target as string,
    enforcement: body.enforcement as string,
    bypass_actors: ((body.bypass_actors ?? []) as BypassActor[]).map((b) => canonical(b) as BypassActor).sort(byJson),
    conditions,
    rules: rules.sort((a, b) => (a.type < b.type ? -1 : a.type > b.type ? 1 : 0)),
  };
}

/** The contexts of every required status check in a ruleset. */
export function requiredChecks(r: Ruleset): string[] {
  return r.rules.filter((x) => x.type === "required_status_checks").flatMap((x) => ((x.parameters?.required_status_checks ?? []) as { context: string }[]).map((c) => c.context)).sort();
}

const same = (a: unknown, b: unknown): boolean => JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));

/**
 * What differs between the ruleset that should hold and the one that does, in words. Empty when they are the same. A token that is not an
 * admin of the repository is never shown the bypass actors, so `skipBypass` leaves them out, and the caller says that it did.
 */
export function compareRulesets(desired: Ruleset, live: Ruleset, opts: { skipBypass?: boolean } = {}): string[] {
  const out: string[] = [];
  const d = toCreateShape({ ...desired });
  const l = toCreateShape({ ...live });
  if (d.target !== l.target) out.push(`target: should be ${d.target}, is ${l.target}`);
  if (d.enforcement !== l.enforcement) out.push(`enforcement: should be ${d.enforcement}, is ${l.enforcement}`);
  if (!same(d.conditions, l.conditions)) out.push(`conditions: should be ${JSON.stringify(d.conditions)}, is ${JSON.stringify(l.conditions)}`);
  if (!opts.skipBypass && !same(d.bypass_actors, l.bypass_actors)) out.push(`bypass actors: should be ${JSON.stringify(d.bypass_actors)}, is ${JSON.stringify(l.bypass_actors)}`);
  const types = [...new Set([...d.rules.map((r) => r.type), ...l.rules.map((r) => r.type)])].sort();
  for (const t of types) {
    const dr = d.rules.find((r) => r.type === t);
    const lr = l.rules.find((r) => r.type === t);
    if (!dr) out.push(`rule ${t}: is present, should not be`);
    else if (!lr) out.push(`rule ${t}: should be present, is missing`);
    else if (!same(dr, lr)) {
      if (t === "required_status_checks") {
        const dc = requiredChecks({ ...d, rules: [dr] });
        const lc = requiredChecks({ ...l, rules: [lr] });
        const only = (a: string[], b: string[]) => a.filter((x) => !b.includes(x));
        const parts = [only(dc, lc).length ? `missing from live: ${only(dc, lc).join(", ")}` : "", only(lc, dc).length ? `extra in live: ${only(lc, dc).join(", ")}` : ""].filter(Boolean);
        out.push(`rule ${t}: ${parts.join("; ") || `parameters differ: should be ${JSON.stringify(dr.parameters)}, is ${JSON.stringify(lr.parameters)}`}`);
      } else out.push(`rule ${t}: should be ${JSON.stringify(dr.parameters ?? {})}, is ${JSON.stringify(lr.parameters ?? {})}`);
    }
  }
  return out;
}
