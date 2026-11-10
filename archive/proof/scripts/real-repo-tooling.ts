// Phase 3 wave D: render everything Phase 3 owns into scratch clones of the four real repositories and judge the result.
// Needs network (https clone), git, and installed copies of hk 2.10.0, pkl 0.32.2, rumdl 0.2.77, shellcheck, yq, actionlint and the Rust
// toolchain the repositories pin, found through mise and rustup. It writes only inside a temporary folder; nothing is pushed.
// usage: bun runner/scripts/real-repo-tooling.ts
import { existsSync, mkdirSync, mkdtempSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const cli = join(root, "runner/src/cli.ts");
const pkg = join(root, "pkl");
const fixture = (name: string) => join(root, "runner/test/fixtures", name, "pantheon.pkl");

// The library repository is the one that builds with --all-features (decision 74).
const REPOS = [
  { name: "pantheon-adr", fixture: "phase3" },
  { name: "pantheon-journal", fixture: "phase3" },
  { name: "pantheon-skill-auditor", fixture: "phase3" },
  { name: "pantheon-agentic-harness-dev-kit", fixture: "phase3-all" },
];
// Git blobs pinned by the tests: pantheon-adr's mise.toml and hk.pkl at 1e2fd46, and the library repository's hk.pkl at 42f48b4.
const MISE_BLOB = "16c264c54f1408330819ecd81131de90eb9b49e4";
const HK_BLOBS: Record<string, string> = { e4bbde76e0d09f3bbc329a01e38a62deb7473513: "the pantheon-adr copy", "37ed8613e5f7f095333fc558bbddecf4d5748f2d": "the library repository copy" };
// What a render may change in a repository that already uses the shared files, once its hk.pkl is replaced by the seeded one (the migration).
// The mise.toml change in lines, as mise.test.ts asserts it line by line: 10 added (two reworded comments, the runner pin and its comment, the
// settings comment and three settings) and 4 removed (the two old comments of two lines each).
const MISE_ADDED = 10;
const MISE_REMOVED = 4;
const EXPECTED_CHANGES = [".github/scripts/sync-labels.sh", ".mise/conf.d/repo.toml", ".pkl-gen/manifest.json", "hk.pkl", "hk.shared.pkl", "mise.toml"];

type Run = { out: string; err: string; code: number };
const sh = async (cmd: string[], cwd: string, env?: Record<string, string>): Promise<Run> => {
  const p = Bun.spawn(cmd, { cwd, stdout: "pipe", stderr: "pipe", ...(env ? { env } : {}) });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};
const strip = (s: string): string => s.replace(/\x1b\[[0-9;]*m/g, "");

/** The real executable of an installed tool, found without a mise shim, so a repository's own mise.toml cannot change which one runs. */
async function installed(tool: string, name = tool.split("@")[0]): Promise<string> {
  const where = (await sh(["mise", "where", tool], root)).out;
  const found = where ? (await sh(["find", where, "-maxdepth", "3", "-name", name, "-type", "f", "-perm", "-u+x"], root)).out.split("\n")[0] : "";
  if (!found) throw new Error(`${tool} is not installed (mise where ${tool} found nothing)`);
  return found;
}

const work = mkdtempSync(join(tmpdir(), "real-repo-tooling-"));
const bin = join(work, "bin");
mkdirSync(bin);
const cargo = (await sh(["rustup", "which", "cargo", "--toolchain", "1.96.1"], root)).out;
if (!cargo) throw new Error("the Rust toolchain 1.96.1 the repositories pin is not installed (rustup which cargo --toolchain 1.96.1)");
for (const [name, path] of Object.entries({
  rumdl: await installed("rumdl@0.2.77"),
  shellcheck: await installed("shellcheck"),
  yq: await installed("yq"),
  actionlint: await installed("actionlint"),
  cargo,
  "cargo-fmt": join(dirname(cargo), "cargo-fmt"),
  rustfmt: join(dirname(cargo), "rustfmt"),
})) {
  if (!existsSync(path)) throw new Error(`${name} not found at ${path}`);
  symlinkSync(path, join(bin, name));
}
const hkBin = dirname(await installed("hk@2.10.0"));
const pklBin = dirname(await installed("pkl@0.32.2"));
const hkEnv: Record<string, string> = { HOME: process.env.HOME ?? "", PATH: `${bin}:${hkBin}:${pklBin}:/usr/bin:/bin`, HK_MISE: "0", NO_COLOR: "1" };
const hkVersion = (await sh(["hk", "--version"], work, hkEnv)).out;

/** The outcome of `hk check --all`: its exit code and the result mark of each step. */
async function hkCheck(repo: string): Promise<{ code: number; steps: string[]; failures: string[] }> {
  const r = await sh(["hk", "check", "--all", "--no-fail-fast"], repo, hkEnv);
  const steps = new Set<string>();
  for (const line of strip(r.out + "\n" + r.err).split("\n")) {
    const m = /^(✔|✗|⚠) ([a-z][a-z-]*)\b/.exec(line.trim());
    if (m && m[2] !== "files") steps.add(`${m[1]} ${m[2]}`);
  }
  // The lines of a failing step's output, so a failure that exists before the render can be read in the report.
  const failures = strip(r.out + "\n" + r.err).split("\n").filter((l) => /^\s+(rumdl|rustfmt|shellcheck|actionlint|yamllint) – /.test(l) && !/ – \d+ files? – /.test(l)).map((l) => l.trim().slice(0, 160));
  return { code: r.code, steps: [...steps].sort(), failures };
}

const render = (config: string, repo: string, ...extra: string[]) => sh(["bun", cli, "render", config, "--root", repo, "--allow-local-package", pkg, ...extra], root);
const check = (config: string, repo: string) => sh(["bun", cli, "check", config, "--root", repo, "--allow-local-package", pkg], root);
const refused = (r: Run): string[] => r.err.split("\n").filter((l) => /^  \S.*?: /.test(l)).map((l) => l.trim().split(":")[0]);

const lines: string[] = [];
const say = (s = "") => lines.push(s);
say("# Real-repository render of the Phase 3 files");
say();
say(`hk ${hkVersion}. Each repository is cloned (https, depth 1) into a temporary folder, rendered into, and discarded. Nothing is written to the repositories.`);
say();

let allPass = true;
const summary: string[] = [];

for (const repoSpec of REPOS) {
  const repo = join(work, repoSpec.name);
  const clone = await sh(["git", "clone", "--quiet", "--depth", "1", `https://github.com/pantheon-org/${repoSpec.name}.git`, repo], work);
  if (clone.code !== 0) throw new Error(`clone of ${repoSpec.name} failed: ${clone.err}`);
  const head = (await sh(["git", "rev-parse", "--short", "HEAD"], repo)).out;
  const config = fixture(repoSpec.fixture);
  const miseBlob = (await sh(["git", "rev-parse", "HEAD:mise.toml"], repo)).out;
  const hkBlob = (await sh(["git", "rev-parse", "HEAD:hk.pkl"], repo)).out;
  const hkKind = HK_BLOBS[hkBlob];

  const before = await hkCheck(repo);
  const adoption = await render(config, repo);
  const adoptionRefused = refused(adoption);
  const driftBefore = await check(config, repo);
  const drift = driftBefore.err.split("\n").filter((l) => /^  \S.*?: /.test(l)).map((l) => l.trim());

  // The seeded hk.pkl is what the migration replaces the repository's own hk.pkl with, so read it from a render into an empty folder.
  const empty = join(work, `${repoSpec.name}-empty`);
  mkdirSync(empty);
  await render(config, empty);
  const seeded = await Bun.file(join(empty, "hk.pkl")).text();

  const forced = await render(config, repo, "--force");
  await Bun.write(join(repo, "hk.pkl"), seeded);
  const changed = (await sh(["git", "status", "--porcelain", "-uall"], repo)).out.split("\n").filter(Boolean).map((l) => l.replace(/^\s*\S+\s+/, "")).sort();
  const sync = (await sh(["git", "diff", "--numstat", "--", ".github/scripts/sync-labels.sh"], repo)).out.split(/\s+/);
  const mise = (await sh(["git", "diff", "--numstat", "--", "mise.toml"], repo)).out.split(/\s+/);
  const miseOk = Number(mise[0]) === MISE_ADDED && Number(mise[1]) === MISE_REMOVED;
  const after = await hkCheck(repo);
  const checkAfter = await check(config, repo);
  const again = await render(config, repo);

  const changesOk = JSON.stringify(changed) === JSON.stringify(EXPECTED_CHANGES);
  const hkSame = before.code === after.code && JSON.stringify(before.steps) === JSON.stringify(after.steps);
  const pass = miseBlob === MISE_BLOB && miseOk && hkKind !== undefined && changesOk && hkSame && forced.code === 0 && checkAfter.code === 0 && again.out.includes("written 0 file(s)");
  if (!pass) allPass = false;
  summary.push(`| ${repoSpec.name} | ${head} | ${repoSpec.fixture === "phase3-all" ? "all" : "default"} | ${pass ? "PASS" : "FAIL"} |`);

  say(`## ${repoSpec.name} at ${head}`);
  say();
  say(`- Renders with options.features = ${repoSpec.fixture === "phase3-all" ? "all" : "default"}.`);
  say(`- mise.toml before the render is the pinned pantheon-adr copy: ${miseBlob === MISE_BLOB ? "yes" : `NO (${miseBlob})`}.`);
  say(`- hk.pkl before the render is ${hkKind ? `byte for byte ${hkKind}` : `UNCATEGORISED (${hkBlob})`}.`);
  say(`- First render without --force: exit ${adoption.code}. ${adoptionRefused.length} file(s) refused as existing and different: ${adoptionRefused.join(", ") || "none"}. Adopted unchanged: ${adoption.out.split("\n").filter((l) => l.startsWith("adopt")).length}.`);
  say(`- \`check\` before any render reports ${drift.length} difference(s): ${drift.join("; ") || "none"}.`);
  say(`- After \`render --force\` and replacing hk.pkl with the seeded one, \`git status\` lists ${changed.length} path(s): ${changed.join(", ")}. Expected exactly: ${EXPECTED_CHANGES.join(", ")}: ${changesOk ? "yes" : "NO"}.`);
  say(`- mise.toml changed by ${mise[0]} lines added and ${mise[1]} removed. Expected ${MISE_ADDED} and ${MISE_REMOVED}: ${miseOk ? "yes" : "NO"}.`);
  say(`- sync-labels.sh: ${sync[0]} lines added, ${sync[1]} removed (a mode change only: it is 100644 in the repository and rendered 755).`);
  say(`- \`hk check --all --no-fail-fast\` before: exit ${before.code}, ${before.steps.join(", ")}.`);
  for (const l of before.failures.slice(0, 6)) say(`  - ${l}`);
  say(`- \`hk check --all --no-fail-fast\` after: exit ${after.code}, ${after.steps.join(", ")}. Same outcome: ${hkSame ? "yes" : "NO"}.`);
  say(`- \`check\` after the render: exit ${checkAfter.code}. Second render: ${again.out.split("\n").pop()}.`);
  say();
}

say("## Summary");
say();
say("| Repository | Commit | Features | Result |");
say("|---|---|---|---|");
for (const s of summary) say(s);
say();
say(`Verdict: ${allPass ? "PASS" : "FAIL"}`);

console.log(lines.join("\n"));
process.exit(allPass ? 0 : 1);
