// Shows what flipping each toggle changes in the rendered contract. Run from the repository root: bun tmp/phase-5-wave-c/diff-toggles.ts
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { evalContractJson } from "../../runner/src/pkl";
const root = resolve(import.meta.dir, "../..");
const dir = mkdtempSync(join(tmpdir(), "toggles-"));
type T = { skills: boolean; binary: boolean; features: "default" | "all"; compositeAction: boolean };
const BASE: T = { skills: true, binary: true, features: "default", compositeAction: false };
async function render(t: T): Promise<Map<string, string>> {
  const f = join(dir, `${JSON.stringify(t).replace(/[^a-z]/g, "")}.pkl`);
  writeFileSync(f, `amends "file://${root}/pkl/Config.pkl"\nimport "file://${root}/pkl/Builtins.pkl"\noptions {\n  toolchain = "1.96.1"\n  skillName = "demo-skill"\n  crateName = "demo"\n  skills = ${t.skills}\n  binary = ${t.binary}\n  features = "${t.features}"\n  compositeAction = ${t.compositeAction}\n}\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles { ...Builtins.standardFiles(options) }\n`);
  const c = (await evalContractJson(f, root, { allowFilesUnder: [join(root, "pkl")] })) as any;
  return new Map(c.files.map((x: any) => [x.path, `${x.class}|${x.mode}|${x.content ?? ""}`]));
}
const base = await render(BASE);
for (const [k, v] of [["skills", false], ["binary", false], ["features", "all"], ["compositeAction", true]] as const) {
  const other = await render({ ...BASE, [k]: v } as T);
  const removed = [...base.keys()].filter((p) => !other.has(p));
  const added = [...other.keys()].filter((p) => !base.has(p));
  const changed = [...base.keys()].filter((p) => other.has(p) && other.get(p) !== base.get(p));
  console.log(`${k} = ${v}\n  removed: ${removed.join(", ") || "-"}\n  added:   ${added.join(", ") || "-"}\n  changed: ${changed.join(", ") || "-"}`);
}
