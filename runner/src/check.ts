import { existsSync, readFileSync, statSync } from "node:fs";
import { finalContent, shortDiff } from "./apply";
import type { Contract } from "./contract";
import { resolveInside } from "./paths";

export type Drift = { path: string; reason: "missing" | "different" | "mode"; diff: string };

/** Compares the render with the files on disk and writes nothing. Seed and external files are skipped (decision 59, task 2.3). */
export function check(contract: Contract, root: string): { drift: Drift[]; checked: number } {
  const drift: Drift[] = [];
  let checked = 0;
  for (const f of contract.files) {
    if (f.class !== "owned") continue;
    checked++;
    const abs = resolveInside(root, f.path);
    const expected = finalContent(f, contract.pins);
    if (!existsSync(abs)) {
      drift.push({ path: f.path, reason: "missing", diff: "" });
      continue;
    }
    const actual = readFileSync(abs, "utf8");
    if (actual !== expected) {
      drift.push({ path: f.path, reason: "different", diff: shortDiff(actual, expected) });
      continue;
    }
    const wantExec = f.mode === "0755";
    const isExec = (statSync(abs).mode & 0o111) !== 0;
    if (wantExec !== isExec)
      drift.push({ path: f.path, reason: "mode", diff: `    expected ${wantExec ? "executable" : "not executable"}, found ${isExec ? "executable" : "not executable"}` });
  }
  return { drift, checked };
}
