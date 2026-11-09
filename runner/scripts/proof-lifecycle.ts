// Phase 7 wave B (task 7.4): upgrade, downgrade and detach on a scratch consumer, and what happens when a package renames an owned file, drops
// a builtin or changes a toggle default. Nothing outside a temporary folder is written, and the four repositories are not touched.
//
// Part 1 uses the REAL releases v<from> and v<to>: mise installs their runners into an isolated data folder (MISE_DATA_DIR inside the scratch
// folder, checksums and provenance enforced), `upgrade` runs from this checkout's source (the published 0.1.3 runner moves only the amends
// line, decision 88), and every render and check is done by the published runner of the version the consumer is pinned to.
// Part 2 uses two local copies of the package and the published runner, because no released pair renames a file or drops a builtin.
// usage: bun runner/scripts/proof-lifecycle.ts [from] [to]   (default 0.1.1 0.1.3)
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const from = Bun.argv[2] ?? "0.1.1";
const to = Bun.argv[3] ?? "0.1.3";
const REPO = "pantheon-org/pantheon-scaffold";
const cli = join(root, "runner/src/cli.ts");

type Run = { out: string; err: string; code: number };
const work = mkdtempSync(join(tmpdir(), "proof-lifecycle-"));
const token = (await Bun.$`env -u GH_TOKEN gh auth token`.quiet().text()).trim();
const baseEnv = Object.fromEntries(Object.entries(process.env).filter(([k, v]) => k !== "GH_TOKEN" && v !== undefined)) as Record<string, string>;
const env: Record<string, string> = {
  ...baseEnv,
  GITHUB_TOKEN: token,
  MISE_DATA_DIR: join(work, "mise-data"),
  MISE_CACHE_DIR: join(work, "mise-cache"),
  MISE_STATE_DIR: join(work, "mise-state"),
  MISE_CONFIG_DIR: join(work, "mise-config"),
  MISE_TRUSTED_CONFIG_PATHS: work,
  MISE_YES: "1",
  NO_COLOR: "1",
};
for (const d of ["mise-data", "mise-cache", "mise-state", "mise-config"]) mkdirSync(join(work, d));
const sh = async (cmd: string[], cwd: string): Promise<Run> => {
  const p = Bun.spawn(cmd, { cwd, stdout: "pipe", stderr: "pipe", env });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};
const need = (r: Run, what: string): Run => {
  if (r.code !== 0) throw new Error(`${what} failed (exit ${r.code}): ${(r.err || r.out).slice(-700)}`);
  return r;
};
const git = (dir: string, ...args: string[]) => sh(["git", "-C", dir, "-c", "user.name=proof", "-c", "user.email=proof@example.invalid", "-c", "commit.gpgsign=false", ...args], work);
const text = (dir: string, rel: string): string => readFileSync(join(dir, rel), "utf8");
const listing = (dir: string, sub = ""): string[] =>
  readdirSync(join(dir, sub), { withFileTypes: true }).flatMap((e) => (e.name === ".git" || e.name === ".pkl-gen" ? [] : e.isDirectory() ? listing(dir, join(sub, e.name)) : [join(sub, e.name)]));

const results: { scenario: string; ok: boolean; detail: string }[] = [];
async function scenario(name: string, fn: () => Promise<string>): Promise<void> {
  try {
    results.push({ scenario: name, ok: true, detail: await fn() });
  } catch (e) {
    results.push({ scenario: name, ok: false, detail: (e as Error).message });
  }
}
const assert = (cond: boolean, what: string): void => {
  if (!cond) throw new Error(`expected: ${what}`);
};

