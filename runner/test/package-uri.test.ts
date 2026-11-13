import { afterEach, expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { packageVersionOf } from "../src/preflight";
import { pklProjectVersion } from "../src/release";
import { evalContractJson } from "./support/contract";
import { cleanupRepos, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const root = join(import.meta.dir, "../..");
// The version comes from pkl/PklProject, so this holds for every release.
const VERSION = pklProjectVersion(readFileSync(join(root, "pkl/PklProject"), "utf8"));
const URI = `package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${VERSION}/pantheon-scaffold@${VERSION}#/Config.pkl`;

/** Builds the real package, serves it locally in place of GitHub (as spike S.1 did) and renders a consumer file through it. */
async function renderThroughPackage(consumerBody: string, opts: { pinChecksum?: boolean } = {}) {
  const work = tempRepo();
  const out = join(work, "out");
  const build = Bun.spawnSync(["pkl", "project", "package", join(root, "pkl"), "--output-path", out], { stdout: "pipe", stderr: "pipe" });
  if (build.exitCode !== 0) throw new Error(build.stderr.toString());
  const server = Bun.serve({ port: 0, fetch: (req) => new Response(Bun.file(join(out, decodeURIComponent(basename(new URL(req.url).pathname))))) });
  try {
    mkdirSync(join(work, "consumer"));
    // A real consumer pins the package with the checksum of its metadata file, which lands in the URI before the #.
    const metaSha = new Bun.CryptoHasher("sha256").update(readFileSync(join(out, `pantheon-scaffold@${VERSION}`))).digest("hex");
    const uri = opts.pinChecksum ? URI.replace("#/Config.pkl", `::sha256:${metaSha}#/Config.pkl`) : URI;
    // @@BUILTINS@@ in a consumer body is the package's own Builtins module, loaded from the same package URI.
    const body = consumerBody.replaceAll("@@BUILTINS@@", uri.replace("#/Config.pkl", "#/Builtins.pkl"));
    writeFileSync(join(work, "consumer/pantheon.pkl"), `amends "${uri}"\n${body}\n`);
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
  expect(packageVersionOf(json.packageUri)).toBe(VERSION);
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

test("a consumer that pins the package with its checksum still gets a packageUri the runner can read the version from", async () => {
  const json = await renderThroughPackage(`options { toolchain = "1.96.1" }`, { pinChecksum: true });
  expect(json.packageUri).toMatch(/::sha256:[0-9a-f]{64}#\/Config\.pkl$/);
  expect(packageVersionOf(json.packageUri)).toBe(VERSION);
});

// The runner pin in the rendered mise.toml is the version of the package that rendered it, with no runnerVersion option set.
for (const pinChecksum of [false, true]) {
  test(`the mise builtin pins the runner at the package version when loaded from a package URI${pinChecksum ? " that carries the pinned checksum" : ""}`, async () => {
    const json = await renderThroughPackage(`import "@@BUILTINS@@"\noptions { toolchain = "1.96.1" }\nfiles { ...Builtins.miseFiles(options) }`, { pinChecksum });
    const mise = json.files.find((f: { path: string }) => f.path === "mise.toml").content as string;
    expect(mise).toContain(`"github:pantheon-org/pantheon-scaffold" = "${VERSION}"`);
  });
}
