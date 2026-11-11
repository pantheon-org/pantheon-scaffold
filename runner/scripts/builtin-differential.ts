// Renders every fixture configuration (runner/test/fixtures/*/pantheon.pkl, all of which build on the Builtins) with the
// TypeScript runner and with a Rust binary, each into a fresh directory, and compares what they write.
//
//   bun runner/scripts/builtin-differential.ts <path to the Rust binary>
//
// Compared, per fixture: the exit code, stdout and stderr (the directory and the checkout path replaced), the `contract`
// output, every file written (path, mode and bytes), the manifest (parsed, keys sorted, so only its content counts, not
// its field order), and a `check` straight after the render. Nothing is allowed to differ besides the field order of
// the manifest. Exits 1 on any difference and says which fixture and what.
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";

const checkout = resolve(import.meta.dir, "../..");
const rust = resolve(Bun.argv[2] ?? "target/debug/pantheon-scaffold");
const runners: Record<string, string[]> = { ts: ["bun", join(checkout, "runner/src/cli.ts")], rust: [rust] };
const fixtures = join(checkout, "runner/test/fixtures");

type Run = { code: number; out: string; err: string };
const normalise = (text: string, dir: string) => text.replaceAll(dir, "<dir>").replaceAll(checkout, "<checkout>");
const run = (runner: string, args: string[], dir: string): Run => {
  const result = Bun.spawnSync([...runners[runner]!, ...args], { cwd: dir, env: { ...process.env, CI: "" } });
  return { code: result.exitCode, out: normalise(result.stdout.toString(), dir), err: normalise(result.stderr.toString(), dir) };
};

/** Every file under the directory except the runner's own scratch state, as "path mode sha256". */
const tree = (dir: string): string[] => {
  const found: string[] = [];
  const walk = (current: string) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const full = join(current, entry.name);
      const rel = relative(dir, full);
      if (rel === ".pkl-gen/stage" || rel === ".pkl-gen/lock" || rel === ".pkl-gen/cache") continue;
      if (entry.isDirectory()) walk(full);
      else found.push(`${rel} ${(statSync(full).mode & 0o777).toString(8)} ${createHash("sha256").update(readFileSync(full)).digest("hex")}`);
    }
  };
  walk(dir);
  return found.sort();
};

const sortKeys = (value: unknown): unknown =>
  Array.isArray(value)
    ? value.map(sortKeys)
    : value && typeof value === "object"
      ? Object.fromEntries(Object.entries(value).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)).map(([k, v]) => [k, sortKeys(v)]))
      : value;
const manifest = (dir: string): string => {
  const path = join(dir, ".pkl-gen/manifest.json");
  return existsSync(path) ? JSON.stringify(sortKeys(JSON.parse(readFileSync(path, "utf8")))) : "(none)";
};

const names = readdirSync(fixtures)
  .filter((name) => existsSync(join(fixtures, name, "pantheon.pkl")))
  .sort();
const allow = ["--allow-local-package", join(checkout, "pkl")];
const problems: string[] = [];
let written = 0;

for (const name of names) {
  const config = join(fixtures, name, "pantheon.pkl");
  const dirs: Record<string, string> = {};
  const results: Record<string, { contract: Run; render: Run; check: Run; tree: string[]; manifest: string }> = {};
  for (const runner of Object.keys(runners)) {
    const dir = mkdtempSync(join(tmpdir(), `builtin-diff-${runner}-`));
    dirs[runner] = dir;
    const contract = run(runner, ["contract", config, ...allow], dir);
    const render = run(runner, ["render", config, "--root", dir, ...allow], dir);
    const check = run(runner, ["check", config, "--root", dir, ...allow], dir);
    results[runner] = { contract, render, check, tree: tree(dir), manifest: manifest(dir) };
  }
  const a = results.ts!;
  const b = results.rust!;
  const differ = (what: string, x: unknown, y: unknown) => {
    if (JSON.stringify(x) !== JSON.stringify(y)) problems.push(`${name}: ${what} differs\n  ts:   ${JSON.stringify(x).slice(0, 400)}\n  rust: ${JSON.stringify(y).slice(0, 400)}`);
  };
  differ("contract", a.contract, b.contract);
  differ("render", a.render, b.render);
  differ("check", a.check, b.check);
  differ("files written", a.tree, b.tree);
  differ("manifest", a.manifest, b.manifest);
  written += a.tree.length;
  console.log(`${problems.some((p) => p.startsWith(`${name}:`)) ? "DIFF" : "same"}  ${name} (${a.tree.length} files, render exit ${a.render.code})`);
  for (const dir of Object.values(dirs)) rmSync(dir, { recursive: true, force: true });
}

console.log(`\n${names.length} fixtures, ${written} files compared per runner, ${problems.length} differences`);
if (problems.length > 0) {
  console.error(`\n${problems.join("\n")}`);
  process.exit(1);
}
