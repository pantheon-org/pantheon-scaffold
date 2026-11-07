// Task 4.6: render the slice into a scratch clone of a real repository and judge the result by the P.1 rule.
// Needs network (git clone over https) and actionlint. Usage: bun runner/scripts/real-repo-render.ts [owner/repo] [commit]
// It writes only inside a temporary folder. Defaults: pantheon-org/pantheon-adr at the commit the tests pin.
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { compareWorkflows } from "../src/equality";

const root = resolve(import.meta.dir, "../..");
const repoName = Bun.argv[2] ?? "pantheon-org/pantheon-adr";
const commit = Bun.argv[3] ?? (await Bun.file(join(root, "runner/test/upstream/COMMIT")).text()).trim();
const cli = join(root, "runner/src/cli.ts");
const slice = join(root, "runner/test/fixtures/slice/pantheon.pkl");

const sh = async (cmd: string[], cwd: string) => {
  const p = Bun.spawn(cmd, { cwd, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};
const render = (repo: string, ...extra: string[]) => sh(["bun", cli, "render", slice, "--root", repo, "--allow-local-package", join(root, "pkl"), ...extra], root);

const work = mkdtempSync(join(tmpdir(), "real-repo-render-"));
const repo = join(work, "repo");
const clone = await sh(["git", "clone", "--quiet", `https://github.com/${repoName}.git`, repo], work);
if (clone.code !== 0) throw new Error(`clone failed: ${clone.err}`);
await sh(["git", "checkout", "--quiet", commit], repo);
const head = (await sh(["git", "rev-parse", "--short", "HEAD"], repo)).out;

const files = ["deny.toml", "rust-toolchain.toml", ".github/scripts/cargo-deny.sh", ".github/scripts/check-inline-scripts.sh", ".github/scripts/check-toolchain-version.sh", ".github/workflows/cargo-deny.yml", ".github/workflows/inline-scripts.yml", ".github/workflows/rust-ci.yml"];
const workflows = files.filter((f) => f.startsWith(".github/workflows/"));

const lines: string[] = [];
const say = (s: string) => lines.push(s);
say(`# Real-repo render of the slice (task 4.6)`);
say(`Repository ${repoName} at ${head}. Scratch clone: ${repo}`);

const first = await render(repo);
say(`\n## First render without --force (adoption rules)`);
say(`exit ${first.code}; ${first.err.split("\n").filter((l) => /^  \S.*: /.test(l)).length} file(s) refused as existing and different`);
for (const l of first.err.split("\n").filter((l) => /^  \S.*: /.test(l))) say(`- ${l.trim().split(":")[0]}`);
say(`unchanged lines in the output that adopted identical files: ${first.out.split("\n").filter((l) => l.startsWith("adopt")).map((l) => l.split(/\s+/)[1]).join(", ") || "none"}`);

const forced = await render(repo, "--force");
say(`\n## Render with --force`);
say(`exit ${forced.code}; ${forced.out.split("\n").pop()}`);

say(`\n## Per file result`);
say(`| File | Changed on disk | Lines differing (git diff) | P.1 equal |`);
say(`|---|---|---|---|`);
let allEqual = true;
for (const f of files) {
  const changed = (await sh(["git", "status", "--porcelain", "--", f], repo)).out !== "";
  const stat = (await sh(["git", "diff", "--numstat", "--", f], repo)).out.split(/\s+/);
  const diffLines = changed ? Number(stat[0]) + Number(stat[1]) : 0;
  let p1 = "n/a (byte comparison)";
  if (workflows.includes(f)) {
    const original = (await sh(["git", "show", `HEAD:${f}`], repo)).out + "\n";
    const rendered = await Bun.file(join(repo, f)).text();
    const r = compareWorkflows(original, rendered);
    p1 = r.equal ? "yes" : `NO: ${[...r.differences, ...r.duplicateKeys].join("; ")}`;
    if (!r.equal) allEqual = false;
  } else if (changed) allEqual = false;
  say(`| ${f} | ${changed ? "yes" : "no"} | ${diffLines} | ${p1} |`);
}

const lint = await sh(["actionlint"], repo);
say(`\n## Checks on the result`);
say(`- actionlint on the whole repository: exit ${lint.code}${lint.code ? `\n${lint.out}` : ""}`);
const check = await sh(["bun", cli, "check", slice, "--root", repo, "--allow-local-package", join(root, "pkl")], root);
say(`- check after the render: exit ${check.code} (${check.out || check.err.split("\n")[0]})`);
const again = await render(repo);
say(`- second render: ${again.out.split("\n").pop()}`);
const contract = await sh(["bun", cli, "contract", slice, "--root", repo, "--allow-local-package", join(root, "pkl")], root);
say(`- ${contract.out}`);
say(`\nVerdict: ${allEqual && lint.code === 0 && check.code === 0 && forced.code === 0 ? "PASS" : "FAIL"}`);

console.log(lines.join("\n"));
process.exit(allEqual && lint.code === 0 && check.code === 0 && forced.code === 0 ? 0 : 1);
