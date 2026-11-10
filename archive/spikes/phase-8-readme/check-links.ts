// Checks docs/adopting.md: every file it names exists, every relative link resolves, and none of the names on a private list appears (the list is not in this repository).
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
// The names this repository must never publish are not in this file. They are read from a private, untracked list (one per line),
// by default .context/private-names.txt, or the file named in PRIVATE_NAMES_FILE. Without that list the name check is skipped, loudly.
const namesFile = process.env.PRIVATE_NAMES_FILE ?? resolve(root, ".context/private-names.txt");
if (existsSync(namesFile)) {
  for (const n of readFileSync(namesFile, "utf8").split("\n").map((l) => l.trim()).filter(Boolean)) if (text.includes(n)) problems.push("names something that must stay private (see the private list)");
} else console.log(`SKIPPED the name check: no private list at ${namesFile}`);
console.log(`${named.size} files and links checked`);
if (problems.length) {
  console.log(problems.join("\n"));
  process.exit(1);
}
console.log("ok");
