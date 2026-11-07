import { closeSync, existsSync, mkdirSync, openSync, readFileSync, rmSync, statSync, writeSync } from "node:fs";
import { join } from "node:path";
import { TOOL_DIR } from "./manifest";

export const LOCK_PATH = `${TOOL_DIR}/lock`;
export const STALE_MS = 10 * 60_000;

export class LockError extends Error {}

export type LockOptions = { ci?: boolean; staleMs?: number };

const alive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === "EPERM";
  }
};

/**
 * Takes the write lock. Locally a lock is stale when its process is gone or it is older than `staleMs`. On CI only the age
 * counts, because the process id of another job means nothing. `check` never takes the lock, so checks run concurrently.
 * Returns a release function that is safe to call twice.
 */
export function acquireLock(root: string, opts: LockOptions = {}): () => void {
  const ci = opts.ci ?? !!process.env.CI;
  const staleMs = opts.staleMs ?? STALE_MS;
  const path = join(root, LOCK_PATH);
  mkdirSync(join(root, TOOL_DIR), { recursive: true });

  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      const fd = openSync(path, "wx");
      writeSync(fd, JSON.stringify({ pid: process.pid, started: new Date().toISOString() }));
      closeSync(fd);
      let released = false;
      return () => {
        if (released) return;
        released = true;
        rmSync(path, { force: true });
      };
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "EEXIST") throw e;
    }
    if (!existsSync(path)) continue;
    let pid: number | undefined;
    let startedMs: number;
    try {
      const held = JSON.parse(readFileSync(path, "utf8")) as { pid: number; started: string };
      pid = held.pid;
      startedMs = Date.parse(held.started);
      if (Number.isNaN(startedMs)) throw new Error("bad date");
    } catch {
      startedMs = statSync(path).mtimeMs;
    }
    const old = Date.now() - startedMs > staleMs;
    const gone = !ci && pid !== undefined && !alive(pid);
    if (!(old || gone)) throw new LockError(`${LOCK_PATH} is held${pid !== undefined ? ` by process ${pid}` : ""} since ${new Date(startedMs).toISOString()}, another run is writing`);
    rmSync(path, { force: true });
  }
  throw new LockError(`${LOCK_PATH} could not be taken, another run won the race`);
}
