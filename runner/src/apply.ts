import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { ContractError, type Contract, type ContractFile, type Pin } from "./contract";
import { MANIFEST_PATH, TOOL_DIR, emptyManifest, readManifest, serialiseManifest, sha256, type Manifest } from "./manifest";
import { deleteGuard } from "./guard";
import { acquireLock, type LockOptions } from "./lock";
import { PathError, findCaseCollisions, resolveInside } from "./paths";
import { appendVersions } from "./versions";

export type Options = { force?: boolean; dryRun?: boolean; repair?: boolean; lock?: LockOptions; packageVersion?: string };

export type Action =
  | { kind: "create"; path: string }
  | { kind: "update"; path: string }
  | { kind: "unchanged"; path: string }
  | { kind: "adopt"; path: string }
  | { kind: "seed"; path: string }
  | { kind: "seed-kept"; path: string }
  | { kind: "external"; path: string }
  | { kind: "chmod"; path: string }
  | { kind: "delete"; path: string }
  | { kind: "forget"; path: string };

export type Report = {
  actions: Action[];
  refusals: string[];
  written: number;
  deleted: number;
  dryRun: boolean;
};

const wantsExec = (f: ContractFile): boolean => f.mode === "0755";
const hasExec = (abs: string): boolean => (statSync(abs).mode & 0o111) !== 0;

const WORKFLOW = /^\.github\/(workflows\/[^/]+\.ya?ml|actions\/[^/]+\/action\.ya?ml)$/;

/** The text that lands on disk and is hashed: version comments are added to pinned uses lines first (decision 17). */
export const finalContent = (f: ContractFile, pins: Pin[]): string => {
  if (!WORKFLOW.test(f.path)) return f.content!;
  try {
    return appendVersions(f.content!, pins);
  } catch (e) {
    throw new ContractError(`${f.path}: ${(e as Error).message}`);
  }
};

const read = (abs: string): string | null => (existsSync(abs) ? readFileSync(abs, "utf8") : null);

/** The first differing lines, enough to see what a refusal is about. */
export function shortDiff(onDisk: string, rendered: string, max = 6): string {
  const a = onDisk.split("\n");
  const b = rendered.split("\n");
  const out: string[] = [];
  for (let i = 0; i < Math.max(a.length, b.length) && out.length < max * 2; i++) {
    if (a[i] === b[i]) continue;
    if (a[i] !== undefined) out.push(`    line ${i + 1} on disk:  ${a[i]}`);
    if (b[i] !== undefined) out.push(`    line ${i + 1} rendered: ${b[i]}`);
  }
  return out.join("\n");
}

/** Plans every action without touching the disk. Refusals are collected so the run can stop before writing anything. */
export function plan(contract: Contract, root: string, manifest: Manifest, opts: Options): { actions: Action[]; refusals: string[]; next: Manifest; writes: ContractFile[]; chmods: ContractFile[]; deletes: string[] } {
  const refusals: string[] = [];
  const actions: Action[] = [];
  const writes: ContractFile[] = [];
  const chmods: ContractFile[] = [];
  const next: Manifest = emptyManifest();
  const packageVersion = opts.packageVersion ?? manifest.packageVersion;
  if (packageVersion !== undefined) next.packageVersion = packageVersion;

  /** The content is already right: only the executable bit may still differ (set and cleared as the contract says). */
  const settled = (f: ContractFile, abs: string, kind: "unchanged" | "adopt"): void => {
    const drift = hasExec(abs) !== wantsExec(f);
    if (drift) chmods.push(f);
    if (kind === "adopt") actions.push({ kind: "adopt", path: f.path });
    if (drift) actions.push({ kind: "chmod", path: f.path });
    else if (kind === "unchanged") actions.push({ kind: "unchanged", path: f.path });
  };

  const collisions = findCaseCollisions(contract.files.map((f) => f.path));
  if (collisions.length) throw new PathError(collisions.join("; "));

  for (const f of contract.files) {
    const abs = resolveInside(root, f.path);
    const prev = manifest.files[f.path];
    const current = read(abs);

    if (f.class === "external") {
      next.files[f.path] = { class: "external" };
      actions.push({ kind: "external", path: f.path });
      continue;
    }
    const content = finalContent(f, contract.pins);
    const hash = sha256(content);

    if (f.class === "seed") {
      if (current === null) {
        writes.push({ ...f, content });
        actions.push({ kind: "seed", path: f.path });
        next.files[f.path] = { class: "seed", hash };
      } else {
        actions.push({ kind: "seed-kept", path: f.path });
        next.files[f.path] = { class: "seed", hash: prev?.class === "seed" && prev.hash ? prev.hash : sha256(current) };
      }
      continue;
    }

    // owned
    next.files[f.path] = { class: "owned", hash, ...(f.removeOnDetach ? { removeOnDetach: true as const } : {}) };
    if (prev?.class === "seed" && current !== null && prev.hash !== undefined && sha256(current) !== prev.hash && !opts.force) {
      refusals.push(`${f.path}: was seeded and has been edited, turning it into an owned file would overwrite your edits (use --force)`);
      continue;
    }
    if (current === null) {
      writes.push({ ...f, content });
      actions.push({ kind: "create", path: f.path });
    } else if (prev?.class === "owned" && prev.hash !== undefined) {
      if (sha256(current) !== prev.hash && !opts.force) {
        refusals.push(`${f.path}: was edited by hand since the last render (use --force to overwrite)\n${shortDiff(current, content)}`);
      } else if (current === content) {
        settled(f, abs, "unchanged");
      } else {
        writes.push({ ...f, content });
        actions.push({ kind: "update", path: f.path });
      }
    } else if (prev?.class === "seed" && prev.hash !== undefined && sha256(current) === prev.hash) {
      // An untouched seed becomes owned: it is still exactly what was seeded, so overwriting loses nothing.
      if (current === content) settled(f, abs, "unchanged");
      else {
        writes.push({ ...f, content });
        actions.push({ kind: "update", path: f.path });
      }
    } else if (current === content) {
      settled(f, abs, "adopt");
    } else if (opts.force) {
      writes.push({ ...f, content });
      actions.push({ kind: "update", path: f.path });
    } else {
      refusals.push(`${f.path}: already exists and differs from the render, so the runner will not adopt it (use --force to overwrite)\n${shortDiff(current, content)}`);
    }
  }

  const orphans = planOrphans(contract, root, manifest, opts);
  actions.push(...orphans.actions);
  refusals.push(...orphans.refusals);
  return { actions, refusals, next, writes, chmods, deletes: orphans.deletes };
}

