import { expect, test } from "bun:test";
import { join } from "node:path";
import { cleanupRepos, tempRepo } from "./helpers";
import { afterEach } from "bun:test";

afterEach(cleanupRepos);

const cli = join(import.meta.dir, "../src/cli.ts");
const releases = (...x: [string, boolean][]) => JSON.stringify(x.map(([tag, pre]) => ({ tag_name: tag, draft: false, prerelease: pre })));

async function run(list: string | null, knownBad: string, ...args: string[]) {
  const server = Bun.serve({
    port: 0,
    fetch: (req) => {
      const path = new URL(req.url).pathname;
      if (path === "/known-bad.json") return new Response(knownBad);
      return list === null ? new Response("gone", { status: 404 }) : new Response(list);
    },
  });
  try {
    const root = tempRepo();
    const base = `http://localhost:${server.port}`;
    const p = Bun.spawn(["bun", cli, "latest", "--url", `${base}/releases`, "--root", root, "--attempts", "1", ...args], {
      stdout: "pipe", stderr: "pipe", env: { ...process.env, PANTHEON_SCAFFOLD_KNOWN_BAD_URL: `${base}/known-bad.json` },
    });
    const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
    return { out: out.trim(), err: err.trim(), code };
  } finally {
    server.stop(true);
  }
}
const none = JSON.stringify({ versions: [] });

test("it prints the newest stable release without the v", async () => {
  const r = await run(releases(["v0.1.0", false], ["v0.2.0", false], ["v0.3.0", true]), none);
  expect(r).toMatchObject({ out: "0.2.0", code: 0 });
});

test("--include-prereleases offers a pre-release", async () => {
  const r = await run(releases(["v0.1.0", false], ["v0.3.0", true]), none, "--include-prereleases");
  expect(r).toMatchObject({ out: "0.3.0", code: 0 });
});

test("a version on the known-bad list is skipped", async () => {
  const bad = JSON.stringify({ versions: [{ version: "0.2.0", reason: "broken" }] });
  expect((await run(releases(["v0.1.0", false], ["v0.2.0", false]), bad)).out).toBe("0.1.0");
});

test("no qualifying release is exit 1 and says so", async () => {
  const r = await run(releases(["v0.1.0", true]), none);
  expect(r.code).toBe(1);
  expect(r.err).toContain("no release found");
});

test("a list that cannot be fetched fails closed with exit 1 and a clean message", async () => {
  const r = await run(null, none);
  expect(r.code).toBe(1);
  expect(r.err).toContain("failing closed");
  expect(r.err).not.toContain("    at ");
});

test("the usage text lists the command", async () => {
  const p = Bun.spawn(["bun", cli, "nope"], { stdout: "pipe", stderr: "pipe" });
  expect(await new Response(p.stderr).text()).toContain("pantheon-scaffold latest");
});
