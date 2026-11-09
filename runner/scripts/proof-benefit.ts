// Phase 7 wave D (task 7.6): the benefit test. Two scratch clones of the proof repositories adopt the v<from> release, then one canonical change
// (the header line of plumber.yml, released as v<to>) reaches both through `upgrade`. Nothing outside a temporary folder is written.
// The consumer runs the published runner of the version it is pinned to (mise, checksums and provenance enforced, in an isolated data folder).
// `upgrade` runs from this checkout's source, because the published <from> runner cannot move the Builtins import (decision 89): the one
// hand edit of that hop is done by the script and reported. A direct render of v<to> into a fresh clone is the reference.
// usage: bun runner/scripts/proof-benefit.ts [from] [to]   (default 0.1.3 0.1.4)
import { lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const from = Bun.argv[2] ?? "0.1.3";
const to = Bun.argv[3] ?? "0.1.4";
const REPO = "pantheon-org/pantheon-scaffold";
const cli = join(root, "runner/src/cli.ts");
const SPECS = [
  { name: "pantheon-adr", options: `crateName = "adr"\n  skillName = "adr-creator"\n  gitDependencyIgnore { "pahdk-core"; "pahdk-install" }` },
  { name: "pantheon-skill-auditor", options: `crateName = "skill-auditor"\n  skillName = "skill-quality-auditor"\n  compositeAction = true\n  gitDependencyIgnore { "pahdk-core"; "pahdk-install" }` },
];

type Run = { out: string; err: string; code: number };
const work = mkdtempSync(join(tmpdir(), "proof-benefit-"));
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
const mx = (dir: string, ...args: string[]) => sh(["mise", "x", "--", "pantheon-scaffold", ...args], dir);
const listing = (dir: string, sub = ""): string[] =>
  readdirSync(join(dir, sub), { withFileTypes: true }).flatMap((e) => (e.name === ".git" || e.name === ".pkl-gen" ? [] : e.isDirectory() ? listing(dir, join(sub, e.name)) : [join(sub, e.name)]));
/** A file is read as text, a symlink as its target, so a link to a directory is compared by what it points at. */
const content = (dir: string, rel: string): string => (lstatSync(join(dir, rel)).isSymbolicLink() ? `-> ${readlinkSync(join(dir, rel))}` : readFileSync(join(dir, rel), "utf8"));
const uri = (v: string, sum: string, file: string) => `package://github.com/${REPO}/releases/download/v${v}/pantheon-scaffold@${v}::sha256:${sum}#/${file}`;
const releaseSum = async (v: string): Promise<string> => {
  const dir = join(work, `sums-${v}`);
  need(await sh(["gh", "release", "download", `v${v}`, "--repo", REPO, "-p", "checksums.txt", "-D", dir], work), `download checksums of v${v}`);
  const line = readFileSync(join(dir, "checksums.txt"), "utf8").split("\n").find((l) => l.trim().endsWith(`pantheon-scaffold@${v}`));
  if (!line) throw new Error(`no package checksum for ${v}`);
  return line.trim().split(/\s+/)[0];
};
const config = (v: string, sum: string, options: string): string =>
  `amends "${uri(v, sum, "Config.pkl")}"\nimport "${uri(v, sum, "Builtins.pkl")}"\n\noptions {\n  toolchain = "1.96.1"\n  ${options}\n}\n\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles()\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n`;
const miseToml = (v: string) =>
  `[settings]\nlockfile = true\nlocked_verify_provenance = true\nlockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]\n\n[tools]\npkl = "0.32.2"\n"github:${REPO}" = "${v}"\n`;

const fromSum = await releaseSum(from);
const toSum = await releaseSum(to);
const lines: string[] = [];
const say = (s = "") => lines.push(s);
let allPass = true;
const rows: string[] = [];
let failure = "";
try {
  for (const spec of SPECS) {
    const dir = join(work, spec.name);
    need(await sh(["git", "clone", "--quiet", "--depth", "1", `https://github.com/pantheon-org/${spec.name}.git`, dir], work), `clone of ${spec.name}`);
    const head = (await git(dir, "rev-parse", "--short", "HEAD")).out;
    writeFileSync(join(dir, "pantheon.pkl"), config(from, fromSum, spec.options));
    writeFileSync(join(dir, "mise.toml"), miseToml(from));
    need(await sh(["mise", "lock"], dir), `mise lock for ${spec.name}`);
    need(await sh(["mise", "install", "--locked"], dir), `mise install for ${spec.name}`);
    need(await mx(dir, "render", "pantheon.pkl", "--force"), `render with ${from} for ${spec.name}`);
    await git(dir, "add", "-A");
    need(await git(dir, "commit", "-q", "-m", `adopt ${from}`), "commit adoption");
    const before = new Map(listing(dir).map((f) => [f, content(dir, f)]));
    // The one hand edit of the 0.1.3 hop (decision 89): the Builtins import, which the 0.1.3 runner's upgrade does not move.
    const pkl = readFileSync(join(dir, "pantheon.pkl"), "utf8");
    writeFileSync(join(dir, "pantheon.pkl"), pkl.replace(uri(from, fromSum, "Builtins.pkl"), uri(to, toSum, "Builtins.pkl")));
    const up = await sh([process.execPath, cli, "upgrade", to, "--config", "pantheon.pkl", "--root", dir], dir);
    // Exit 3 is the render inside upgrade refusing mise.toml (an owned file whose runner pin upgrade just rewrote): recorded, then forced.
    const refusal = up.code === 3 ? up.err.split("\n").filter((l) => /^ {2}\S.*: /.test(l)).map((l) => l.trim()).join("; ") : "";
    if (up.code !== 0 && up.code !== 3) need(up, `upgrade ${spec.name}`);
    if (up.code === 3) need(await mx(dir, "render", "pantheon.pkl", "--force"), `render --force after upgrade ${spec.name}`);
    const version = (await mx(dir, "--version")).out;
    const check = need(await mx(dir, "check", "pantheon.pkl"), `check ${spec.name}`);
    const again = need(await mx(dir, "render", "pantheon.pkl"), `second render ${spec.name}`);
    const after = new Map(listing(dir).map((f) => [f, content(dir, f)]));
    const changed = [...after.keys()].filter((f) => before.get(f) !== after.get(f) && !["mise.lock"].includes(f)).sort();
    const added = [...after.keys()].filter((f) => !before.has(f));
    const removed = [...before.keys()].filter((f) => !after.has(f));
    // The reference: a fresh clone adopted straight on `to` with the published `to` runner. The upgraded tree must equal it.
    const ref = join(work, `${spec.name}-direct`);
    need(await sh(["git", "clone", "--quiet", "--depth", "1", `https://github.com/pantheon-org/${spec.name}.git`, ref], work), `clone of ${spec.name} (reference)`);
    writeFileSync(join(ref, "pantheon.pkl"), config(to, toSum, spec.options));
    writeFileSync(join(ref, "mise.toml"), miseToml(to));
    need(await sh(["mise", "lock"], ref), "mise lock (reference)");
    need(await sh(["mise", "install", "--locked"], ref), "mise install (reference)");
    need(await mx(ref, "render", "pantheon.pkl", "--force"), `direct render of ${to}`);
    const refFiles = new Map(listing(ref).map((f) => [f, content(ref, f)]));
    const differ = [...new Set([...after.keys(), ...refFiles.keys()])].filter((f) => after.get(f) !== refFiles.get(f) && f !== "mise.lock").sort();
    const diff = (await git(dir, "diff", "--numstat", "HEAD", "--", ".github/workflows/plumber.yml")).out;
    const onlyHeader = changed.filter((f) => !["pantheon.pkl", "mise.toml"].includes(f)).join(",") === ".github/workflows/plumber.yml" && /^1\s+0\s/.test(diff.replace(/^\s+/, "")) === false ? true : /^2\s+0\s/.test(diff);
    const pass = version === to && differ.length === 0 && added.length === 0 && removed.length === 0 && again.out.includes("written 0 file(s)") && onlyHeader;
    if (!pass) allPass = false;
    rows.push(`| ${spec.name} | ${head} | ${changed.join(", ")} | ${diff.replace(/\s+/g, " ")} | ${differ.length} | ${version} | ${check.out.split("\n").pop()} | ${again.out.split("\n").pop()} | ${pass ? "PASS" : "FAIL"} |`);
    say(`### ${spec.name}`);
    say();
    say(`- Exit of \`upgrade ${to}\`: ${up.code}${refusal ? `, its own render refused: ${refusal}, so a \`render --force\` followed` : ""}.`);
    say(`- \`upgrade ${to}\` output: ${up.out.split("\n").filter((l) => /^(changed|upgraded)/.test(l)).join("; ")}`);
    say(`- Files changed by the hop: ${changed.join(", ")}. Added: ${added.join(", ") || "none"}. Removed: ${removed.join(", ") || "none"}.`);
    say(`- \`plumber.yml\` numstat (added removed): ${diff.replace(/\s+/g, " ")}`);
    say(`- Files that differ from a direct render of v${to}: ${differ.join(", ") || "none"}.`);
    say();
  }
} catch (e) {
  allPass = false;
  failure = (e as Error).message;
}
console.log(`## Wave D: the benefit test (task 7.6)\n\n${lines.join("\n")}\n| Repository | Commit | Changed by the hop | plumber.yml | Differ from a direct ${to} render | Runner | check | Second render | Result |\n|---|---|---|---|---|---|---|---|---|\n${rows.join("\n")}\n\n${failure ? `ERROR: ${failure}\n\n` : ""}Verdict: ${allPass ? "PASS" : "FAIL"}`);
rmSync(work, { recursive: true, force: true });
process.exit(allPass ? 0 : 1);
