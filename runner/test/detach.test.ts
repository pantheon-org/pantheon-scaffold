import { afterEach, expect, test } from "bun:test";
import { apply } from "../src/apply";
import { detach } from "../src/detach";
import { LockError } from "../src/lock";
import { ManifestError } from "../src/manifest";
import { cleanupRepos, commitAll, contractOf, external, get, gitRepo, has, owned, put, seed, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const withUpgrade = () => contractOf({ ...owned(".github/workflows/upgrade.yml", "name: upgrade\n"), removeOnDetach: true }, owned("deny.toml", "# deny\n"), seed("pantheon.pkl", "# mine\n"), external("mise.lock"));
const managed = async () => {
  const root = gitRepo({ ".gitignore": ".pkl-gen/lock\n.pkl-gen/stage/\n.pkl-gen/cache/\n" });
  await apply(withUpgrade(), root);
  put(root, "mise.lock", "lock\n");
  commitAll(root, "render");
  return root;
};
const kinds = (r: Awaited<ReturnType<typeof detach>>) => r.actions.map((a) => `${a.kind}:${a.path}`);

test("detach removes the manifest and the flagged upgrade workflow and leaves every other file untouched", async () => {
  const root = await managed();
  const before = { deny: get(root, "deny.toml"), pkl: get(root, "pantheon.pkl"), lock: get(root, "mise.lock") };
  const r = await detach(root);
  expect(kinds(r)).toEqual(["delete:.github/workflows/upgrade.yml", "manifest:.pkl-gen/manifest.json"]);
  expect(has(root, ".github/workflows/upgrade.yml")).toBe(false);
  expect(has(root, ".pkl-gen")).toBe(false);
  expect({ deny: get(root, "deny.toml"), pkl: get(root, "pantheon.pkl"), lock: get(root, "mise.lock") }).toEqual(before);
});

test("the cache, stage and lock folders go with the manifest", async () => {
  const root = await managed();
  put(root, ".pkl-gen/cache/known-bad.json", "{}");
  put(root, ".pkl-gen/stage/x", "x");
  await detach(root);
  expect(has(root, ".pkl-gen")).toBe(false);
});

test("--dry-run lists the plan and changes nothing", async () => {
  const root = await managed();
  const r = await detach(root, { dryRun: true });
  expect(kinds(r)).toContain("delete:.github/workflows/upgrade.yml");
  expect(has(root, ".github/workflows/upgrade.yml")).toBe(true);
  expect(has(root, ".pkl-gen/manifest.json")).toBe(true);
});

test("a hand-edited upgrade workflow is refused and nothing is removed, and --force removes it", async () => {
  const root = await managed();
  put(root, ".github/workflows/upgrade.yml", "name: mine\n");
  commitAll(root, "edit");
  const refused = await detach(root);
  expect(refused.refusals.join()).toContain("edited by hand");
  expect(has(root, ".pkl-gen/manifest.json")).toBe(true);
  expect(has(root, ".github/workflows/upgrade.yml")).toBe(true);
  expect(kinds(await detach(root, { force: true }))).toContain("delete:.github/workflows/upgrade.yml");
  expect(has(root, ".pkl-gen")).toBe(false);
});

test("a dirty tracked tree refuses, the delete guard applies to detach too", async () => {
  const root = await managed();
  put(root, "deny.toml", "# changed\n");
  const r = await detach(root);
  expect(r.refusals.join()).toContain("not clean");
  expect(has(root, ".pkl-gen/manifest.json")).toBe(true);
});

test("a workflow that is already gone is forgotten and the manifest still goes", async () => {
  const root = await managed();
  const { rmSync } = await import("node:fs");
  rmSync(`${root}/.github/workflows/upgrade.yml`);
  commitAll(root, "removed by hand");
  expect(kinds(await detach(root))).toEqual(["forget:.github/workflows/upgrade.yml", "manifest:.pkl-gen/manifest.json"]);
  expect(has(root, ".pkl-gen")).toBe(false);
});

test("with no flagged file git is not needed", async () => {
  const root = tempRepo();
  await apply(contractOf(owned("a.txt", "x\n")), root);
  expect(kinds(await detach(root))).toEqual(["manifest:.pkl-gen/manifest.json"]);
  expect(get(root, "a.txt")).toBe("x\n");
});

test("a repository with no manifest has nothing to detach", async () => {
  await expect(detach(tempRepo())).rejects.toThrow(ManifestError);
});

test("detach refuses while another run holds the lock", async () => {
  const root = await managed();
  put(root, ".pkl-gen/lock", JSON.stringify({ pid: process.pid, started: new Date().toISOString() }));
  await expect(detach(root)).rejects.toThrow(LockError);
  expect(has(root, ".github/workflows/upgrade.yml")).toBe(true);
});

test("after a detach the files can be adopted again by a fresh render", async () => {
  const root = await managed();
  await detach(root);
  const r = await apply(withUpgrade(), root);
  expect(r.refusals).toEqual([]);
  expect(r.actions.map((a) => a.kind)).toContain("adopt");
});