/** Orphans: owned files the contract no longer lists. Seed and external files are never deleted, only dropped from the manifest. */
function planOrphans(contract: Contract, root: string, manifest: Manifest, opts: Options): { actions: Action[]; refusals: string[]; deletes: string[] } {
  const out = { actions: [] as Action[], refusals: [] as string[], deletes: [] as string[] };
  for (const path of Object.keys(manifest.files)) {
    const entry = manifest.files[path];
    if (entry.class !== "owned" || contract.files.some((f) => f.path === path)) continue;
    const current = read(resolveInside(root, path));
    if (current === null) out.actions.push({ kind: "forget", path });
    else if (entry.hash !== undefined && sha256(current) !== entry.hash && !opts.force)
      out.refusals.push(`${path}: no longer rendered, but it was edited by hand since the last render, so it is not deleted (use --force)`);
    else {
      out.deletes.push(path);
      out.actions.push({ kind: "delete", path });
    }
  }
  return out;
}

/** Atomic write: the file is written inside the tool folder, on the same filesystem, then renamed into place. */
function writeAtomic(root: string, f: ContractFile): void {
  const abs = resolveInside(root, f.path);
  mkdirSync(dirname(abs), { recursive: true });
  const stage = join(root, TOOL_DIR, "stage");
  mkdirSync(stage, { recursive: true });
  const tmp = join(stage, `${process.pid}-${Math.random().toString(36).slice(2)}`);
  writeFileSync(tmp, f.content!);
  chmodSync(tmp, f.mode === "0755" ? 0o755 : 0o644);
  renameSync(tmp, abs);
}

export async function apply(contract: Contract, root: string, opts: Options = {}): Promise<Report> {
  if (!existsSync(root) || !statSync(root).isDirectory()) throw new PathError(`${root}: not a directory`);

  const release = opts.dryRun ? () => {} : acquireLock(root, opts.lock);
  try {
    return await run(contract, root, opts);
  } finally {
    release();
  }
}

async function run(contract: Contract, root: string, opts: Options): Promise<Report> {
  if (opts.repair) {
    // Rebuild the manifest from a clean render. Files that differ from the render are left out, never adopted.
    const next = emptyManifest();
    if (opts.packageVersion !== undefined) next.packageVersion = opts.packageVersion;
    const actions: Action[] = [];
    const refusals: string[] = [];
    for (const f of contract.files) {
      const abs = resolveInside(root, f.path);
      if (f.class === "external") next.files[f.path] = { class: "external" };
      else if (read(abs) === finalContent(f, contract.pins)) next.files[f.path] = { class: f.class, hash: sha256(finalContent(f, contract.pins)), ...(f.removeOnDetach ? { removeOnDetach: true as const } : {}) };
      else refusals.push(`${f.path}: ${read(abs) === null ? "missing" : "differs from the render"}, left out of the repaired manifest`);
    }
    if (!opts.dryRun) {
      mkdirSync(join(root, TOOL_DIR), { recursive: true });
      await Bun.write(join(root, MANIFEST_PATH), serialiseManifest(next));
    }
    return { actions, refusals, written: 0, deleted: 0, dryRun: !!opts.dryRun };
  }

  const manifest = await readManifest(root);
  const { actions, refusals, next, writes, chmods, deletes } = plan(contract, root, manifest, opts);
  // The delete guard runs once, here, before anything is written. --force does not override it.
  if (deletes.length) refusals.push(...deleteGuard(root, deletes));
  if (refusals.length || opts.dryRun) return { actions, refusals, written: 0, deleted: 0, dryRun: !!opts.dryRun };

  for (const f of writes) writeAtomic(root, f);
  for (const f of chmods) chmodSync(resolveInside(root, f.path), wantsExec(f) ? 0o755 : 0o644);
  for (const path of deletes) rmSync(resolveInside(root, path));
  const stage = join(root, TOOL_DIR, "stage");
  if (existsSync(stage)) rmSync(stage, { recursive: true, force: true });
  mkdirSync(join(root, TOOL_DIR), { recursive: true });
  const changed = serialiseManifest(next) !== (existsSync(join(root, MANIFEST_PATH)) ? readFileSync(join(root, MANIFEST_PATH), "utf8") : null);
  if (changed) await Bun.write(join(root, MANIFEST_PATH), serialiseManifest(next));
  return { actions, refusals, written: writes.length, deleted: deletes.length, dryRun: false };
}
