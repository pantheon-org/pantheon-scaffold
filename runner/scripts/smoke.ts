// Smoke suite for a compiled runner binary. It runs the binary from a directory unrelated to the source tree, so anything the
// binary needs (the schema, the version) has to be inside it. It needs pkl and git on the PATH and a checkout of this repository
// for the pkl package and the fixtures. Run it on every target: the filesystem it runs on is part of the test (case-sensitive on
// Linux, case-insensitive on a default macOS volume).
// usage: bun runner/scripts/smoke.ts <binary> [--version <x.y.z>] [--repo <checkout>]
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const argv = Bun.argv.slice(2);
const flag = (name: string, fallback?: string): string => {
  const i = argv.indexOf(name);
  const v = i === -1 ? fallback : argv[i + 1];
  if (v === undefined) {
    console.error(`usage: bun runner/scripts/smoke.ts <binary> [--version <x.y.z>] [--repo <checkout>]`);
    process.exit(2);
  }
  return v;
};
const binary = resolve(argv[0] ?? flag("--binary"));
const expectedVersion = flag("--version", "0.0.0");
const repoRoot = resolve(flag("--repo", join(import.meta.dir, "../..")));
const fixture = (name: string) => join(repoRoot, "runner/test/fixtures", name, "pantheon.pkl");
const pkg = join(repoRoot, "pkl");

const base = mkdtempSync(join(tmpdir(), "scaffold-smoke-"));
const fresh = (name: string): string => {
  const dir = join(base, name);
  mkdirSync(dir);
  return dir;
};
const cleanEnv = (): Record<string, string> => Object.fromEntries(Object.entries(process.env).filter(([k, v]) => v !== undefined && !k.startsWith("GIT_"))) as Record<string, string>;

