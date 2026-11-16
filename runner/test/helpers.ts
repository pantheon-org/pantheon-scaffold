import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

// What the update-pins script tests need until Phase 6b deletes them: a temporary folder, and writing and checking files in it.
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
export const has = (root: string, rel: string): boolean => existsSync(join(root, rel));
