// Spike runner: evaluates a pkl config, then writes its files into a target directory.
// Usage: bun runner.ts <config.pkl> <target-dir>
import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

type Entry = { seed: boolean; executable: boolean };
type Manifest = Record<string, Entry>;

const [config, target] = process.argv.slice(2);
if (!config || !target) {
  console.error("usage: bun runner.ts <config.pkl> <target-dir>");
  process.exit(2);
}

const stage = join(import.meta.dir, ".stage");
rmSync(stage, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });

const pkl = Bun.spawnSync(["pkl", "eval", "-m", stage, config], { stderr: "pipe" });
if (pkl.exitCode !== 0) {
  console.error(pkl.stderr.toString());
  process.exit(1);
}

const manifest: Manifest = JSON.parse(readFileSync(join(stage, "manifest.json"), "utf8"));
const stateFile = join(target, ".pkl-gen", "manifest.json");
const previous: Manifest = existsSync(stateFile) ? JSON.parse(readFileSync(stateFile, "utf8")) : {};

const counts = { written: 0, unchanged: 0, seeded: 0, seedKept: 0, deleted: 0, chmod: 0 };

for (const [path, entry] of Object.entries(manifest)) {
  const dest = join(target, path);
  const content = readFileSync(join(stage, "files", path));
  if (entry.seed && existsSync(dest)) {
    counts.seedKept++;
    continue;
  }
  if (!existsSync(dest) || !content.equals(readFileSync(dest))) {
    mkdirSync(dirname(dest), { recursive: true });
    writeFileSync(dest, content);
    entry.seed ? counts.seeded++ : counts.written++;
  } else {
    counts.unchanged++;
  }
  if (entry.executable) {
    chmodSync(dest, 0o755);
    counts.chmod++;
  }
}

// Orphans: owned (non-seed) files recorded last run that the template no longer produces.
for (const [path, entry] of Object.entries(previous)) {
  if (!entry.seed && !(path in manifest)) {
    rmSync(join(target, path), { force: true });
    counts.deleted++;
  }
}

mkdirSync(dirname(stateFile), { recursive: true });
writeFileSync(stateFile, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(JSON.stringify(counts));
