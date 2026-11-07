import { afterEach, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { LockError, acquireLock } from "../src/lock";
import { cleanupRepos, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const LOCK = ".pkl-gen/lock";
const deadPid = (): number => Bun.spawnSync(["true"]).pid;
const writeLock = (root: string, pid: number, startedMsAgo: number) =>
  put(root, LOCK, JSON.stringify({ pid, started: new Date(Date.now() - startedMsAgo).toISOString() }));

test("acquiring creates the lock file and releasing removes it", () => {
  const root = tempRepo();
  const release = acquireLock(root);
  expect(existsSync(join(root, LOCK))).toBe(true);
  release();
  expect(existsSync(join(root, LOCK))).toBe(false);
});

test("a second acquire while the lock is held fails and names the holder", () => {
  const root = tempRepo();
  const release = acquireLock(root);
  expect(() => acquireLock(root)).toThrow(LockError);
  expect(() => acquireLock(root)).toThrow(String(process.pid));
  release();
});

test("releasing twice is harmless", () => {
  const root = tempRepo();
  const release = acquireLock(root);
  release();
  expect(() => release()).not.toThrow();
});

test("locally, a lock held by a process that no longer exists is recovered", () => {
  const root = tempRepo();
  writeLock(root, deadPid(), 1000);
  const release = acquireLock(root, { ci: false });
  expect(existsSync(join(root, LOCK))).toBe(true);
  release();
});

test("locally, a lock held by a live process that is young is respected", () => {
  const root = tempRepo();
  writeLock(root, process.pid, 1000);
  expect(() => acquireLock(root, { ci: false })).toThrow(LockError);
});

test("locally, an old lock is recovered even if the process id is alive", () => {
  const root = tempRepo();
  writeLock(root, process.pid, 11 * 60_000);
  expect(() => acquireLock(root, { ci: false })()).not.toThrow();
});

test("on CI the process id is ignored: a young lock of a dead process is still respected", () => {
  const root = tempRepo();
  writeLock(root, deadPid(), 1000);
  expect(() => acquireLock(root, { ci: true })).toThrow(LockError);
});

test("on CI an old lock is recovered", () => {
  const root = tempRepo();
  writeLock(root, deadPid(), 11 * 60_000);
  expect(() => acquireLock(root, { ci: true })()).not.toThrow();
});

test("the stale age is configurable", () => {
  const root = tempRepo();
  writeLock(root, process.pid, 5000);
  expect(() => acquireLock(root, { ci: false, staleMs: 1000 })()).not.toThrow();
});

test("an unreadable lock file is treated as stale only when old, never silently deleted while young", () => {
  const root = tempRepo();
  put(root, LOCK, "not json");
  expect(() => acquireLock(root)).toThrow(LockError);
});
