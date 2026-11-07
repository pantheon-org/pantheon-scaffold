import { existsSync, readFileSync, rmSync, rmdirSync } from "node:fs";
import { join } from "node:path";
import { MANIFEST_PATH, ManifestError, TOOL_DIR, readManifest, sha256 } from "./manifest";
import { deleteGuard } from "./guard";
import { acquireLock, type LockOptions } from "./lock";
import { resolveInside } from "./paths";

export type DetachAction = { kind: "delete" | "forget" | "manifest"; path: string };
export type DetachReport = { actions: DetachAction[]; refusals: string[]; dryRun: boolean };
export type DetachOptions = { dryRun?: boolean; force?: boolean; lock?: LockOptions };

/**
 * `detach` (decision 53): removes the manifest and the rendered files the package marked removeOnDetach (the consumer upgrade
 * workflow, which would fail on every run once detached), and leaves every other file as a plain file. It needs no pantheon.pkl
 * and no network. Deleting the flagged files goes through the same delete guard as a render.
 */
export async function detach(root: string, opts: DetachOptions = {}): Promise<DetachReport> {
  if (!existsSync(join(root, MANIFEST_PATH))) throw new ManifestError(`${MANIFEST_PATH} not found, there is nothing to detach`);
  const release = opts.dryRun ? () => {} : acquireLock(root, opts.lock);
  try {
    const manifest = await readManifest(root);
    const actions: DetachAction[] = [];
    const refusals: string[] = [];
    const deletes: string[] = [];
    for (const path of Object.keys(manifest.files).sort()) {
      const entry = manifest.files[path];
      if (!entry.removeOnDetach) continue;
      const abs = resolveInside(root, path);
      if (!existsSync(abs)) actions.push({ kind: "forget", path });
      else if (entry.hash !== undefined && sha256(readFileSync(abs, "utf8")) !== entry.hash && !opts.force)
        refusals.push(`${path}: edited by hand since the last render, so detach will not delete it (use --force)`);
      else {
        actions.push({ kind: "delete", path });
        deletes.push(path);
      }
    }
    if (deletes.length) refusals.push(...deleteGuard(root, deletes));
    actions.push({ kind: "manifest", path: MANIFEST_PATH });
    if (refusals.length || opts.dryRun) return { actions, refusals, dryRun: !!opts.dryRun };

    for (const path of deletes) rmSync(resolveInside(root, path));
    const tool = join(root, TOOL_DIR);
    for (const name of ["manifest.json", "stage", "cache"]) rmSync(join(tool, name), { recursive: true, force: true });
    release();
    try {
      rmdirSync(tool);
    } catch {
      // The folder holds something the runner did not make: leave it.
    }
    return { actions, refusals, dryRun: false };
  } finally {
    release();
  }
}
