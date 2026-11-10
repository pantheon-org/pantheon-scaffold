// Mutation check of runner/src/promote.ts: each mutation removes one guard, and the tests must then fail. The file is always restored.
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const src = resolve(root, "runner/src/promote.ts");
const original = readFileSync(src, "utf8");
const mutations: [string, string, string][] = [
  ["run from main", 'if (ref !== "refs/heads/main") fail(', "if (false) fail("],
  ["tag shape", "if (!m || !parseSemver(m[1])) fail(", "if (false) fail("],
  ["draft", "if (info.isDraft) fail(", "if (false) fail("],
  ["asset set", "if (JSON.stringify(got) !== JSON.stringify(expected)) fail(", "if (false) fail("],
  ["attestation", ".catch((e: Error) => fail(", ".catch((e: Error) => String(e) && undefined && fail("],
  ["digest", "if (sha256(bytes) !== findChecksum(text, name)) fail(", "if (false) fail("],
  ["already stable", "if (!before.isPrerelease) fail(", "if (false) fail("],
  ["known bad", "if (bad) fail(", "if (false) fail("],
  ["no run", "if (!run) fail(", "if (false) fail("],
  ["unsuccessful run", "if (unsuccessful.length) fail(", "if (false) fail("],
  ["no verify job", "if (!run!.jobs.some(", "if (false && !run!.jobs.some("],
  ["dry run returns early", "if (opts.dryRun) return report;", ""],
  ["still a pre-release", "if (after.isPrerelease) fail(", "if (false) fail("],
  ["checksums changed", "if (!same(attested, afterChecksums)) fail(", "if (false) fail("],
  ["not latest", "if (latest !== tag) fail(", "if (false) fail("],
];
let caught = 0;
try {
  for (const [name, from, to] of mutations) {
    if (!original.includes(from)) throw new Error(`mutation "${name}": text not found`);
    writeFileSync(src, original.replace(from, to));
    const r = Bun.spawnSync(["bun", "test", "test/promote.test.ts"], { cwd: resolve(root, "runner"), stdout: "pipe", stderr: "pipe" });
    const failed = r.exitCode !== 0;
    if (failed) caught++;
    console.log(`${failed ? "caught  " : "MISSED  "} ${name}`);
  }
} finally {
  writeFileSync(src, original);
}
console.log(`${caught} of ${mutations.length} caught`);
process.exit(caught === mutations.length ? 0 : 1);
