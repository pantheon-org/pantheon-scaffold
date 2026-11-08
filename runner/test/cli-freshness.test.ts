import { expect, test } from "bun:test";
import { join } from "node:path";

const cli = join(import.meta.dir, "../src/cli.ts");
const DAY = 86_400_000;
const record = (ageMs: number) => JSON.stringify({ schemaVersion: 1, lastSuccessfulPinCheck: new Date(Date.now() - ageMs).toISOString().replace(/\.\d{3}Z$/, "Z") });

async function run(body: string | null, ...args: string[]) {
  const server = Bun.serve({ port: 0, fetch: () => (body === null ? new Response("gone", { status: 404 }) : new Response(body)) });
  try {
    const p = Bun.spawn(["bun", cli, "freshness", "--url", `http://localhost:${server.port}/freshness.json`, "--attempts", "1", ...args], { stdout: "pipe", stderr: "pipe" });
    const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
    return { out: out.trim(), err: err.trim(), code };
  } finally {
    server.stop(true);
  }
}

test("a fresh record exits 0 and says how old it is", async () => {
  const r = await run(record(3 * DAY));
  expect(r.code).toBe(0);
  expect(r.out).toMatch(/^fresh: the last successful pin check was 3 day\(s\) ago \(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\)$/);
});

test("a stale record exits 3 and the message says it is stale and by how much", async () => {
  const r = await run(record(45 * DAY));
  expect(r.code).toBe(3);
  expect(r.err).toContain("stale: the last successful pin check was 45 day(s) ago");
  expect(r.err).toContain("more than 30");
});

test("the limit can be changed", async () => {
  expect((await run(record(5 * DAY), "--max-age-days", "3")).code).toBe(3);
  expect((await run(record(5 * DAY), "--max-age-days", "7")).code).toBe(0);
});

test("a record that cannot be fetched fails closed with exit 1 and a clean message", async () => {
  const r = await run(null);
  expect(r.code).toBe(1);
  expect(r.err).toContain("failing closed");
  expect(r.err).not.toContain("    at ");
});

test("a record that is not valid is exit 1", async () => {
  const r = await run("garbage");
  expect(r.code).toBe(1);
  expect(r.err).toContain("not valid JSON");
});

test("a limit that is not a whole number of days is a usage error", async () => {
  expect((await run(record(DAY), "--max-age-days", "0")).code).toBe(2);
  expect((await run(record(DAY), "--max-age-days", "soon")).code).toBe(2);
});

test("the usage text lists the command", async () => {
  const p = Bun.spawn(["bun", cli, "nope"], { stdout: "pipe", stderr: "pipe" });
  const err = await new Response(p.stderr).text();
  expect(err).toContain("pantheon-scaffold freshness");
});
