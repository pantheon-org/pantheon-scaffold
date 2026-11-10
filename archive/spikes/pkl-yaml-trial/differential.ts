// Differential check: Bun.YAML against the yaml package on the real workflows of the two proof repos (read-only gh api).
// Usage: YAML_PKG=<dir containing node_modules/yaml> bun differential.ts
import { $ } from "bun";
import { join } from "node:path";

const dir = process.env.YAML_PKG;
if (!dir) throw new Error("set YAML_PKG to the folder that holds node_modules/yaml");
const YAML = (await import(join(dir, "node_modules/yaml/dist/index.js"))).default ?? (await import(join(dir, "node_modules/yaml/dist/index.js")));
const version = JSON.parse(await Bun.file(join(dir, "node_modules/yaml/package.json")).text()).version;

const sortKeys = (v: unknown): unknown => Array.isArray(v) ? v.map(sortKeys) : v && typeof v === "object" ? Object.fromEntries(Object.entries(v as object).sort(([a], [b]) => a.localeCompare(b)).map(([k, x]) => [k, sortKeys(x)])) : v;
let same = 0, differ = 0;
for (const repo of ["pantheon-org/pantheon-adr", "pantheon-org/pantheon-skill-auditor"]) {
  const names = (await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows --jq '.[].name'`.text()).trim().split("\n");
  for (const n of names) {
    const text = Buffer.from((await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows/${n} --jq .content`.text()).trim(), "base64").toString("utf8");
    const a = JSON.stringify(sortKeys(Bun.YAML.parse(text)));
    const b = JSON.stringify(sortKeys(YAML.parse(text, { version: "1.2" })));
    if (a === b) same++; else { differ++; console.log(`DIFFER ${repo}/${n}`); }
  }
}
// Duplicate keys: the yaml package rejects them, Bun.YAML takes the last value.
const dup = "a: 1\na: 2\n";
const bunDup = JSON.stringify(Bun.YAML.parse(dup));
let pkgDup: string;
try { pkgDup = JSON.stringify(YAML.parse(dup)); } catch (e) { pkgDup = "THROWS " + (e as Error).message.split("\n")[0].slice(0, 60); }
console.log(`yaml package ${version}: ${same} real workflows parse to identical data, ${differ} differ`);
console.log(`duplicate keys "a: 1 / a: 2": Bun.YAML -> ${bunDup}; yaml package -> ${pkgDup}`);
process.exit(differ ? 1 : 0);
