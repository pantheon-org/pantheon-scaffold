// Compares two snapshot folders made by snapshot.ts, configuration by configuration and field by field.
//
//   command bun compare.ts <baseline dir or frozen baseline.json> <candidate dir>
//
// A frozen baseline (made by freeze.ts) holds the exit codes, the files and the manifest hash only, and is compared with the same fields of the candidate.
//
// Exits 1 on any difference and names the configuration, the field and the first differing line.
// stderr is compared after the line numbers and source snippets of pkl error messages are removed (allowed to differ).
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const [a, b] = Bun.argv.slice(2).map((p) => resolve(p));
if (!a || !b) {
  console.error("usage: bun compare.ts <baseline dir> <candidate dir>");
  process.exit(2);
}
const names = (dir: string) => readdirSync(dir).filter((f) => f.endsWith(".json")).sort();
const load = (dir: string, f: string) => JSON.parse(readFileSync(join(dir, f), "utf8"));
// Allowed to differ: the source snippets, line numbers and stack frames ("at <module>#<member> (file://..., line N)") of pkl errors,
// because a builtin that moves into a folder changes its module name, path and line. The error message itself is compared.
const loose = (text: string) =>
  text.replace(/^\s*\d+ \|.*$/gm, "").replace(/^\s*at .*\(.*\.pkl, line \d+\)\s*$/gm, "").replace(/line \d+/g, "line N").replace(/^\s*\^+\s*$/gm, "");
const firstDiff = (x: string, y: string) => {
  const l = x.split("\n"), m = y.split("\n");
  for (let i = 0; i < Math.max(l.length, m.length); i++) if (l[i] !== m[i]) return `line ${i + 1}\n    baseline:  ${JSON.stringify(l[i] ?? "(none)").slice(0, 300)}\n    candidate: ${JSON.stringify(m[i] ?? "(none)").slice(0, 300)}`;
  return "(equal)";
};

const problems: string[] = [];
if (statSync(a).isFile()) {
  const frozen = JSON.parse(readFileSync(a, "utf8")) as Record<string, { codes: number[]; files: string[]; manifest: string }>;
  const nc = names(b).map((n) => n.replace(/\.json$/, "")).sort();
  const fk = Object.keys(frozen).sort();
  if (JSON.stringify(fk) !== JSON.stringify(nc)) problems.push(`configuration sets differ: only in baseline ${fk.filter((n) => !nc.includes(n))}, only in candidate ${nc.filter((n) => !fk.includes(n))}`);
  for (const n of fk.filter((k) => nc.includes(k))) {
    const y = load(b, `${n}.json`), x = frozen[n]!;
    const got = [y.contract.code, y.render.code, y.check.code];
    if (JSON.stringify(x.codes) !== JSON.stringify(got)) problems.push(`${n}: exit codes ${x.codes} vs ${got}`);
    if (JSON.stringify(x.files) !== JSON.stringify(y.files)) {
      const sa = new Set<string>(x.files), sb = new Set<string>(y.files);
      problems.push(`${n}: files differ: only in baseline ${[...sa].filter((v) => !sb.has(v)).slice(0, 5).join(" | ")}; only in candidate ${[...sb].filter((v) => !sa.has(v)).slice(0, 5).join(" | ")}`);
    }
    if (x.manifest !== createHash("sha256").update(y.manifest).digest("hex")) problems.push(`${n}: manifest differs`);
  }
  console.log(`${fk.length} configurations compared with the frozen baseline, ${problems.length} differences`);
  if (problems.length > 0) {
    console.error(problems.join("\n"));
    process.exit(1);
  }
  process.exit(0);
}
const na = names(a), nb = names(b);
if (JSON.stringify(na) !== JSON.stringify(nb)) problems.push(`configuration sets differ: only in baseline ${na.filter((n) => !nb.includes(n))}, only in candidate ${nb.filter((n) => !na.includes(n))}`);
let compared = 0;
for (const f of na.filter((n) => nb.includes(n))) {
  const x = load(a, f), y = load(b, f);
  compared++;
  for (const step of ["contract", "render", "check"] as const) {
    if (x[step].code !== y[step].code) problems.push(`${f}: ${step} exit ${x[step].code} vs ${y[step].code}`);
    if (x[step].out !== y[step].out) problems.push(`${f}: ${step} stdout differs at ${firstDiff(x[step].out, y[step].out)}`);
    if (loose(x[step].err) !== loose(y[step].err)) problems.push(`${f}: ${step} stderr differs at ${firstDiff(loose(x[step].err), loose(y[step].err))}`);
  }
  if (JSON.stringify(x.files) !== JSON.stringify(y.files)) {
    const sa = new Set<string>(x.files), sb = new Set<string>(y.files);
    problems.push(`${f}: files differ: only in baseline ${[...sa].filter((v) => !sb.has(v)).slice(0, 5).join(" | ")}; only in candidate ${[...sb].filter((v) => !sa.has(v)).slice(0, 5).join(" | ")}`);
  }
  if (x.manifest !== y.manifest) problems.push(`${f}: manifest differs at ${firstDiff(x.manifest, y.manifest)}`);
}
console.log(`${compared} configurations compared, ${problems.length} differences`);
if (problems.length > 0) {
  console.error(problems.join("\n"));
  process.exit(1);
}