const releaseSum = async (v: string): Promise<string> => {
  const dir = join(work, `sums-${v}`);
  need(await sh(["gh", "release", "download", `v${v}`, "--repo", REPO, "-p", "checksums.txt", "-D", dir], work), `download checksums of v${v}`);
  const line = readFileSync(join(dir, "checksums.txt"), "utf8").split("\n").find((l) => l.trim().endsWith(`pantheon-scaffold@${v}`));
  if (!line) throw new Error(`no package checksum for ${v}`);
  return line.trim().split(/\s+/)[0];
};
const uri = (v: string, sum: string, file: string) => `package://github.com/${REPO}/releases/download/v${v}/pantheon-scaffold@${v}::sha256:${sum}#/${file}`;
const slice = (v: string, sum: string) =>
  `amends "${uri(v, sum, "Config.pkl")}"\nimport "${uri(v, sum, "Builtins.pkl")}"\n\noptions { toolchain = "1.96.1" }\n\nworkflows {\n  ["cargo-deny"] = Builtins.cargoDeny(options)\n  ["rust-ci"] = Builtins.rustCi(options)\n  ["inline-scripts"] = Builtins.inlineScripts()\n}\n\nfiles {\n  ...Builtins.baseFiles(options)\n}\n`;
const miseToml = (v: string) =>
  `[settings]\nlockfile = true\nlocked_verify_provenance = true\nlockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]\n\n[tools]\npkl = "0.32.2"\n"github:${REPO}" = "${v}"\n`;
const mx = (consumer: string, ...args: string[]) => sh(["mise", "x", "--", "pantheon-scaffold", ...args], consumer);

// Part 1: the real releases.
const consumer = join(work, "consumer");
mkdirSync(consumer);
const fromSum = await releaseSum(from);
const toSum = await releaseSum(to);
await git(consumer, "init", "-q", "-b", "main");
writeFileSync(join(consumer, "pantheon.pkl"), slice(from, fromSum));
writeFileSync(join(consumer, "mise.toml"), miseToml(from));
writeFileSync(join(consumer, ".gitignore"), ".pkl-gen/cache/\n");
need(await sh(["mise", "lock"], consumer), "mise lock");
need(await sh(["mise", "install", "--locked"], consumer), "mise install --locked");
need(await mx(consumer, "render", "pantheon.pkl"), `render with ${from}`);
await git(consumer, "add", "-A");
await git(consumer, "commit", "-q", "-m", `render with ${from}`);
const initial = { pkl: text(consumer, "pantheon.pkl"), mise: text(consumer, "mise.toml"), files: listing(consumer).filter((f) => !["mise.lock", ".gitignore"].includes(f)).sort() };
const initialBytes = new Map(initial.files.map((f) => [f, text(consumer, f)]));
const runnerPath = (await sh(["mise", "which", "pantheon-scaffold"], consumer)).out;

await scenario(`upgrade ${from} to ${to}, the real releases`, async () => {
  const r = await sh([process.execPath, cli, "upgrade", to, "--config", "pantheon.pkl", "--root", consumer], consumer);
  need(r, "upgrade");
  assert(!text(consumer, "pantheon.pkl").includes(`v${from}/`), `no reference to ${from} is left in pantheon.pkl`);
  assert(text(consumer, "pantheon.pkl").includes(`v${to}/pantheon-scaffold@${to}::sha256:${toSum}#/Builtins.pkl`), "the Builtins import is on the new version with its checksum");
  assert(text(consumer, "mise.toml").includes(`"github:${REPO}" = "${to}"`), "the runner pin moved");
  assert((await mx(consumer, "--version")).out === to, `mise now runs ${to}`);
  const check = need(await mx(consumer, "check", "pantheon.pkl"), "check after upgrade");
  assert(readFileSync(join(consumer, "mise.lock"), "utf8").includes(`version = "${to}"`), "mise.lock names the new runner");
  return `${r.out.split("\n").filter((l) => /^(changed|upgraded)/.test(l)).join("; ")}; ${check.out.split("\n").pop()}`;
});
await git(consumer, "add", "-A");
await git(consumer, "commit", "-q", "-m", `upgrade to ${to}`);

