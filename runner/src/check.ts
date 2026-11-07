import { existsSync, readFileSync } from "node:fs";
import { finalContent, shortDiff } from "./apply";
import type { Contract } from "./contract";
import { resolveInside } from "./paths";

export type Drift = { path: string; reason: "missing" | "different"; diff: string };

/** Compares the render with the files on disk and writes nothing. Seed and external files are skipped (decision 59, task 2.3). */
export function check(contract: Contract, root: string): { drift: Drift[]; checked: number } {
  const drift: Drift[] = [];
  let checked = 0;
  for (const f of contract.files) {
    if (f.class !== "owned") continue;
    checked++;
    const abs = resolveInside(root, f.path);
    const expected = finalContent(f, contract.pins);
    if (!existsSync(abs)) drift.push({ path: f.path, reason: "missing", diff: "" });
    else {
      const actual = readFileSync(abs, "utf8");
      if (actual !== expected) drift.push({ path: f.path, reason: "different", diff: shortDiff(actual, expected) });
    }
  }
  return { drift, checked };
}
