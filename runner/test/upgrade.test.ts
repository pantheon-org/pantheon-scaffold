import { afterEach, expect, test } from "bun:test";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { LockError } from "../src/lock";
import { RUNNER_TOOL } from "../src/pins";
import { UpgradeError, upgrade, type Deps } from "../src/upgrade";
import { cleanupRepos, get, has, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const h = (c: string) => c.repeat(64);
const BASE = "https://example.test/releases/download";
const amends = (v: string, sum: string) =>
  `amends "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${v}/pantheon-scaffold@${v}::sha256:${sum}#/Config.pkl"\n\noptions { toolchain = "1.96.1" }\n`;
const mise = (v: string) => `[tools]\nbun = "1.4.2"\n"${RUNNER_TOOL}" = "${v}"\n`;

function repo(v = "0.1.0") {
  const root = tempRepo();
  put(root, "pantheon.pkl", amends(v, h("a")));
  put(root, "mise.toml", mise(v));
  put(root, "mise.lock", "# lock for 0.1.0\n");
  return root;
}

type Fake = { calls: string[]; verified: string[]; deps: Deps };
/** Fakes for the three things upgrade reaches outside the repository for: the checksums download, the attestation check and mise. */
function fakes(o: { text?: string; failFetch?: boolean; failVerify?: boolean; exit?: Record<string, number>; onExec?: (cmd: string) => void } = {}): Fake {
  const calls: string[] = [];
  const verified: string[] = [];
  const text = o.text ?? `${h("c")}  pantheon-scaffold@0.2.0\n${h("d")}  runner-linux-x64\n`;
  return {
    calls,
    verified,
    deps: {
      fetchText: async (url) => {
        calls.push(`fetch ${url}`);
        if (o.failFetch) throw new Error("HTTP 404");
        return text;
      },
      verifyAttestation: async (file, repoName) => {
        calls.push(`verify ${repoName}`);
        verified.push(readFileSync(file, "utf8"));
        if (o.failVerify) throw new Error("no valid attestation");
      },
      exec: async (cmd, _cwd) => {
        const line = cmd.join(" ");
        calls.push(`exec ${line}`);
        o.onExec?.(line);
        return o.exit?.[line] ?? 0;
      },
    },
  };
}
const snapshot = (root: string) => ({ pkl: get(root, "pantheon.pkl"), mise: get(root, "mise.toml"), lock: get(root, "mise.lock") });
const run = (root: string, f: Fake, version = "0.2.0", extra: object = {}) => upgrade(root, version, { baseUrl: BASE, deps: f.deps, ...extra });

test("upgrade rewrites the pins, refreshes mise and re-renders with the new runner, in that order", async () => {
  const root = repo();
  const f = fakes();
  const r = await run(root, f);
  expect(get(root, "pantheon.pkl")).toBe(amends("0.2.0", h("c")));
  expect(get(root, "mise.toml")).toBe(mise("0.2.0"));
  expect(f.calls).toEqual([
    `fetch ${BASE}/v0.2.0/checksums.txt`,
    "verify pantheon-org/pantheon-scaffold",
    "exec mise install",
    "exec mise lock",
    `exec mise x -- pantheon-scaffold render pantheon.pkl --root ${root}`,
  ]);
  expect(r).toMatchObject({ from: "0.1.0", to: "0.2.0", dryRun: false, renderExit: 0 });
});

test("the attestation is checked on the exact text that was downloaded, before any file is touched", async () => {
  const root = repo();
  const f = fakes({ failVerify: true });
  const before = snapshot(root);
  await expect(run(root, f)).rejects.toThrow("no valid attestation");
  expect(f.verified[0]).toContain(`${h("c")}  pantheon-scaffold@0.2.0`);
  expect(snapshot(root)).toEqual(before);
  expect(f.calls.some((c) => c.startsWith("exec"))).toBe(false);
});

test("a checksum for the package that is missing, repeated or malformed stops the upgrade with nothing written", async () => {
  for (const text of [`${h("d")}  runner-linux-x64\n`, `${h("c")}  pantheon-scaffold@0.2.0\n${h("e")}  pantheon-scaffold@0.2.0\n`, "garbage\n"]) {
    const root = repo();
    const before = snapshot(root);
    await expect(run(root, fakes({ text }))).rejects.toThrow(UpgradeError);
    expect(snapshot(root)).toEqual(before);
  }
});

test("a download that fails stops the upgrade closed", async () => {
  const root = repo();
  const before = snapshot(root);
  await expect(run(root, fakes({ failFetch: true }))).rejects.toThrow("could not fetch");
  expect(snapshot(root)).toEqual(before);
});

test("when mise install fails every file is put back exactly as it was", async () => {
  const root = repo();
  const before = snapshot(root);
  await expect(run(root, fakes({ exit: { "mise install": 1 } }))).rejects.toThrow("mise install failed");
  expect(snapshot(root)).toEqual(before);
});

test("when mise lock fails the lock file mise rewrote is restored too", async () => {
  const root = repo();
  const before = snapshot(root);
  const f = fakes({ exit: { "mise lock": 1 }, onExec: (c) => c === "mise lock" && writeFileSync(join(root, "mise.lock"), "# half written\n") });
  await expect(run(root, f)).rejects.toThrow("mise lock failed");
  expect(snapshot(root)).toEqual(before);
});

test("a lock file that did not exist before is removed again on failure", async () => {
  const root = repo();
  const { rmSync } = await import("node:fs");
  rmSync(join(root, "mise.lock"));
  const f = fakes({ exit: { "mise lock": 1 }, onExec: (c) => c === "mise lock" && writeFileSync(join(root, "mise.lock"), "# new\n") });
  await expect(run(root, f)).rejects.toThrow(UpgradeError);
  expect(has(root, "mise.lock")).toBe(false);
});

test("a render that fails afterwards leaves the new pins in place and reports its exit code", async () => {
  const root = repo();
  const f = fakes({ exit: { [`mise x -- pantheon-scaffold render pantheon.pkl --root ${root}`]: 3 } });
  const r = await run(root, f);
  expect(r.renderExit).toBe(3);
  expect(get(root, "mise.toml")).toBe(mise("0.2.0"));
  expect(get(root, "pantheon.pkl")).toBe(amends("0.2.0", h("c")));
});

test("--no-render stops after mise", async () => {
  const root = repo();
  const f = fakes();
  const r = await run(root, f, "0.2.0", { noRender: true });
  expect(f.calls.filter((c) => c.startsWith("exec"))).toEqual(["exec mise install", "exec mise lock"]);
  expect(r.renderExit).toBeUndefined();
});

test("--dry-run downloads and verifies, lists what would change, and writes and runs nothing", async () => {
  const root = repo();
  const before = snapshot(root);
  const f = fakes();
  const r = await run(root, f, "0.2.0", { dryRun: true });
  expect(snapshot(root)).toEqual(before);
  expect(f.calls.map((c) => c.split(" ")[0])).toEqual(["fetch", "verify"]);
  expect(r.dryRun).toBe(true);
  expect(r.changes.join("\n")).toContain("0.1.0 -> 0.2.0");
});

test("the same version is refused, and an older one needs --force", async () => {
  const root = repo("0.2.0");
  await expect(run(root, fakes())).rejects.toThrow("already at 0.2.0");
  const older = repo("0.3.0");
  await expect(run(older, fakes())).rejects.toThrow("older than 0.3.0");
  const forced = await run(older, fakes(), "0.2.0", { force: true });
  expect(forced.to).toBe("0.2.0");
});

test("a pre-release is an exact pin like any version, and a leading v is accepted", async () => {
  const root = repo();
  const f = fakes({ text: `${h("c")}  pantheon-scaffold@0.2.0-rc.1\n` });
  const r = await run(root, f, "v0.2.0-rc.1");
  expect(r.to).toBe("0.2.0-rc.1");
  expect(get(root, "pantheon.pkl")).toContain("pantheon-scaffold@0.2.0-rc.1::sha256:");
});

test("a version that is not a version is refused before anything is fetched", async () => {
  const f = fakes();
  await expect(run(repo(), f, "latest")).rejects.toThrow("not a version");
  expect(f.calls).toEqual([]);
});

test("a repository whose pins cannot be found is an error before anything is fetched", async () => {
  const root = repo();
  put(root, "mise.toml", `[tools]\nbun = "1.4.2"\n`);
  const f = fakes();
  await expect(run(root, f)).rejects.toThrow(RUNNER_TOOL);
  expect(f.calls).toEqual([]);
});

test("a custom config file name is used for the pins and the render", async () => {
  const root = repo();
  put(root, "other.pkl", get(root, "pantheon.pkl"));
  const f = fakes();
  await run(root, f, "0.2.0", { config: "other.pkl" });
  expect(get(root, "other.pkl")).toContain("v0.2.0");
  expect(f.calls.at(-1)).toContain("render other.pkl");
});

test("an upgrade refuses while another run holds the lock", async () => {
  const root = repo();
  put(root, ".pkl-gen/lock", JSON.stringify({ pid: process.pid, started: new Date().toISOString() }));
  const f = fakes();
  await expect(run(root, f)).rejects.toThrow(LockError);
  expect(f.calls.some((c) => c.startsWith("exec"))).toBe(false);
});

test("the lock is released before the new runner renders, so its own render can take it", async () => {
  const root = repo();
  let lockedDuringRender: boolean | undefined;
  const f = fakes({ onExec: (c) => c.includes("render") && (lockedDuringRender = existsSync(join(root, ".pkl-gen/lock"))) });
  await run(root, f);
  expect(lockedDuringRender).toBe(false);
});

// What mise lock records for the runner is checked after it runs. Both locks are real output for the published v0.1.1.
const lockFixture = (name: string) => readFileSync(join(import.meta.dir, "fixtures", name), "utf8");
const lockWrites = (root: string, name: string) => (c: string) => c === "mise lock" && writeFileSync(join(root, "mise.lock"), lockFixture(name));

test("a lock that names platforms the runner is not built for fails the upgrade and puts everything back", async () => {
  const root = repo();
  const before = snapshot(root);
  const f = fakes({ onExec: lockWrites(root, "mise-lock-all-platforms.lock") });
  const err = await run(root, f).then(() => null, (e: Error) => e);
  expect(err).toBeInstanceOf(UpgradeError);
  expect(err!.message).toContain("windows-x64");
  expect(err!.message).toContain("macos-x64");
  expect(err!.message).toContain("lockfile_platforms");
  expect(snapshot(root)).toEqual(before);
  expect(f.calls.some((c) => c.includes("render"))).toBe(false);
});

test("a lock limited to the supported platforms passes and is kept", async () => {
  const root = repo();
  const f = fakes({ onExec: lockWrites(root, "mise-lock-three-platforms.lock") });
  const r = await run(root, f);
  expect(r.renderExit).toBe(0);
  expect(get(root, "mise.lock")).toBe(lockFixture("mise-lock-three-platforms.lock"));
});

test("a lock with no entry for the runner is not an error", async () => {
  const root = repo();
  const r = await run(root, fakes());
  expect(r.renderExit).toBe(0);
});

// Phase 7 finding: the imports of the package (Builtins.pkl) have to move with the amends line, and the report says how many did.
test("upgrade moves every import of the package with the amends line and says so", async () => {
  const root = repo();
  const imp = (v: string, sum: string) => `import "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${v}/pantheon-scaffold@${v}::sha256:${sum}#/Builtins.pkl"`;
  put(root, "pantheon.pkl", `${amends("0.1.0", h("a")).split("\n")[0]}\n${imp("0.1.0", h("a"))}\n\noptions { toolchain = "1.96.1" }\n`);
  const r = await run(root, fakes());
  expect(get(root, "pantheon.pkl")).toContain(imp("0.2.0", h("c")));
  expect(get(root, "pantheon.pkl")).not.toContain("0.1.0");
  expect(r.changes[0]).toContain("1 import");
});

test("a dry run lists the imports it would move and changes nothing", async () => {
  const root = repo();
  const imp = `import "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.1.0/pantheon-scaffold@0.1.0::sha256:${h("a")}#/Builtins.pkl"`;
  const text = `${amends("0.1.0", h("a")).split("\n")[0]}\n${imp}\n`;
  put(root, "pantheon.pkl", text);
  const r = await run(root, fakes(), "0.2.0", { dryRun: true });
  expect(r.changes[0]).toContain("1 import");
  expect(get(root, "pantheon.pkl")).toBe(text);
});