await scenario(`pin-back to ${from}: upgrade --force, then render --force`, async () => {
  // Pinning back is two steps by design: upgrade --force moves the pins, and the render that follows refuses an older package than the one that
  // last rendered (exit 3, the version rule of runner-lifecycle.md), so the consumer renders once more with --force.
  const r = await sh([process.execPath, cli, "upgrade", from, "--force", "--config", "pantheon.pkl", "--root", consumer], consumer);
  assert(r.code === 3, `upgrade --force moves the pins and its render is refused with exit 3 (got ${r.code})`);
  assert(r.err.includes(`package ${from} is older than ${to}`), "the refusal says the package is older than the one that rendered last");
  const forced = need(await mx(consumer, "render", "pantheon.pkl", "--force"), "render --force with the older runner");
  assert(text(consumer, "pantheon.pkl") === initial.pkl, `pantheon.pkl is byte for byte the ${from} one again`);
  assert(text(consumer, "mise.toml") === initial.mise, `mise.toml is byte for byte the ${from} one again`);
  assert((await mx(consumer, "--version")).out === from, `mise runs ${from} again`);
  const files = listing(consumer).filter((f) => !["mise.lock", ".gitignore"].includes(f)).sort();
  assert(JSON.stringify(files) === JSON.stringify(initial.files), "the same files exist");
  for (const f of files) assert(text(consumer, f) === initialBytes.get(f), `${f} is back to the ${from} render`);
  const check = need(await mx(consumer, "check", "pantheon.pkl"), "check after pin-back");
  return `upgrade --force: pins moved, render refused (exit 3, older package); render --force: ${forced.out.split("\n").pop()}; ${files.length} files identical to the ${from} render; ${check.out.split("\n").pop()}`;
});
await git(consumer, "add", "-A");
await git(consumer, "commit", "-q", "-m", `pin back to ${from}`);
need(await sh([process.execPath, cli, "upgrade", to, "--config", "pantheon.pkl", "--root", consumer], consumer), `second upgrade to ${to}`);
await git(consumer, "add", "-A");
await git(consumer, "commit", "-q", "-m", `upgrade to ${to} again`);

await scenario("detach, then a git revert of the detach", async () => {
  const before = listing(consumer).sort();
  const d = need(await mx(consumer, "detach"), "detach");
  assert(!existsSync(join(consumer, ".pkl-gen/manifest.json")), "the manifest is gone");
  const afterDetach = listing(consumer).sort();
  const gone = before.filter((f) => !afterDetach.includes(f));
  assert(gone.length === 0, `detach keeps every rendered file as a plain file (deleted: ${gone.join(", ") || "none"})`);
  need(await git(consumer, "add", "-A"), "git add");
  need(await git(consumer, "commit", "-q", "-m", "detach"), "git commit detach");
  need(await git(consumer, "revert", "--no-edit", "HEAD"), "git revert");
  assert(existsSync(join(consumer, ".pkl-gen/manifest.json")), "the revert brought the manifest back");
  const check = need(await mx(consumer, "check", "pantheon.pkl"), "check after the revert");
  const again = need(await mx(consumer, "render", "pantheon.pkl"), "render after the revert");
  assert(again.out.includes("written 0 file(s)"), "a render after the revert writes nothing");
  return `${d.out.split("\n").pop()}; after the revert: ${check.out.split("\n").pop()}; render: ${again.out.split("\n").pop()}`;
});

// Part 2: two local copies of the package, the published runner.
const pkgA = join(work, "pkgA");
const pkgB = join(work, "pkgB");
cpSync(join(root, "pkl"), pkgA, { recursive: true });
cpSync(join(root, "pkl"), pkgB, { recursive: true });
const edit = (dir: string, file: string, a: string, b: string) => {
  const t = readFileSync(join(dir, file), "utf8");
  if (!t.includes(a)) throw new Error(`${file} has no ${JSON.stringify(a)}`);
  writeFileSync(join(dir, file), t.replace(a, () => b));
};
edit(pkgB, "Builtins.pkl", `  ["deny.toml"] = new Config.GenFile { content = read("assets/deny.toml").text }`, `  ["config/deny.toml"] = new Config.GenFile { content = read("assets/deny.toml").text }`);
edit(pkgB, "Config.pkl", "  skills: Boolean = true", "  skills: Boolean = false");
const local = (pkg: string, withInline: boolean) =>
  `amends "file://${pkg}/Config.pkl"\nimport "file://${pkg}/Builtins.pkl"\n\noptions {\n  toolchain = "1.96.1"\n  skillName = "demo-skill"\n  crateName = "demo"\n}\n\nworkflows {\n  ["plumber"] = Builtins.plumber()\n${withInline ? '  ["inline-scripts"] = Builtins.inlineScripts()\n' : ""}  when (options.skills) { ["audit-skill"] = Builtins.auditSkill(options) }\n}\n\nfiles {\n  ...Builtins.baseFiles(options)\n  ...Builtins.workflowFiles()\n}\n`;
