import { afterEach, expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { packageVersionOf } from "../src/preflight";
import { evalContractJson } from "../src/pkl";
import { cleanupRepos, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const root = join(import.meta.dir, "../..");
const URI = "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v0.0.0/pantheon-scaffold@0.0.0#/Config.pkl";

/** Builds the real package, serves it locally in place of GitHub (as spike S.1 did) and renders a consumer file through it. */
async function renderThroughPackage(consumerBody: string) {
  const work = tempRepo();
  const out = join(work, "out");
  const build = Bun.spawnSync(["pkl", "project", "package", join(root, "pkl"), "--output-path", out], { stdout: "pipe", stderr: "pipe" });
  if (build.exitCode !== 0) throw new Error(build.stderr.toString());
  const server = Bun.serve({ port: 0, fetch: (req) => new Response(Bun.file(join(out, decodeURIComponent(basename(new URL(req.url).pathname))))) });
  try {
    mkdirSync(join(work, "consumer"));
    writeFileSync(join(work, "consumer/pantheon.pkl"), `amends "${URI}"\n${consumerBody}\n`);
    // Async on purpose: a blocking spawn would stop this process from answering pkl's download request.
    const p = Bun.spawn(
      ["pkl", "eval", "-f", "json", "--cache-dir", join(work, "cache"), "--http-rewrite", `https://github.com/=http://localhost:${server.port}/`, "--allowed-resources", "package:,https:,http:", join(work, "consumer/pantheon.pkl")],
      { stdout: "pipe", stderr: "pipe" },
    );
    const [out2, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
    if (code !== 0) throw new Error(err);
    return JSON.parse(out2);
  } finally {
    server.stop(true);
  }
}

test("a package loaded from a package URI reports that URI, and the runner reads its version from it", async () => {
  const json = await renderThroughPackage(`options { toolchain = "1.96.1" }`);
  expect(json.packageUri).toBe(URI);
  expect(packageVersionOf(json.packageUri)).toBe("0.0.0");
  expect(json.requires).toEqual({ runner: "0.0.0" });
});

test("a package loaded from a local folder has no packageUri", async () => {
  const json = (await evalContractJson(join(import.meta.dir, "fixtures/minimal/pantheon.pkl"), root)) as Record<string, unknown>;
  expect(json.packageUri).toBeUndefined();
  expect(json.requires).toEqual({ runner: "0.0.0" });
});

test("a file marked removeOnDetach reaches the contract, and an unmarked one does not carry the key", async () => {
  const json = await renderThroughPackage(`options { toolchain = "1.96.1" }\nfiles { ["up.txt"] { content = "x\\n"; removeOnDetach = true }\n["b.txt"] { content = "y\\n" } }`);
  const byPath = Object.fromEntries(json.files.map((f: { path: string }) => [f.path, f]));
  expect(byPath["up.txt"].removeOnDetach).toBe(true);
  expect("removeOnDetach" in byPath["b.txt"]).toBe(false);
});
