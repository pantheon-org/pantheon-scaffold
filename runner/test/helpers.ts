import { expect } from "bun:test";
import { mkdtempSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync, existsSync } from "node:fs";
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
export const contractOf = (...files: ContractFile[]): Contract => ({ schemaVersion: 2, files, pins: [], escapeHatch: { uses: 0, constructs: 0 } });

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

/** Replaces the commit of every pinned action with a marker, so a pin moving to a newer release does not break a comparison with a pinned upstream copy. The pins themselves are checked by shape (`expectPins`) and by the pin update script. */
/**
 * The repository-relative path of an asset the package ships, found by name under `pkl/Builtins/<builtin>/assets/`. Fails when there is no copy,
 * or more than one, and names what it found.
 */
export function assetPath(name: string): string {
  const builtins = join(import.meta.dir, "../../pkl/Builtins");
  const found: string[] = [];
  for (const e of readdirSync(builtins, { withFileTypes: true })) {
    if (e.isDirectory() && existsSync(join(builtins, e.name, "assets", name))) found.push(`pkl/Builtins/${e.name}/assets/${name}`);
  }
  if (found.length !== 1) throw new Error(`asset ${name}: expected one copy under pkl/Builtins/*/assets, found ${found.length}${found.length ? `: ${found.join(", ")}` : ""}`);
  return found[0]!;
}

export const unpin =(text: string): string => text.replace(/@[0-9a-f]{40}\b/g, "@<pin>");

/** The pins are exactly these actions, each a 40 character commit with a version tag. The values move with the pin update workflow, so they are not asserted. */
export function expectPins(pins: { action: string; sha: string; version: string }[], actions: string[]): void {
  expect(pins.map((p) => p.action).sort()).toEqual([...actions].sort());
  for (const p of pins) {
    expect(p.sha, p.action).toMatch(/^[0-9a-f]{40}$/);
    expect(p.version, p.action).toMatch(/^v\d+(\.\d+){0,2}$/);
  }
}
