import { mkdtempSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type { Contract, ContractFile } from "../src/contract";

const made: string[] = [];

export function tempRepo(): string {
  const dir = mkdtempSync(join(tmpdir(), "scaffold-test-"));
  made.push(dir);
  return dir;
}

export function cleanupRepos(): void {
  for (const dir of made.splice(0)) if (dir.startsWith(tmpdir()) && dir.includes("scaffold-test-")) rmSync(dir, { recursive: true, force: true });
}

export const put = (root: string, rel: string, text: string): void => {
  mkdirSync(dirname(join(root, rel)), { recursive: true });
  writeFileSync(join(root, rel), text);
};
export const get = (root: string, rel: string): string => readFileSync(join(root, rel), "utf8");
export const has = (root: string, rel: string): boolean => existsSync(join(root, rel));
export const modeOf = (root: string, rel: string): string => (statSync(join(root, rel)).mode & 0o777).toString(8).padStart(4, "0");

export const owned = (path: string, content: string, mode: "0644" | "0755" = "0644"): ContractFile => ({ path, class: "owned", mode, content });
export const seed = (path: string, content: string): ContractFile => ({ path, class: "seed", mode: "0644", content });
export const external = (path: string): ContractFile => ({ path, class: "external", mode: "0644" });
export const contractOf = (...files: ContractFile[]): Contract => ({ schemaVersion: 1, files, pins: [], escapeHatch: { uses: 0, constructs: 0 } });

/** Runs git in a directory with the GIT_* variables removed, so a hook or worktree environment cannot redirect it. */
export function git(dir: string, ...args: string[]): string {
  const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith("GIT_")));
  const r = Bun.spawnSync(["git", "-C", dir, "-c", "user.name=test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false", ...args], { env, stdout: "pipe", stderr: "pipe" });
  if (r.exitCode !== 0) throw new Error(`git ${args.join(" ")} failed: ${r.stderr.toString()}`);
  return r.stdout.toString();
}

/** A temp directory that is a git repository with one commit of every file written so far. */
export function gitRepo(files: Record<string, string> = { "README.md": "x\n" }): string {
  const root = tempRepo();
  git(root, "init", "-q", "-b", "main");
  for (const [rel, text] of Object.entries(files)) put(root, rel, text);
  commitAll(root, "init");
  return root;
}

export const commitAll = (root: string, message: string): void => {
  git(root, "add", "-A");
  git(root, "commit", "-q", "-m", message);
};