const lib = join(work, "library");
mkdirSync(lib);
await git(lib, "init", "-q", "-b", "main");
const rr = (pkg: string, ...extra: string[]) => sh([runnerPath, ...extra, "--allow-local-package", pkg], lib);
writeFileSync(join(lib, "pantheon.pkl"), local(pkgA, true));
writeFileSync(join(lib, ".gitignore"), ".pkl-gen/cache/\n");
need(await rr(pkgA, "render", "pantheon.pkl", "--root", lib), "render with package A");
await git(lib, "add", "-A");
await git(lib, "commit", "-q", "-m", "package A");
const filesA = listing(lib).sort();
const plumberA = text(lib, ".github/workflows/plumber.yml");
const denyA = text(lib, "deny.toml");

await scenario("a package that renames an owned file, a consumer that drops a builtin, a toggle default that changes", async () => {
  assert(filesA.includes("deny.toml") && filesA.includes(".github/workflows/inline-scripts.yml") && filesA.includes(".github/workflows/audit-skill.yml"), "package A renders deny.toml, inline-scripts.yml and audit-skill.yml");
  // The runner deletes an orphan only from a clean tracked tree, so the consumer commits its config change first.
  writeFileSync(join(lib, "pantheon.pkl"), local(pkgB, false));
  await git(lib, "add", "-A");
  await git(lib, "commit", "-q", "-m", "use package B");
  need(await rr(pkgB, "render", "pantheon.pkl", "--root", lib), "render with package B");
  const filesB = listing(lib).sort();
  const removed = filesA.filter((f) => !filesB.includes(f)).sort();
  const added = filesB.filter((f) => !filesA.includes(f)).sort();
  assert(JSON.stringify(removed) === JSON.stringify([".github/workflows/audit-skill.yml", ".github/workflows/inline-scripts.yml", "deny.toml"]), `exactly the renamed file, the dropped builtin and the file of the changed toggle default were deleted (deleted: ${removed.join(", ")})`);
  assert(JSON.stringify(added) === JSON.stringify(["config/deny.toml"]), `the renamed file was written under its new name (added: ${added.join(", ")})`);
  assert(text(lib, "config/deny.toml") === denyA, "the renamed file has the same content");
  assert(text(lib, ".github/workflows/plumber.yml") === plumberA, "an unrelated file is untouched");
  const check = need(await rr(pkgB, "check", "pantheon.pkl", "--root", lib), "check with package B");
  const again = need(await rr(pkgB, "render", "pantheon.pkl", "--root", lib), "second render");
  assert(again.out.includes("written 0 file(s)"), "a second render writes nothing");
  return `deleted ${removed.join(", ")}; wrote ${added.join(", ")}; ${check.out.split("\n").pop()}`;
});

const lines: string[] = [];
lines.push(`| Scenario | Result | What was checked |`);
lines.push(`|---|---|---|`);
for (const r of results) lines.push(`| ${r.scenario} | ${r.ok ? "PASS" : "FAIL"} | ${r.detail.replace(/\|/g, "\\|").replace(/\n/g, " ")} |`);
lines.push("");
lines.push(`Verdict: ${results.every((r) => r.ok) ? "PASS" : "FAIL"}`);
console.log(lines.join("\n"));
// The isolated mise install and the scratch repositories are deleted, as promised when the install was allowed.
rmSync(work, { recursive: true, force: true });
process.exit(results.every((r) => r.ok) ? 0 : 1);
