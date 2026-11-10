// Semantic YAML comparison. Usage: bun compare.ts <expected.yml> <actual.yml>
// Prints each differing path and exits with the number of differences.
import { readFileSync } from "node:fs";

const [expectedPath, actualPath] = process.argv.slice(2);
if (!expectedPath || !actualPath) {
  console.error("usage: bun compare.ts <expected.yml> <actual.yml>");
  process.exit(2);
}

const expected = Bun.YAML.parse(readFileSync(expectedPath, "utf8"));
const actual = Bun.YAML.parse(readFileSync(actualPath, "utf8"));

const diffs: string[] = [];
const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

function walk(path: string, a: unknown, b: unknown): void {
  if (Array.isArray(a) && Array.isArray(b)) {
    const n = Math.max(a.length, b.length);
    for (let i = 0; i < n; i++) walk(`${path}[${i}]`, a[i], b[i]);
  } else if (isObject(a) && isObject(b)) {
    for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) walk(`${path}.${k}`, a[k], b[k]);
  } else if (JSON.stringify(a) !== JSON.stringify(b)) {
    diffs.push(`${path}: expected ${JSON.stringify(a)}, got ${JSON.stringify(b)}`);
  }
}

walk("$", expected, actual);
for (const d of diffs) process.stdout.write(`DIFF ${d}\n`);
if (diffs.length === 0) process.stdout.write("semantically equal\n");
process.exit(diffs.length);
