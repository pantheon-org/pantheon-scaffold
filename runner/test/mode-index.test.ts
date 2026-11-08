import { afterEach, expect, test } from "bun:test";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { cleanupRepos, commitAll, git, tempRepo } from "./helpers";

afterEach(cleanupRepos);

// Task 3.3: the executable bit as git records it. The runner never reads the index. In CI `check` runs on a checkout, where the mode on
// disk is the mode in the index, so a committed mode that is wrong shows up as mode drift. These tests prove that with real git.
const root = join(import.meta.dir, "../..");
const cli = join(import.meta.dir, "../src/cli.ts");
const config = join(import.meta.dir, "fixtures/tooling/pantheon.pkl");
const NINE = ["agnix-assemble-comment", "agnix-file-issues", "agnix-introduced-errors", "auto-rebase", "cargo-deny", "check-commit-types", "check-inline-scripts", "release-automerge", "sync-labels"].map((n) => `.github/scripts/${n}.sh`);

const run = async (cwd: string, ...args: string[]) => {
  const p = Bun.spawn(["bun", cli, ...args, "--allow-local-package", join(root, "pkl")], { cwd, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};
const modes = (dir: string): Record<string, string> =>
  Object.fromEntries(git(dir, "ls-files", "-s").trim().split("\n").map((l) => [l.split("\t")[1], l.split(" ")[0]]));
const clone = (source: string): string => {
  const dir = join(mkdtempSync(join(tmpdir(), "scaffold-test-")), "clone");
  git(source, "clone", "-q", source, dir);
  return dir;
};

async function committedRepo(): Promise<string> {
  const repo = tempRepo();
  git(repo, "init", "-q", "-b", "main");
  expect((await run(repo, "render", config, "--root", repo)).code).toBe(0);
  commitAll(repo, "render");
  return repo;
}

test("git records the nine shared scripts as 100755 and the configs as 100644 after a render and a commit", async () => {
  const m = modes(await committedRepo());
  for (const p of NINE) expect(m[p], p).toBe("100755");
  for (const p of [".rumdl.toml", ".claude/settings.json", "deny.toml", "rust-toolchain.toml"]) expect(m[p], p).toBe("100644");
});

test("check on a fresh clone of the committed render finds no drift", async () => {
  const dir = clone(await committedRepo());
  const r = await run(dir, "check", config, "--root", dir);
  expect(r.code).toBe(0);
});

test("a script committed without the executable bit is mode drift on a fresh clone (exit 4)", async () => {
  const repo = await committedRepo();
  git(repo, "update-index", "--chmod=-x", ".github/scripts/auto-rebase.sh");
  git(repo, "commit", "-q", "-m", "lose the bit");
  expect(modes(repo)[".github/scripts/auto-rebase.sh"]).toBe("100644");
  const dir = clone(repo);
  const r = await run(dir, "check", config, "--root", dir);
  expect(r.code).toBe(4);
  expect(r.err).toContain(".github/scripts/auto-rebase.sh: mode");
});
