import { afterEach, expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { RUNNER_TOOL } from "../src/pins";
import { cleanupRepos, get, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const cli = join(import.meta.dir, "../src/cli.ts");
const h = (c: string) => c.repeat(64);
const amends = (v: string, sum: string) => `amends "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${v}/pantheon-scaffold@${v}::sha256:${sum}#/Config.pkl"\n`;
const mise = (v: string) => `[tools]\n"${RUNNER_TOOL}" = "${v}"\n`;

/** A local server in place of the release, and fake mise and gh programs that record their arguments. */
function setup(opts: { checksums?: string | null } = {}) {
  const bin = tempRepo();
  const calls = join(bin, "calls");
  writeFileSync(calls, "");
  for (const name of ["mise", "gh"]) {
    writeFileSync(join(bin, name), `#!/bin/sh\necho "${name} $*" >> "${calls}"\nif [ "${name}" = gh ] && [ -n "$GH_FAIL" ]; then echo "no valid attestation" >&2; exit 1; fi\nif [ "${name}" = mise ] && [ "$1" = x ] && [ -n "$MISE_RENDER_EXIT" ]; then exit "$MISE_RENDER_EXIT"; fi\nexit 0\n`);
    chmodSync(join(bin, name), 0o755);
  }
  const body = opts.checksums === undefined ? `${h("c")}  pantheon-scaffold@0.2.0\n` : opts.checksums;
  const server = Bun.serve({ port: 0, fetch: (req) => (body !== null && new URL(req.url).pathname === "/v0.2.0/checksums.txt" ? new Response(body) : new Response("gone", { status: 404 })) });
  const repo = tempRepo();
  put(repo, "pantheon.pkl", amends("0.1.0", h("a")));
  put(repo, "mise.toml", mise("0.1.0"));
  const run = async (args: string[], env: Record<string, string> = {}) => {
    const p = Bun.spawn(["bun", cli, ...args], { cwd: repo, stdout: "pipe", stderr: "pipe", env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, PANTHEON_SCAFFOLD_RELEASE_BASE: `http://localhost:${server.port}`, ...env } });
    const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
    return { out: out.trim(), err: err.trim(), code };
  };
  return { repo, run, calls: () => readFileSync(calls, "utf8").trim().split("\n").filter(Boolean), stop: () => server.stop(true) };
}

test("upgrade verifies the release, rewrites the pins, refreshes mise and re-renders", async () => {
  const s = setup();
  try {
    const r = await s.run(["upgrade", "0.2.0", "--root", s.repo]);
    expect(r.code).toBe(0);
    expect(r.out).toContain("0.1.0 -> 0.2.0");
    expect(get(s.repo, "pantheon.pkl")).toBe(amends("0.2.0", h("c")));
    expect(get(s.repo, "mise.toml")).toBe(mise("0.2.0"));
    const calls = s.calls();
    expect(calls[0]).toMatch(/^gh attestation verify \S+checksums\.txt --repo pantheon-org\/pantheon-scaffold$/);
    expect(calls.slice(1)).toEqual(["mise install", "mise lock", `mise x -- pantheon-scaffold render pantheon.pkl --root ${s.repo}`]);
  } finally {
    s.stop();
  }
});

test("a failed attestation stops with exit 1, a clean message, and no file touched and mise never run", async () => {
  const s = setup();
  try {
    const before = get(s.repo, "pantheon.pkl");
    const r = await s.run(["upgrade", "0.2.0", "--root", s.repo], { GH_FAIL: "1" });
    expect(r.code).toBe(1);
    expect(r.err).toContain("attestation check failed");
    expect(r.err).not.toContain("    at ");
    expect(get(s.repo, "pantheon.pkl")).toBe(before);
    expect(s.calls().filter((c) => c.startsWith("mise"))).toEqual([]);
  } finally {
    s.stop();
  }
});

test("the exit code of the re-render is the exit code of upgrade", async () => {
  const s = setup();
  try {
    expect((await s.run(["upgrade", "0.2.0", "--root", s.repo], { MISE_RENDER_EXIT: "3" })).code).toBe(3);
    expect(get(s.repo, "mise.toml")).toBe(mise("0.2.0"));
  } finally {
    s.stop();
  }
});

test("--dry-run verifies and lists the change and writes and runs nothing else", async () => {
  const s = setup();
  try {
    const r = await s.run(["upgrade", "0.2.0", "--root", s.repo, "--dry-run"]);
    expect(r.code).toBe(0);
    expect(r.out).toContain("dry run");
    expect(get(s.repo, "mise.toml")).toBe(mise("0.1.0"));
    expect(s.calls().every((c) => c.startsWith("gh"))).toBe(true);
  } finally {
    s.stop();
  }
});

test("--no-render stops after mise", async () => {
  const s = setup();
  try {
    expect((await s.run(["upgrade", "0.2.0", "--root", s.repo, "--no-render"])).code).toBe(0);
    expect(s.calls().some((c) => c.includes("render"))).toBe(false);
  } finally {
    s.stop();
  }
});

test("a release that cannot be fetched fails closed with exit 1", async () => {
  const s = setup({ checksums: null });
  try {
    const r = await s.run(["upgrade", "0.2.0", "--root", s.repo]);
    expect(r.code).toBe(1);
    expect(r.err).toContain("could not fetch");
    expect(get(s.repo, "mise.toml")).toBe(mise("0.1.0"));
  } finally {
    s.stop();
  }
});

test("a missing version, an unknown option and a version that is not one", async () => {
  const s = setup();
  try {
    expect((await s.run(["upgrade", "--root", s.repo])).code).toBe(2);
    expect((await s.run(["upgrade", "0.2.0", "--nope"])).code).toBe(2);
    expect((await s.run(["upgrade", "latest", "--root", s.repo])).code).toBe(1);
  } finally {
    s.stop();
  }
});