type Result = { code: number; out: string; err: string };
const bin = async (args: string[], cwd = base): Promise<Result> => {
  const p = Bun.spawn([binary, ...args], { cwd, stdout: "pipe", stderr: "pipe", env: cleanEnv() });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { code, out: out.trim(), err: err.trim() };
};
const render = (config: string, repo: string, ...extra: string[]) => bin(["render", fixture(config), "--root", repo, "--allow-local-package", pkg, ...extra]);
const check = (config: string, repo: string) => bin(["check", fixture(config), "--root", repo, "--allow-local-package", pkg]);
const git = (dir: string, ...args: string[]): void => {
  const r = Bun.spawnSync(["git", "-C", dir, "-c", "user.name=smoke", "-c", "user.email=smoke@example.invalid", "-c", "commit.gpgsign=false", ...args], { env: cleanEnv(), stdout: "pipe", stderr: "pipe" });
  if (r.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${r.stderr.toString()}`);
};
const commitAll = (dir: string): void => {
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", "snapshot");
};
const mode = (path: string): string => (statSync(path).mode & 0o777).toString(8);

// Case-sensitive or not: write one name and look for it in another case.
const probe = fresh("probe");
writeFileSync(join(probe, "probe-file"), "x");
const caseInsensitive = existsSync(join(probe, "PROBE-FILE"));

let failures = 0;
const ok = (name: string, pass: boolean, detail = ""): void => {
  console.log(`${pass ? "PASS" : "FAIL"}  ${name}${!pass && detail ? `\n      ${detail.replaceAll("\n", "\n      ")}` : ""}`);
  if (!pass) failures++;
};
const show = (r: Result): string => `exit ${r.code}\nstdout: ${r.out}\nstderr: ${r.err}`;

console.log(`binary ${binary}\nfilesystem: ${caseInsensitive ? "case-insensitive" : "case-sensitive"}, platform ${process.platform}-${process.arch}\n`);

{
  const r = await bin(["--version"]);
  ok("--version prints the version the binary was built for", r.code === 0 && r.out === expectedVersion, show(r));
  const u = await bin(["nope"]);
  ok("an unknown command is a usage error (exit 2)", u.code === 2 && u.err.includes("usage"), show(u));
  const c = await bin(["contract", fixture("minimal"), "--allow-local-package", pkg]);
  ok("contract works from an unrelated directory, so the schema is inside the binary", c.code === 0 && c.out.startsWith("contract ok: 4 files"), show(c));
}

{
  const repo = fresh("render");
  const r = await render("minimal", repo);
  ok("render writes the files", r.code === 0 && existsSync(join(repo, "rust-toolchain.toml")) && existsSync(join(repo, ".pkl-gen/manifest.json")), show(r));
  ok("the executable bit is set from the contract", existsSync(join(repo, ".github/scripts/hello.sh")) && mode(join(repo, ".github/scripts/hello.sh")) === "755", existsSync(join(repo, ".github/scripts/hello.sh")) ? mode(join(repo, ".github/scripts/hello.sh")) : "file missing");
  const again = await render("minimal", repo);
  ok("a second render writes and deletes nothing", again.code === 0 && again.out.includes("written 0 file(s), deleted 0"), show(again));
  const c = await check("minimal", repo);
  ok("check finds no drift", c.code === 0, show(c));
  writeFileSync(join(repo, "rust-toolchain.toml"), "# mine\n");
  const drift = await check("minimal", repo);
  ok("check reports a hand edit as drift (exit 4)", drift.code === 4, show(drift));
  const refused = await render("minimal", repo);
  ok("render refuses to overwrite a hand edit (exit 3) and leaves it", refused.code === 3 && readFileSync(join(repo, "rust-toolchain.toml"), "utf8") === "# mine\n", show(refused));
  const forced = await render("minimal", repo, "--force");
  ok("--force overwrites it", forced.code === 0 && readFileSync(join(repo, "rust-toolchain.toml"), "utf8") !== "# mine\n", show(forced));
  const d = await bin(["detach", "--root", repo]);
  ok("detach removes .pkl-gen and leaves the files", d.code === 0 && !existsSync(join(repo, ".pkl-gen")) && existsSync(join(repo, "rust-toolchain.toml")), show(d));
}

{
  const repo = fresh("locked");
  mkdirSync(join(repo, ".pkl-gen"));
  writeFileSync(join(repo, ".pkl-gen/lock"), JSON.stringify({ pid: process.pid, started: new Date().toISOString() }));
  const r = await render("minimal", repo);
  ok("a render while the lock is held fails (exit 1) and writes nothing", r.code === 1 && r.err.includes("is held") && !existsSync(join(repo, "rust-toolchain.toml")), show(r));
}

{
  const r = await render("smoke-case-collision", fresh("collision"));
  ok("two paths that differ only by case are refused on any filesystem (exit 1)", r.code === 1 && r.err.includes("differ only by case"), show(r));
  const repo = fresh("rename");
  await render("smoke-case-a", repo);
  const rn = await render("smoke-case-b", repo);
  ok("a case-only rename is refused (exit 3) and the old file is kept", rn.code === 3 && rn.err.includes("differ only by case") && readFileSync(join(repo, "readme.md"), "utf8") === "one\n", show(rn));
}

{
  const repo = fresh("orphan");
  git(repo, "init", "-q", "-b", "main");
  await render("smoke-orphan-a", repo);
  commitAll(repo);
  const r = await render("smoke-orphan-b", repo);
  ok("an orphan is deleted in a clean git repository, so git works inside the binary", r.code === 0 && r.out.includes("deleted 1") && !existsSync(join(repo, "b.txt")) && existsSync(join(repo, "a.txt")), show(r));

  const dirty = fresh("orphan-dirty");
  git(dirty, "init", "-q", "-b", "main");
  await render("smoke-orphan-a", dirty);
  writeFileSync(join(dirty, "notes.md"), "n\n");
  commitAll(dirty);
  writeFileSync(join(dirty, "notes.md"), "changed\n");
  const d = await render("smoke-orphan-b", dirty);
  ok("a dirty tracked tree refuses the deletion (exit 3) and keeps the file", d.code === 3 && d.err.includes("not clean") && existsSync(join(dirty, "b.txt")), show(d));
}

if (base.startsWith(tmpdir()) && base.includes("scaffold-smoke-")) rmSync(base, { recursive: true, force: true });
console.log(failures ? `\n${failures} failing` : "\nall passed");
process.exit(failures ? 1 : 0);
