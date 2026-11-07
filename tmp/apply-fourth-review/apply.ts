import { readFileSync, writeFileSync } from "node:fs";

const path = ".context/plans/2026-11-06-pantheon-projen-pkl-variant.md";
let text = readFileSync(path, "utf8");
const pairs: [string, string][] = JSON.parse(readFileSync("tmp/apply-fourth-review/pairs.json", "utf8"));
const bad: string[] = [];
for (const [o, n] of pairs) {
  const count = text.split(o).length - 1;
  if (count !== 1) bad.push(`${count} matches: ${o.slice(0, 70)}`);
}
if (bad.length) {
  console.error("No changes written. Problems:\n" + bad.join("\n"));
  process.exit(1);
}
for (const [o, n] of pairs) text = text.replace(o, () => n);
writeFileSync(path, text);
console.log(`applied ${pairs.length} replacements`);
