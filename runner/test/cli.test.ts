import { expect, test } from "bun:test";
import { join } from "node:path";

const cli = join(import.meta.dir, "../src/cli.ts");
const root = join(import.meta.dir, "../..");
const run = async (...args: string[]) => {
  const p = Bun.spawn(["bun", cli, ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};

test("--version prints the version", async () => expect(await run("--version")).toMatchObject({ out: "0.0.0", code: 0 }));
test("contract summarises a valid fixture", async () => {
  const r = await run("contract", "runner/test/fixtures/minimal/pantheon.pkl");
  expect(r.code).toBe(0);
  expect(r.out).toBe("contract ok: 4 files, 1 pins, escape hatch 0 of 4 constructs");
});
test("contract fails with exit 1 on a refused read", async () => {
  const r = await run("contract", "runner/test/fixtures/read-file/pantheon.pkl");
  expect(r.code).toBe(1);
  expect(r.err).toContain("Refusing to read resource");
});
test("an unknown command prints usage with exit 2", async () => expect((await run("nope")).code).toBe(2));
