// Checks docs/adopting.md: every file it names exists, every relative link resolves, and no organisation secret or variable name appears.
// usage: bun tmp/phase-8-readme/check-links.ts
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const text = readFileSync(resolve(root, "docs/adopting.md"), "utf8");
const problems: string[] = [];
const named = new Set<string>();
for (const m of text.matchAll(/`((?:runner|docs|pkl|\.github|tmp)\/[A-Za-z0-9_./@-]+|known-bad\.json)`/g)) named.add(m[1]);
for (const m of text.matchAll(/\]\(([^)#:]+\.md)\)/g)) named.add(`docs/${m[1]}`);
for (const f of named) if (!existsSync(resolve(root, f))) problems.push(`missing: ${f}`);
for (const n of ["CEREBRAS", "GEMINI_API", "GEMINI_CLI", "GEMINI_MODEL", "MISTRAL", "NPM_TOKEN", "TESSL"]) if (text.includes(n)) problems.push(`names a secret or variable: ${n}`);
console.log(`${named.size} files and links checked`);
if (problems.length) {
  console.log(problems.join("\n"));
  process.exit(1);
}
console.log("ok");
