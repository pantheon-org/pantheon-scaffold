// Mutation check for the Phase 7 upgrade fix: each entry breaks one property of how upgrade rewrites the imports of the package.
// Run from the repository root: bun tmp/phase-7-upgrade-imports/mutate.ts [text]
import { $ } from "bun";

const M: [string, string, string, string][] = [
  ["runner/src/pins.ts", `\n    .replace(PACKAGE_IMPORT, (_m, p1, _v, p3, _v2, p5) => \`\${p1}\${next.version}\${p3}\${next.version}::sha256:\${next.checksum}\${p5}\`);`, `;`, "imports of the package are rewritten"],
  ["runner/src/pins.ts", `(import\\*? "package:`, `(import "package:`, "an import* is rewritten too"],
  ["runner/src/pins.ts", `github\\.com\\/pantheon-org\\/pantheon-scaffold\\/releases\\/download\\/v)([^/"]+)(\\/pantheon-scaffold@)([^:#"]+)(?:::sha256:[0-9a-f]{64})?(#\\/[^"]+")/g;`, `github\\.com\\/[^/]+\\/[^/]+\\/releases\\/download\\/v)([^/"]+)(\\/[^@]+@)([^:#"]+)(?:::sha256:[0-9a-f]{64})?(#\\/[^"]+")/g;`, "an import of another package is left alone"],
  ["runner/src/pins.ts", `.replace(PACKAGE_IMPORT, (_m, p1, _v, p3, _v2, p5) => \`\${p1}\${next.version}\${p3}\${next.version}::sha256:\${next.checksum}\${p5}\`)`, `.replace(PACKAGE_IMPORT, (_m, p1, _v, p3, _v2, p5) => \`\${p1}\${next.version}\${p3}\${next.version}\${p5}\`)`, "a moved import carries the new checksum"],
  ["runner/src/pins.ts", `(?:::sha256:[0-9a-f]{64})?(#\\/[^"]+")/g;\nconst TOOL_STRING`, `::sha256:[0-9a-f]{64}(#\\/[^"]+")/g;\nconst TOOL_STRING`, "an import with no checksum yet is moved"],
  ["runner/src/upgrade.ts", `\${imports ? \`, \${imports} import\${imports === 1 ? "" : "s"} of the package\` : ""}`, ``, "the report says how many imports move"],
];
const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
const only = Bun.argv[2];
for (const [file, from, to, what] of M) {
  if (only && !what.includes(only)) continue;
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/pins.test.ts test/upgrade.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
