// Turns a snapshot folder made by snapshot.ts into the frozen snapshot that CI compares against: for each configuration the exit codes of
// contract, render and check, every file written (path, mode, sha256) and the sha256 of the manifest. Messages are left out, because a
// message may change when the runner changes; what a repository gets must not.
//
//   command bun freeze.ts <snapshot dir> > baseline.json
import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const dir = resolve(Bun.argv[2] ?? "");
if (!Bun.argv[2]) {
  console.error("usage: bun freeze.ts <snapshot dir> > baseline.json");
  process.exit(2);
}
const out: Record<string, unknown> = {};
for (const f of readdirSync(dir).filter((x) => x.endsWith(".json")).sort()) {
  const x = JSON.parse(readFileSync(join(dir, f), "utf8"));
  out[f.replace(/\.json$/, "")] = {
    codes: [x.contract.code, x.render.code, x.check.code],
    files: x.files,
    manifest: createHash("sha256").update(x.manifest).digest("hex"),
  };
}
console.log(JSON.stringify(out, null, 1));
