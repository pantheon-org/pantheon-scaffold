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
