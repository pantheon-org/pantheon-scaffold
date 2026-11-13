// Renders a pantheon-scaffold tree with one binary and writes what came out, one JSON file per configuration.
//
//   command bun snapshot.ts <tree root> <binary> <out dir> [--api old|new]
//
// --api names how the matrix configurations call the aggregators that gained a parameter: `new` (the default) is `toolingFiles(options)`, `old` is `toolingFiles()`, for a tree before the presence work.
// Use old for the baseline tree (origin/main at 0b466dc).
//
// Configurations: the tree's own pantheon.pkl, every runner/test/fixtures/*/pantheon.pkl, and an option matrix written into
// <out dir>/_matrix (skills, binary, compositeAction, features, plus error paths), all built on the tree's pkl/ folder.
// Per configuration: exit code, stdout and stderr of `contract`, `render` and `check`, every file written
// (path, mode, sha256) and the manifest as text. The tree path and the render directory are replaced by placeholders.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";

const argv = Bun.argv.slice(2);
const api = argv.includes("--api") ? argv[argv.indexOf("--api") + 1] : "new";
if (api !== "old" && api !== "new") {
  console.error("--api must be old or new");
  process.exit(2);
}
const [treeArg, binaryArg, outArg] = argv.filter((a, i) => a !== "--api" && argv[i - 1] !== "--api");
if (!treeArg || !binaryArg || !outArg) {
  console.error("usage: bun snapshot.ts <tree root> <binary> <out dir> [--api old|new]");
  process.exit(2);
}
const tree = resolve(treeArg);
const binary = resolve(binaryArg);
const out = resolve(outArg);
mkdirSync(out, { recursive: true });
const allow = ["--allow-local-package", join(tree, "pkl")];

type Run = { code: number; out: string; err: string };
const norm = (text: string, dir: string) => text.replaceAll(dir, "<dir>").replaceAll(out, "<out>").replaceAll(tree, "<tree>");
const run = (args: string[], dir: string): Run => {
  const r = Bun.spawnSync([binary, ...args], { cwd: dir, env: { ...process.env, CI: "" } });
  return { code: r.exitCode, out: norm(r.stdout.toString(), dir), err: norm(r.stderr.toString(), dir) };
};

const files = (dir: string): string[] => {
  const found: string[] = [];
  const walk = (current: string) => {
    for (const e of readdirSync(current, { withFileTypes: true })) {
      const full = join(current, e.name);
      const rel = relative(dir, full);
      if (rel === ".pkl-gen/stage" || rel === ".pkl-gen/lock" || rel === ".pkl-gen/cache") continue;
      if (e.isDirectory()) walk(full);
      else found.push(`${rel} ${(statSync(full).mode & 0o777).toString(8)} ${createHash("sha256").update(readFileSync(full)).digest("hex")}`);
    }
  };
  walk(dir);
  return found.sort();
};

type Opts = { skills: boolean; binary: boolean; compositeAction: boolean; features: "default" | "all"; skillName: string | null; crateName: string | null; runnerVersion: string | null; ignore?: string[] };
const BASE: Opts = { skills: true, binary: true, compositeAction: false, features: "default", skillName: "demo-skill", crateName: "demo", runnerVersion: "0.1.9" };
const matrix: Record<string, Opts> = {};
for (const skills of [true, false])
  for (const bin of [true, false])
    for (const compositeAction of [false, true])
      for (const features of ["default", "all"] as const)
        matrix[`m-skills-${skills}-binary-${bin}-composite-${compositeAction}-features-${features}`] = { ...BASE, skills, binary: bin, compositeAction, features };
matrix["m-ignore"] = { ...BASE, ignore: ["serde", "tokio"] };
matrix["e-no-skill-name"] = { ...BASE, skillName: null };
matrix["e-no-crate-name"] = { ...BASE, crateName: null };
matrix["e-no-runner-version"] = { ...BASE, runnerVersion: null };

const writeMatrix = (name: string, o: Opts): string => {
  const dir = join(out, "_matrix", name);
  mkdirSync(dir, { recursive: true });
  const lines = [
    `toolchain = "1.96.1"`,
    o.skillName ? `skillName = "${o.skillName}"` : "",
    o.crateName ? `crateName = "${o.crateName}"` : "",
    o.runnerVersion ? `runnerVersion = "${o.runnerVersion}"` : "",
    `skills = ${o.skills}`,
    `binary = ${o.binary}`,
    `features = "${o.features}"`,
    `compositeAction = ${o.compositeAction}`,
    o.ignore ? `gitDependencyIgnore { ${o.ignore.map((x) => `"${x}"`).join("; ")} }` : "",
  ].filter(Boolean);
  const config = join(dir, "pantheon.pkl");
  writeFileSync(
    config,
    `amends "file://${tree}/pkl/Config.pkl"\nimport "file://${tree}/pkl/Builtins.pkl"\noptions {\n  ${lines.join("\n  ")}\n}\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles(${api === "new" ? "options" : ""})\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n`,
  );
  return config;
};

const configs: [string, string][] = [];
if (existsSync(join(tree, "pantheon.pkl"))) configs.push(["root", join(tree, "pantheon.pkl")]);
const fixtures = join(tree, "runner/test/fixtures");
for (const name of readdirSync(fixtures).sort()) {
  const c = join(fixtures, name, "pantheon.pkl");
  if (existsSync(c)) configs.push([`fixture-${name}`, c]);
}
for (const [name, o] of Object.entries(matrix)) configs.push([name, writeMatrix(name, o)]);

for (const [name, config] of configs) {
  const dir = mkdtempSync(join(tmpdir(), "builtins-snap-"));
  const contract = run(["contract", config, ...allow], dir);
  const render = run(["render", config, "--root", dir, ...allow], dir);
  const check = run(["check", config, "--root", dir, ...allow], dir);
  const manifestPath = join(dir, ".pkl-gen/manifest.json");
  const result = { contract, render, check, files: files(dir), manifest: existsSync(manifestPath) ? norm(readFileSync(manifestPath, "utf8"), dir) : "(none)" };
  writeFileSync(join(out, `${name}.json`), JSON.stringify(result, null, 1));
  rmSync(dir, { recursive: true, force: true });
  console.log(`${name}: contract ${contract.code}, render ${render.code}, check ${check.code}, ${result.files.length} files`);
}
console.log(`\n${configs.length} configurations written to ${out}`);
